#![warn(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use bevy::{
    app::Plugin,
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs as _, SystemSet},
        system::{Res, ResMut},
        world::World,
    },
    image::Image,
    pbr::{PreparedMaterial, StandardMaterial},
    platform::collections::HashMap,
    render::{
        Render, RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        texture::GpuImage,
    },
    utils::default,
};
use drm_fourcc::DrmFourcc;
use thiserror::Error;
use tracing::{debug, debug_span, error, warn};
use wgpu::{
    TextureUsages, TextureViewDescriptor,
    hal::{Device, MemoryFlags, TextureDescriptor, TextureUses, vulkan::Api as Vulkan},
};

use crate::{dmatex::Dmatex, format_mapping::fourcc_to_wgpu};

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        let handles = ImportedDmatexs(default());
        app.insert_resource(handles.clone());
        app.add_plugins(ExtractResourcePlugin::<ImportedDmatexs>::default());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.configure_sets(
                Render,
                (
                    DmatexRenderSystemSet::InsertIntoGpuImages
                        .in_set(RenderSet::PrepareAssets)
                        .after(prepare_assets::<GpuImage>)
                        .before(prepare_assets::<PreparedMaterial<StandardMaterial>>),
                    DmatexRenderSystemSet::AcquireDmatexs
                        .in_set(RenderSet::PrepareAssets)
                        .after(DmatexRenderSystemSet::InsertIntoGpuImages),
                    DmatexRenderSystemSet::ReleaseDmatexs.in_set(RenderSet::Cleanup),
                ),
            );
            render_app.add_systems(
                Render,
                insert_dmatex_into_gpu_images.in_set(DmatexRenderSystemSet::InsertIntoGpuImages),
            );
            // render_app.add_systems(
            //     Render,
            //     (
            //         acquire_dmatex_images.in_set(DmatexRenderSystemSet::AcquireDmatexs),
            //         release_dmatex_images.in_set(DmatexRenderSystemSet::ReleaseDmatexs),
            //     ),
            // );
        } else {
            warn!("unable to init dmabuf importing!");
        }
    }
}

#[derive(SystemSet, Hash, Debug, Clone, PartialEq, Eq, Copy)]
pub enum DmatexRenderSystemSet {
    InsertIntoGpuImages,
    AcquireDmatexs,
    ReleaseDmatexs,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct ImportedDmatexs(Arc<Mutex<HashMap<Handle<Image>, DmaImage>>>);

#[derive(Debug)]
enum DmaImage {
    UnImported(Dmatex, DropCallback, DmatexUsage),
    Imported(ImportedTexture),
}

#[derive(Clone, Copy, Debug)]
pub enum DmatexUsage {
    Sampling,
}

pub struct DropCallback(pub Option<Box<dyn FnOnce() + 'static + Send + Sync>>);
impl Debug for DropCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DropCallback").finish()
    }
}
impl Drop for DropCallback {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

impl ImportedDmatexs {
    pub fn set(
        &self,
        images: &mut Assets<Image>,
        buf: Dmatex,
        usage: DmatexUsage,
        on_drop: Option<Box<dyn FnOnce() + 'static + Send + Sync>>,
    ) -> Result<Handle<Image>, ImportError> {
        let handle = get_handle(images, &buf)?;
        #[expect(clippy::unwrap_used)]
        self.0.lock().unwrap().insert(
            handle.clone_weak(),
            DmaImage::UnImported(buf, DropCallback(on_drop), usage),
        );
        Ok(handle)
    }
    pub fn insert_imported_dmatex(
        &self,
        images: &mut Assets<Image>,
        tex: ImportedTexture,
    ) -> Handle<Image> {
        let handle = debug_span!("creating dummy image").in_scope(|| {
            images.add(Image::new_uninit(
                tex.texture.size(),
                tex.texture.dimension(),
                tex.texture.format(),
                RenderAssetUsages::RENDER_WORLD,
            ))
        });

        let _span = debug_span!("inserting image handle").entered();
        #[expect(clippy::unwrap_used)]
        self.0
            .lock()
            .unwrap()
            .insert(handle.clone_weak(), DmaImage::Imported(tex));
        handle
    }
}

// idk if we need these?? you tell me

// fn acquire_dmatex_images(world: &mut World) {
//     let device = world.resource::<RenderDevice>();
//     let dmatexs = world.resource::<ImportedDmatexs>();
//     memory_barrier(device, dmatexs, ImageQueueTransfer::Acquire);
// }
// fn release_dmatex_images(world: &mut World) {
//     let device = world.resource::<RenderDevice>();
//     let dmatexs = world.resource::<ImportedDmatexs>();
//     memory_barrier(device, dmatexs, ImageQueueTransfer::Release);
// }

// enum ImageQueueTransfer {
//     Acquire,
//     Release,
// }

// fn memory_barrier(
//     device: &RenderDevice,
//     dmatexs: &ImportedDmatexs,
//     queue_transfer_direction: ImageQueueTransfer,
// ) {
//     unsafe {
//         device.wgpu_device().as_hal::<Vulkan, _, _>(|dev| {
//             let Some(dev) = dev else {
//                 return;
//             };
//             let vk_dev = dev.raw_device();
//             let Ok(command_pool) = vk_dev
//                 .create_command_pool(
//                     &vk::CommandPoolCreateInfo {
//                         flags: vk::CommandPoolCreateFlags::TRANSIENT,
//                         queue_family_index: dev.queue_family_index(),
//                         ..Default::default()
//                     },
//                     None,
//                 )
//                 .inspect_err(|e| error!("Unable to create command pool: {e}"))
//             else {
//                 return;
//             };

//             let Ok(Some(buffer)) = dev
//                 .raw_device()
//                 .allocate_command_buffers(&vk::CommandBufferAllocateInfo {
//                     command_pool,
//                     level: vk::CommandBufferLevel::PRIMARY,
//                     command_buffer_count: 1,
//                     ..Default::default()
//                 })
//                 .inspect_err(|e| error!("Unable to allocate command buffer: {e}"))
//                 .map(|v| v.into_iter().next())
//             else {
//                 vk_dev.destroy_command_pool(command_pool, None);
//                 return;
//             };
//             let Ok(texes) = dmatexs
//                 .0
//                 .lock()
//                 .inspect_err(|e| error!("Unable to lock dmatexs: {e}"))
//             else {
//                 vk_dev.destroy_command_pool(command_pool, None);
//                 return;
//             };

//             vk_dev
//                 .begin_command_buffer(
//                     buffer,
//                     &CommandBufferBeginInfo {
//                         flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
//                         ..Default::default()
//                     },
//                 )
//                 .unwrap();

//             let vk_submit_span = debug_span!("VK dmatex image acquire").entered();
//             for image in texes
//                 .iter()
//                 .filter_map(|v| match v.1 {
//                     DmaImage::UnImported(_, _, _) => None,
//                     DmaImage::Imported(imported_texture) => Some(imported_texture),
//                 })
//                 .filter_map(|i| {
//                     i.texture
//                         .as_hal::<Vulkan, _, _>(|i| i.map(|i| i.raw_handle()))
//                 })
//             {
//                 vk_dev.cmd_pipeline_barrier(
//                     buffer,
//                     vk::PipelineStageFlags::TOP_OF_PIPE,
//                     vk::PipelineStageFlags::BOTTOM_OF_PIPE,
//                     vk::DependencyFlags::empty(),
//                     &[],
//                     &[],
//                     &[vk::ImageMemoryBarrier {
//                         src_access_mask: vk::AccessFlags::NONE,
//                         dst_access_mask: vk::AccessFlags::SHADER_READ,
//                         old_layout: vk::ImageLayout::GENERAL,
//                         new_layout: vk::ImageLayout::GENERAL,
//                         // TODO: might want to use vk::QUEUE_FAMILY_FOREIGN_EXT instead
//                         src_queue_family_index: match queue_transfer_direction {
//                             ImageQueueTransfer::Acquire => vk::QUEUE_FAMILY_EXTERNAL,
//                             ImageQueueTransfer::Release => dev.queue_family_index(),
//                         },
//                         dst_queue_family_index: match queue_transfer_direction {
//                             ImageQueueTransfer::Acquire => dev.queue_family_index(),
//                             ImageQueueTransfer::Release => vk::QUEUE_FAMILY_EXTERNAL,
//                         },
//                         image,
//                         subresource_range: vk::ImageSubresourceRange {
//                             aspect_mask: vk::ImageAspectFlags::COLOR,
//                             base_mip_level: 0,
//                             level_count: 1,
//                             base_array_layer: 0,
//                             layer_count: 1,
//                         },
//                         ..Default::default()
//                     }],
//                 );
//             }
//             drop(vk_submit_span);
//             vk_dev.end_command_buffer(buffer).unwrap();
//             let fence = vk_dev
//                 .create_fence(
//                     &vk::FenceCreateInfo {
//                         flags: vk::FenceCreateFlags::empty(),
//                         ..Default::default()
//                     },
//                     None,
//                 )
//                 .unwrap();
//             vk_dev
//                 .queue_submit(
//                     dev.raw_queue(),
//                     &[vk::SubmitInfo::default().command_buffers(&[buffer])],
//                     fence,
//                 )
//                 .unwrap();
//             vk_dev.wait_for_fences(&[fence], true, u64::MAX).unwrap();
//             vk_dev.destroy_command_pool(command_pool, None);
//         })
//     };
// }

fn insert_dmatex_into_gpu_images(
    mut gpu_images: ResMut<RenderAssets<GpuImage>>,
    imported: Res<ImportedDmatexs>,
    device: Res<RenderDevice>,
) {
    #[expect(clippy::unwrap_used)]
    let mut imported = imported.0.lock().unwrap();
    let handles = imported.keys().cloned().collect::<Vec<_>>();
    for handle in handles {
        // filter out outdated dmatexs
        if gpu_images.get(&handle).is_none() {
            imported.remove(&handle);
            continue;
        }
        if matches!(imported.get(&handle), Some(DmaImage::UnImported(_, _, _))) {
            if let Some(DmaImage::UnImported(dmabuf, on_drop, usage)) = imported.remove(&handle) {
                match import_texture(&device, dmabuf, on_drop, usage) {
                    Ok(tex) => {
                        debug!("imported dmatex");
                        imported.insert(handle.clone(), DmaImage::Imported(tex));
                    }
                    Err(err) => {
                        error!("failed to import dmatex: {err}");
                        continue;
                    }
                }
            }
        }
        let Some(render_tex) = gpu_images.get_mut(&handle) else {
            warn!("invalid texture handle (unreachable)");
            continue;
        };

        if let Some(DmaImage::Imported(tex)) = imported.get(&handle) {
            debug!("setting texture view!");
            render_tex.texture_view = tex.texture_view.clone();
            render_tex.size = tex.texture.size();
            render_tex.mip_level_count = tex.texture.mip_level_count();
            render_tex.texture = tex.texture.clone();
        } else {
            error!("unreachable");
        }
    }
}

fn get_handle(images: &mut Assets<Image>, buf: &Dmatex) -> Result<Handle<Image>, ImportError> {
    let desc = get_imported_descriptor(buf)?;
    Ok(images.add(Image::new_uninit(
        desc.size,
        desc.dimension,
        desc.format,
        RenderAssetUsages::RENDER_WORLD,
    )))
}

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("Format is not compatible with Vulkan")]
    VulkanIncompatibleFormat,
    #[error("Format is not compatible with Wgpu")]
    WgpuIncompatibleFormat,
    #[error("Wgpu Error: {0}")]
    Wgpu(#[from] wgpu::Error),
    #[error("Unsupported Modifier for Format")]
    ModifierInvalid,
    #[error("Unrecognized Fourcc/Format")]
    UnrecognizedFourcc(#[from] drm_fourcc::UnrecognizedFourcc),
    #[error("RenderDevice is not a Vulkan Device")]
    NotVulkan,
    #[error("Unable to find valid Gpu Memory type index")]
    NoValidMemoryTypes,
    #[error(
        "The number of DmaTex planes does not equal the number of planes defined by the drm modifier"
    )]
    IncorrectNumberOfPlanes,
    #[error("No Planes to Import")]
    NoPlanes,
}

fn get_imported_descriptor(buf: &Dmatex) -> Result<wgpu::TextureDescriptor<'static>, ImportError> {
    Ok(wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: buf.res.x,
            height: buf.res.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fourcc_to_wgpu(
            DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
        )
        .ok_or(ImportError::WgpuIncompatibleFormat)?,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

#[derive(Clone, Debug)]
pub struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
    usage: DmatexUsage,
}

#[tracing::instrument(level = "debug", skip(device, on_drop))]
pub fn import_texture(
    device: &RenderDevice,
    buf: Dmatex,
    on_drop: DropCallback,
    usage: DmatexUsage,
) -> Result<ImportedTexture, ImportError> {
    // import wgpu stuff here

    let descriptor = TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: buf.res.x,
            height: buf.res.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fourcc_to_wgpu(
            DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
        )
        .ok_or(ImportError::WgpuIncompatibleFormat)?,
        usage: TextureUses::COLOR_TARGET | TextureUses::PRESENT,
        memory_flags: MemoryFlags::empty(),
        view_formats: vec![],
    };
    let texture = unsafe {
        wgpu::hal::vulkan::Device::texture_from_raw(
            image,
            &descriptor,
            Some({
                let dev = device.clone();
                Box::new(move || {
                    let _on_drop = on_drop;
                    dev.wgpu_device().as_hal::<Vulkan, _, _>(move |dev| {
                        if let Some(dev) = dev {
                            for (mem, _) in mem {
                                dev.raw_device().free_memory(mem, None);
                            }
                            dev.raw_device().destroy_image(image, None);
                        }
                    });
                })
            }),
        )
    };
    let wgpu_texture = unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<Vulkan>(texture, &wgpu_desc)
    };
    let texture = Texture::from(wgpu_texture);
    let texture_view = texture.create_view(&TextureViewDescriptor {
        label: None,
        format: Some(texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(texture.usage()),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(texture.mip_level_count()),
        base_array_layer: 0,
        array_layer_count: Some(texture.depth_or_array_layers()),
    });
    Ok(ImportedTexture {
        texture,
        texture_view,
        usage,
    })
}
