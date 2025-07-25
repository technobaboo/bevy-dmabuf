use std::{os::fd::OwnedFd, sync::Arc};

use bevy_dmabuf::{
    dmatex::{Dmatex, DmatexPlane, Resolution},
    format_mapping::vk_format_to_drm_fourcc,
};
use example_usages::TestInterfaceProxy;
use glam::UVec2;
use vulkano::{
    buffer::{BufferCreateFlags, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        allocator::{
            CommandBufferAllocator, StandardCommandBufferAllocator,
            StandardCommandBufferAllocatorCreateInfo,
        },
    },
    device::{Device, QueueCreateFlags, QueueCreateInfo, QueueFlags, physical::PhysicalDevice},
    image::{
        ImageAspect, ImageCreateFlags, ImageCreateInfo, ImageLayout, ImageMemory, ImageTiling,
        ImageUsage, sys::RawImage,
    },
    instance::Instance,
    memory::{
        DedicatedAllocation, DeviceMemory, ExternalMemoryHandleType, ExternalMemoryHandleTypes,
        MemoryAllocateInfo, ResourceMemory,
        allocator::{AllocationCreateInfo, MemoryAllocator, StandardMemoryAllocator},
    },
    sync::{self, GpuFuture as _, Sharing},
};

#[tokio::main]
async fn main() {
    let conn = zbus::connection::Connection::session().await.unwrap();
    let proxy = TestInterfaceProxy::builder(&conn).build().await.unwrap();
    let vk = get_ctx();
    let vk_format = vulkano::format::Format::R8G8B8A8_UNORM;

    let modifiers = vk
        .phys_dev
        .format_properties(vk_format)
        .unwrap()
        .drm_format_modifier_properties
        .into_iter()
        .map(|v| v.drm_format_modifier)
        .collect::<Vec<_>>();
    let size = UVec2::splat(2048);

    let raw_image = RawImage::new(
        vk.dev.clone(),
        ImageCreateInfo {
            flags: ImageCreateFlags::empty(),
            image_type: vulkano::image::ImageType::Dim2d,
            format: vk_format,
            view_formats: Vec::new(),
            extent: [size.x, size.y, 1],
            tiling: ImageTiling::DrmFormatModifier,
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
            initial_layout: ImageLayout::Undefined,
            drm_format_modifiers: modifiers,
            external_memory_handle_types: ExternalMemoryHandleTypes::DMA_BUF,
            ..Default::default()
        },
    )
    .unwrap();
    let (modifier, num_planes) = raw_image.drm_format_modifier().unwrap();
    let mem_reqs = raw_image.memory_requirements()[0];
    let index = vk
        .phys_dev
        .memory_properties()
        .memory_types
        .iter()
        .enumerate()
        .map(|(i, _v)| i as u32)
        .find(|i| mem_reqs.memory_type_bits & (1 << i) != 0)
        .expect("no valid memory type");
    let mem = ResourceMemory::new_dedicated(
        DeviceMemory::allocate(
            vk.dev.clone(),
            MemoryAllocateInfo {
                allocation_size: mem_reqs.layout.size(),
                memory_type_index: index,
                dedicated_allocation: Some(DedicatedAllocation::Image(&raw_image)),
                export_handle_types: ExternalMemoryHandleTypes::DMA_BUF,
                ..Default::default()
            },
        )
        .unwrap(),
    );

    let image = Arc::new(match raw_image.bind_memory([mem]) {
        Ok(v) => v,
        Err(_) => panic!("unable to bind memory"),
    });

    let ImageMemory::Normal(mem) = image.memory() else {
        unreachable!()
    };
    let [mem] = mem.as_slice() else {
        unreachable!()
    };
    let fd = OwnedFd::from(
        mem.device_memory()
            .export_fd(ExternalMemoryHandleType::DmaBuf)
            .unwrap(),
    );
    let planes = (0..num_planes)
        .filter_map(|i| {
            Some(match i {
                0 => ImageAspect::MemoryPlane0,
                1 => ImageAspect::MemoryPlane1,
                2 => ImageAspect::MemoryPlane2,
                3 => ImageAspect::MemoryPlane3,
                _ => return None,
            })
        })
        .map(|aspect| {
            let plane_layout = image.subresource_layout(aspect, 0, 0).unwrap();

            DmatexPlane {
                dmabuf_fd: fd.try_clone().unwrap().into(),
                modifier,
                offset: plane_layout.offset as u32,
                stride: plane_layout.row_pitch as i32,
            }
        })
        .collect::<Vec<_>>();
    let dmatex = Dmatex {
        planes,
        res: Resolution {
            x: size.x,
            y: size.y,
        },
        format: vk_format_to_drm_fourcc(vk_format.into()).unwrap() as u32,
        flip_y: false,
        srgb: true,
    };

    proxy.dmatex(dmatex).await.unwrap();

    let data_len = size.x * size.y * 4;

    let buffer = vulkano::buffer::Buffer::new_slice::<u8>(
        vk.alloc.clone(),
        BufferCreateInfo {
            flags: BufferCreateFlags::empty(),
            sharing: Sharing::Exclusive,
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: vulkano::memory::allocator::MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            memory_type_bits: u32::MAX & !(1 << 1),
            ..Default::default()
        },
        data_len as u64,
    )
    .unwrap();

    let mut buf_mem = buffer.write().unwrap();
    [255, 0, 255, 255u8]
        .into_iter()
        .cycle()
        .take(data_len as usize)
        .enumerate()
        .for_each(|(i, v)| buf_mem[i] = v);
    drop(buf_mem);
    let mut command_buffer = AutoCommandBufferBuilder::primary(
        vk.command_buffer_alloc.clone(),
        vk.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();
    command_buffer
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            buffer.clone(),
            image.clone(),
        ))
        .unwrap();
    let command_buffer = command_buffer.build().unwrap();
    sync::now(vk.dev.clone())
        .then_execute(vk.queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap()
        .await
        .unwrap();

    tokio::signal::ctrl_c().await.unwrap()
}

fn get_ctx() -> VulkanoContext {
    let entry = vulkano::VulkanLibrary::new().unwrap();
    let instance = Instance::new(
        entry.clone(),
        vulkano::instance::InstanceCreateInfo {
            application_name: Some("vulkano dmabuf export".to_string()),
            application_version: vulkano::Version::V1_0,
            engine_name: None,
            engine_version: vulkano::Version::V1_0,
            max_api_version: Some(vulkano::Version::V1_1),
            enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    // don't care, just pick the first dev
    let phys_dev = instance
        .enumerate_physical_devices()
        .unwrap()
        .next()
        .unwrap();
    let (index, _) = phys_dev
        .queue_family_properties()
        .iter()
        .enumerate()
        .find(|(_, props)| props.queue_flags.contains(QueueFlags::TRANSFER))
        .unwrap();
    let (dev, mut queues) = Device::new(
        phys_dev.clone(),
        vulkano::device::DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                flags: QueueCreateFlags::empty(),
                queue_family_index: index as u32,
                ..Default::default()
            }],
            enabled_extensions: vulkano::device::DeviceExtensions {
                ext_external_memory_dma_buf: true,
                ext_image_drm_format_modifier: true,
                khr_external_memory: true,
                khr_external_memory_fd: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let queue = queues.next().unwrap();

    let alloc = Arc::new(StandardMemoryAllocator::new_default(dev.clone()));
    let command_buffer_alloc = Arc::new(StandardCommandBufferAllocator::new(
        dev.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    ));
    VulkanoContext {
        instance,
        phys_dev,
        dev,
        queue,
        alloc,
        command_buffer_alloc,
    }
}
pub struct VulkanoContext {
    pub instance: Arc<vulkano::instance::Instance>,
    pub phys_dev: Arc<PhysicalDevice>,
    pub dev: Arc<vulkano::device::Device>,
    pub queue: Arc<vulkano::device::Queue>,
    pub alloc: Arc<dyn MemoryAllocator>,
    pub command_buffer_alloc: Arc<dyn CommandBufferAllocator>,
}
