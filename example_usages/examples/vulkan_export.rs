use ash::{Device, Entry, Instance, vk};
use bevy_dmabuf::{
    dmabuf::{DmabufBuffer, DmabufPlane},
    format_mapping::get_drm_modifiers,
};
use std::{
    ffi::{CStr, c_char},
    os::fd::{self, FromRawFd},
};

use example_usages::TestInterfaceProxy;

#[tokio::main]
async fn main() {
    let conn = zbus::connection::Connection::session().await.unwrap();
    let proxy = TestInterfaceProxy::builder(&conn).build().await.unwrap();
    let vk = create_instance();
    let image = create_exportable_image(
        &vk,
        vk::Format::R8G8B8A8_UNORM,
        vk::Extent3D {
            width: 2,
            height: 2,
            depth: 1,
        },
    );
    write_clear_color(&vk, &image, [127, 255, 255, 255]);
    let planes = get_planes(&image)
        .map(|r| unsafe { vk.dev.get_image_subresource_layout(image.image, r) })
        .map(|p| DmabufPlane {
            // dmabuf_fd: image.fd.into(),
            offset: p.offset as u32,
            stride: p.row_pitch as i32,
        })
        .collect();
    println!("fd: {:?}", image.fd);
    proxy
        .dmabuf(DmabufBuffer {
            dmabuf_fd: image.fd.try_clone().unwrap().into(),
            planes,
            res: bevy_dmabuf::dmabuf::Resolution { x: 512, y: 512 },
            modifier: image.modifier,
            format: bevy_dmabuf::format_mapping::vk_format_to_drm_fourcc(vk::Format::R8G8B8A8_UNORM)
                .unwrap() as u32,
            flip_y: false,
        })
        .await
        .unwrap();
    tokio::signal::ctrl_c().await.unwrap();
}

fn get_planes(image: &ExportedImage) -> impl Iterator<Item = vk::ImageSubresource> {
    (0..image.planes).map(|i| vk::ImageSubresource {
        aspect_mask: plane_flag_from_plane_index(i),
        mip_level: 0,
        array_layer: 0,
    })
}

fn plane_flag_from_plane_index(index: u32) -> vk::ImageAspectFlags {
    match index {
        0 => vk::ImageAspectFlags::MEMORY_PLANE_0_EXT,
        1 => vk::ImageAspectFlags::MEMORY_PLANE_1_EXT,
        2 => vk::ImageAspectFlags::MEMORY_PLANE_2_EXT,
        3 => vk::ImageAspectFlags::MEMORY_PLANE_3_EXT,
        _ => panic!("invalid index"),
    }
}

fn write_clear_color(vk: &VulkanInfo, image: &ExportedImage, color: [u32; 4]) {
    let buffer = vk.command_buffers[0];
    unsafe {
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        vk.dev
            .reset_command_buffer(buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES)
            .unwrap();
        vk.dev.begin_command_buffer(buffer, &begin_info).unwrap();
        let plane_ranges = get_planes(image)
            .map(|_| vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .collect::<Vec<_>>();
        vk.dev.cmd_clear_color_image(
            buffer,
            image.image,
            vk::ImageLayout::GENERAL,
            &vk::ClearColorValue { uint32: color },
            &plane_ranges,
        );
        vk.dev.end_command_buffer(buffer).unwrap();
        let command_buffers = &[buffer];

        let submit_info = vk::SubmitInfo::default().command_buffers(command_buffers);
        let fence = vk
            .dev
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .unwrap();
        vk.dev
            .queue_submit(vk.render_queue, &[submit_info], fence)
            .expect("queue submit failed.");
        vk.dev.wait_for_fences(&[fence], true, u64::MAX).unwrap();
    }
}

fn create_exportable_image(
    vk: &VulkanInfo,
    format: vk::Format,
    size: vk::Extent3D,
) -> ExportedImage {
    let (_format_properties, drm_format_properties) =
        get_drm_modifiers(&vk.instance, vk.phys_dev, format);
    let drm_modifiers = drm_format_properties
        .iter()
        .map(|v| v.drm_format_modifier)
        .collect::<Vec<_>>();
    let mut drm_list_info =
        vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(&drm_modifiers);
    let mut external_creation_info = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);

    let image_type = vk::ImageType::TYPE_2D;
    let usage_flags = vk::ImageUsageFlags::COLOR_ATTACHMENT
        | vk::ImageUsageFlags::SAMPLED
        | vk::ImageUsageFlags::TRANSFER_SRC
        | vk::ImageUsageFlags::TRANSFER_DST;
    let image_flags = vk::ImageCreateFlags::empty();
    let image_create_info = vk::ImageCreateInfo {
        flags: image_flags,
        image_type,
        format,
        extent: size,
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT,
        usage: usage_flags,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        ..Default::default()
    }
    .push_next(&mut drm_list_info)
    .push_next(&mut external_creation_info);
    let image = unsafe { vk.dev.create_image(&image_create_info, None) }.unwrap();
    let drm_modifier = {
        let drm_ext_dev = ash::ext::image_drm_format_modifier::Device::new(&vk.instance, &vk.dev);
        let mut properties = vk::ImageDrmFormatModifierPropertiesEXT::default();
        unsafe { drm_ext_dev.get_image_drm_format_modifier_properties(image, &mut properties) }
            .unwrap();
        properties.drm_format_modifier
    };

    let mem_properties = unsafe {
        vk.instance
            .get_physical_device_memory_properties(vk.phys_dev)
    };

    let image_mem_requirements = unsafe { vk.dev.get_image_memory_requirements(image) };
    let mem_type_index = find_memorytype_index(
        &image_mem_requirements,
        &mem_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .unwrap();

    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut export_alloc_info = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(image_mem_requirements.size)
        .memory_type_index(mem_type_index)
        .push_next(&mut dedicated)
        .push_next(&mut export_alloc_info);
    let mem = unsafe { vk.dev.allocate_memory(&alloc_info, None) }.unwrap();
    unsafe { vk.dev.bind_image_memory(image, mem, 0).unwrap() };
    let fd_mem_dev = ash::khr::external_memory_fd::Device::new(&vk.instance, &vk.dev);
    let get_fd_info = vk::MemoryGetFdInfoKHR::default()
        .memory(mem)
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let fd = unsafe { fd_mem_dev.get_memory_fd(&get_fd_info).unwrap() };
    ExportedImage {
        image,
        memory: mem,
        fd: unsafe { fd::OwnedFd::from_raw_fd(fd) },
        modifier: drm_modifier,
        device: vk.dev.clone(),
        planes: drm_format_properties
            .iter()
            .find(|m| m.drm_format_modifier == drm_modifier)
            .unwrap()
            .drm_format_modifier_plane_count,
    }
}

struct ExportedImage {
    image: vk::Image,
    memory: vk::DeviceMemory,
    fd: fd::OwnedFd,
    modifier: u64,
    planes: u32,
    device: Device,
}
impl Drop for ExportedImage {
    fn drop(&mut self) {
        unsafe {
            self.device.free_memory(self.memory, None);
        }
    }
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

const DEVICE_EXTS: [&CStr; 4] = [
    ash::ext::external_memory_dma_buf::NAME,
    ash::khr::external_memory_fd::NAME,
    ash::khr::external_memory::NAME,
    ash::ext::image_drm_format_modifier::NAME,
];

struct VulkanInfo {
    instance: Instance,
    phys_dev: vk::PhysicalDevice,
    dev: Device,
    render_queue: vk::Queue,
    command_buffers: Vec<vk::CommandBuffer>,
}

fn create_instance() -> VulkanInfo {
    unsafe {
        let entry = Entry::load().unwrap();
        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layers_names_raw: Vec<*const c_char> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();
        let appinfo = vk::ApplicationInfo::default()
            .application_name(c"Dmabuf Test Export")
            .application_version(0)
            .engine_name(c"No Engine")
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 3, 0));
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&appinfo)
            .enabled_layer_names(&layers_names_raw);

        let instance = entry
            .create_instance(&create_info, None)
            .expect("Instance creation error");
        let pdevices = instance
            .enumerate_physical_devices()
            .expect("Physical device error");
        let (phys_dev, queue_index) = pdevices
            .iter()
            .find_map(|phys_dev| {
                instance
                    .get_physical_device_queue_family_properties(*phys_dev)
                    .iter()
                    .enumerate()
                    .find(|(_, queue)| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                    .map(|(index, _)| (*phys_dev, index as u32))
            })
            .expect("Can't find phys dev and render queue");

        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_index)
            .queue_priorities(&priorities);

        let dev_ext_pointers = DEVICE_EXTS.map(|s| s.as_ptr());

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&dev_ext_pointers);

        let device = instance
            .create_device(phys_dev, &device_create_info, None)
            .unwrap();
        let render_queue = device.get_device_queue(queue_index, 0);

        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_index);

        let pool = device.create_command_pool(&pool_create_info, None).unwrap();

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_buffer_count(2)
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY);

        let command_buffers = device
            .allocate_command_buffers(&command_buffer_allocate_info)
            .unwrap();

        VulkanInfo {
            instance,
            phys_dev,
            // queue_family_index: queue_index,
            dev: device,
            render_queue,
            command_buffers,
        }
    }
}
