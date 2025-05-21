use std::{borrow::Cow, ffi::c_void, mem};

use ash::vk;
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_graph::{RenderLabel, ViewNode},
        render_resource::{
            CachedComputePipelineId, ComputePipelineDescriptor, PipelineCache,
            RenderPipelineDescriptor,
        },
        renderer::RenderDevice,
        texture::GpuImage,
        view::ViewTarget,
    },
};
use wgpu::hal::{Device, api::Vulkan};
pub struct DmabufExportPlugin;

impl Plugin for DmabufExportPlugin {
    fn build(&self, app: &mut App) {
        todo!()
    }
}

fn create_exported_texture(
    device: &RenderDevice,
    desc: &wgpu::hal::TextureDescriptor,
    // w: RenderAssets<>
) -> wgpu::Texture {
    unsafe {
        device
            .wgpu_device()
            .as_hal::<Vulkan, _, _>(|dev| -> Option<()> {
                let mem = dev
                    .unwrap()
                    .raw_device()
                    .allocate_memory(
                        &vk::MemoryAllocateInfo {
                            // allocation_size: desc,
                            memory_type_index: todo!(),
                            ..Default::default()
                        },
                        None,
                    )
                    .ok()?;
            });
    }
    todo!()
}

fn create_texture(dev: &wgpu::hal::vulkan::Device, desc: &wgpu::hal::TextureDescriptor) {}

fn import_texture(device: &RenderDevice) -> wgpu::Texture {
    let w = unsafe {
        device.wgpu_device().as_hal::<Vulkan, _, _>(|dev| {
            let dev = dev.unwrap();
            let mem_properties = {
                unsafe {
                    dev.shared_instance()
                        .raw_instance()
                        .get_physical_device_memory_properties(dev.raw_physical_device())
                }
            };
            let memory_types = &mem_properties.memory_types_as_slice();
            let valid_ash_memory_types =
                memory_types
                    .iter()
                    .enumerate()
                    .fold(u32::MAX, |u, (i, mem)| {
                        if (vk::MemoryPropertyFlags::DEVICE_LOCAL
                            | vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT
                            | vk::MemoryPropertyFlags::HOST_CACHED
                            | vk::MemoryPropertyFlags::LAZILY_ALLOCATED)
                            .contains(mem.property_flags)
                        {
                            u | (1 << i)
                        } else {
                            u
                        }
                    });
        })
    };
    unsafe {
        wgpu::hal::vulkan::Device::texture_from_raw(todo!(), todo!(), todo!());
    }
    todo!()
}

// #[derive(Debug, Clone, Component)]
// struct ExportedDmabufTarget {
//     image: Handle<Image>,
//     dmabuf_texture: GpuImage,
// }
//
// #[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
// struct ExportedDmabufCopyLabel;
//
// #[derive(Default)]
// struct ExportedDmabufCopyNode;
//
// impl ViewNode for ExportedDmabufCopyNode {
//     type ViewQuery = (&'static ViewTarget, &'static ExportedDmabufTarget);
//
//     fn run<'w>(
//         &self,
//         graph: &mut bevy::render::render_graph::RenderGraphContext,
//         render_context: &mut bevy::render::renderer::RenderContext<'w>,
//         (view_target, dmabuf_target): bevy::ecs::query::QueryItem<'w, Self::ViewQuery>,
//         world: &'w World,
//     ) -> Result<(), bevy::render::render_graph::NodeRunError> {
//         // view_target.out_texture()
//
//         Ok(())
//     }
// }
// #[derive(Resource)]
// struct ExportedDmabufCopyPipeline {
//     pipeline: CachedComputePipelineId,
// }
// impl FromWorld for ExportedDmabufCopyPipeline {
//     fn from_world(world: &mut World) -> Self {
//         let render_device = world.resource::<RenderDevice>();
//         let pipeline_cache = world.resource::<PipelineCache>();
//         pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
//             label: (),
//             layout: (),
//             push_constant_ranges: (),
//             shader: (),
//             shader_defs: (),
//             entry_point: (),
//             zero_initialize_workgroup_memory: false,
//         });
//         todo!()
//     }
// }

// pub fn export_dma_buf(
//     handle: &Handle<Image>,
//     assets: &RenderAssets<GpuImage>,
//     device: &wgpu::Device,
//     instance: &wgpu::Instance,
// ) {
//     let image = assets.get(handle).unwrap();
//     unsafe {
//         image
//             .texture
//             .as_hal::<Vulkan, _, _>(|texture| -> Option<()> {
//                 let texture = texture?;
//                 let instance = instance.as_hal::<Vulkan>()?;
//                 device.as_hal::<Vulkan, _, _>(|dev| -> Option<()> {
//                     let dev = dev?;
//                     let get_mem_fd = instance
//                         .shared_instance()
//                         .raw_instance()
//                         .get_device_proc_addr(
//                             dev.raw_device().handle(),
//                             c"vkGetMemoryFdKHR".as_ptr(),
//                         )?;
//                     let get_mem_fd = unsafe {
//                         mem::transmute::<
//                             unsafe extern "system" fn(),
//                             unsafe extern "system" fn(vk::Device, &VkMemoryGetFdInfoKHR, *mut u32),
//                         >(get_mem_fd)
//                     };
//                     unsafe {
//                         let fd = 0u32;
//                         let w = vk::MemoryGetFdInfoKHR {
//                             memory: todo!(),
//                             handle_type: vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
//                             ..default()
//                         };
//                     }
//                     Some(())
//                 });
//
//                 Some(())
//             });
//     }
// }
//
// // Provided by VK_KHR_external_memory_fd
// #[repr(C)]
// struct VkMemoryGetFdInfoKHR {
//     s_type: vk::StructureType,
//     p_next: *const c_void,
//     memory: vk::DeviceMemory,
//     handle_type: vk::ExternalMemoryHandleTypeFlags,
// }
