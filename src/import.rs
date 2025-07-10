#![warn(clippy::unwrap_used, clippy::expect_used)]
use std::{
    os::fd::{IntoRawFd as _, OwnedFd},
    sync::{Arc, Mutex},
};

use ash::vk::{self, SubresourceLayout};
use bevy::{
    app::Plugin,
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        resource::Resource,
        schedule::IntoScheduleConfigs as _,
        system::{Res, ResMut},
    },
    image::Image,
    pbr::{PreparedMaterial, StandardMaterial},
    platform::collections::HashMap,
    render::{
        RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetDependency as _, RenderAssets, prepare_assets},
        render_resource::{Texture, TextureView},
        renderer::RenderDevice,
        texture::GpuImage,
    },
    utils::default,
};
use drm_fourcc::DrmFourcc;
use thiserror::Error;
use tracing::{error, info, warn};
use wgpu::{
    TextureUsages, TextureViewDescriptor,
    hal::{MemoryFlags, TextureDescriptor, TextureUses, vulkan::Api as Vulkan},
};

use crate::{
    dmatex::Dmatex,
    format_mapping::{drm_fourcc_to_vk_format, get_drm_image_modifier_info, get_drm_modifiers},
    wgpu_init::vulkan_to_wgpu,
};

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        let handles = ImportedDmatexs(default());
        app.insert_resource(handles.clone());
        app.add_plugins(ExtractResourcePlugin::<ImportedDmatexs>::default());
        if let Some(renderapp) = app.get_sub_app_mut(RenderApp) {
            GpuImage::register_system(
                renderapp,
                do_stuff
                    .in_set(RenderSet::PrepareAssets)
                    .before(prepare_assets::<PreparedMaterial<StandardMaterial>>),
            );
        } else {
            warn!("unable to init dmabuf importing!");
        }
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct ImportedDmatexs(Arc<Mutex<HashMap<Handle<Image>, DmaImage>>>);

enum DmaImage {
    UnImported(Dmatex, DropCallback),
    Imported(ImportedTexture),
}

struct DropCallback(Option<Box<dyn FnOnce() + 'static + Send + Sync>>);
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
        on_drop: Option<Box<dyn FnOnce() + 'static + Send + Sync>>,
    ) -> Result<Handle<Image>, ImportError> {
        let handle = get_handle(images, &buf)?;
        #[expect(clippy::unwrap_used)]
        self.0.lock().unwrap().insert(
            handle.clone_weak(),
            DmaImage::UnImported(buf, DropCallback(on_drop)),
        );
        Ok(handle)
    }
}

fn do_stuff(
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
        if matches!(imported.get(&handle), Some(DmaImage::UnImported(_, _))) {
            if let Some(DmaImage::UnImported(dmabuf, on_drop)) = imported.remove(&handle) {
                match import_texture(&device, dmabuf, on_drop) {
                    Ok(tex) => {
                        info!("imported dmatex");
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

        if let Some(DmaImage::Imported(tex)) = imported.remove(&handle) {
            info!("setting texture view!");
            render_tex.texture_view = tex.texture_view;
            render_tex.size = tex.texture.size();
            render_tex.mip_level_count = tex.texture.mip_level_count();
            render_tex.texture = tex.texture;
        } else {
            error!("unreachable");
        }
    }
}

fn get_handle(images: &mut Assets<Image>, buf: &Dmatex) -> Result<Handle<Image>, ImportError> {
    let desc = get_imported_descriptor(buf)?;
    Ok(images.add(Image::new_fill(
        desc.size,
        desc.dimension,
        &[255, 255, 255, 255],
        desc.format,
        RenderAssetUsages::RENDER_WORLD,
    )))
}

#[derive(Error, Debug, Clone, Copy)]
pub enum ImportError {
    #[error("Format is not compatible with Vulkan")]
    VulkanIncompatibleFormat,
    #[error("Format is not compatible with Wgpu")]
    WgpuIncompatibleFormat,
    #[error("Unsupported Modifier for Format")]
    ModifierInvalid,
    #[error("Unable to create Vulkan Image: {0}")]
    VulkanImageCreationFailed(vk::Result),
    #[error("Unrecognized Fourcc/Format")]
    UnrecognizedFourcc(#[from] drm_fourcc::UnrecognizedFourcc),
    #[error("RenderDevice is not a Vulkan Device")]
    NotVulkan,
    #[error("Unable to find valid Gpu Memory type index")]
    NoValidMemoryTypes,
    #[error("Unable to allocate Vulkan Gpu Memory: {0}")]
    VulkanMemoryAllocFailed(vk::Result),
    #[error("Unable to bind Vulkan Gpu Memory to Vulkan Image: {0}")]
    VulkanImageMemoryBindFailed(vk::Result),
}

fn get_imported_descriptor(buf: &Dmatex) -> Result<wgpu::TextureDescriptor<'static>, ImportError> {
    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
    .ok_or(ImportError::VulkanIncompatibleFormat)?;
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
        format: vulkan_to_wgpu(vulkan_format).ok_or(ImportError::WgpuIncompatibleFormat)?,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

pub struct ImportedTexture {
    texture: Texture,
    texture_view: TextureView,
}

#[tracing::instrument(level = "debug", skip(device, on_drop))]
fn import_texture(
    device: &RenderDevice,
    buf: Dmatex,
    on_drop: DropCallback,
) -> Result<ImportedTexture, ImportError> {
    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
    .ok_or(ImportError::VulkanIncompatibleFormat)?;
    let wgpu_desc = get_imported_descriptor(&buf)?;
    let (image, mem) = unsafe {
        device
            .wgpu_device()
            .as_hal::<Vulkan, _, _>(|dev| -> Result<_, ImportError> {
                let dev = dev.ok_or(ImportError::NotVulkan)?;
                let (_format_properties, drm_format_properties) = get_drm_modifiers(
                    dev.shared_instance().raw_instance(),
                    dev.raw_physical_device(),
                    vulkan_format,
                );
                let _used_modifier = drm_format_properties
                    .iter()
                    .find(|v| v.drm_format_modifier == buf.modifier)
                    .ok_or(ImportError::ModifierInvalid)?;

                let image_type = vk::ImageType::TYPE_2D;
                let usage_flags = vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST;
                let create_flags = vk::ImageCreateFlags::empty();
                let _format_info = get_drm_image_modifier_info(
                    dev.shared_instance().raw_instance(),
                    dev.raw_physical_device(),
                    vulkan_format,
                    image_type,
                    usage_flags,
                    create_flags,
                    buf.modifier,
                )
                .ok_or(ImportError::ModifierInvalid);
                let plane_layouts = buf
                    .planes
                    .iter()
                    .map(|p| SubresourceLayout {
                        offset: p.offset as _,
                        size: 0,
                        row_pitch: p.stride as _,
                        array_pitch: 0,
                        depth_pitch: 0,
                    })
                    .collect::<Vec<_>>();
                let modifiers = vec![buf.modifier; buf.planes.len()];
                let mut drm_explicit_create_info = (buf.planes.len() == 1).then(|| {
                    vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                        .drm_format_modifier(buf.modifier)
                        .plane_layouts(&plane_layouts)
                });
                let mut drm_list_create_info = (buf.planes.len() > 1).then(|| {
                    vk::ImageDrmFormatModifierListCreateInfoEXT::default()
                        .drm_format_modifiers(&modifiers)
                });
                let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default()
                    .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);

                let mut image_create_info = vk::ImageCreateInfo::default()
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .image_type(image_type)
                    .usage(usage_flags)
                    .flags(create_flags)
                    .format(vulkan_format)
                    .extent(vk::Extent3D {
                        width: buf.res.x,
                        height: buf.res.y,
                        depth: 1,
                    })
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .array_layers(1)
                    .mip_levels(1)
                    .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                    .push_next(&mut external_memory_info);
                if let Some(info) = drm_explicit_create_info.as_mut() {
                    image_create_info = image_create_info.push_next(info);
                }
                if let Some(info) = drm_list_create_info.as_mut() {
                    image_create_info = image_create_info.push_next(info);
                }
                let image = dev
                    .raw_device()
                    .create_image(
                        &image_create_info,
                        None,
                        // Some(vk::AllocationCallbacks::default().pfn_allocation()),
                    )
                    .map_err(ImportError::VulkanImageCreationFailed)?;

                let fd = OwnedFd::from(buf.dmabuf_fd).into_raw_fd();
                let mut external_fd_info = vk::ImportMemoryFdInfoKHR::default()
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                    .fd(fd);

                let mem_properties = {
                    dev.shared_instance()
                        .raw_instance()
                        .get_physical_device_memory_properties(dev.raw_physical_device())
                };
                let memory_types = &mem_properties.memory_types_as_slice();
                let valid_memory_types =
                    memory_types
                        .iter()
                        .enumerate()
                        .fold(u32::MAX, |u, (i, mem)| {
                            if (vk::MemoryPropertyFlags::RDMA_CAPABLE_NV
                                | vk::MemoryPropertyFlags::DEVICE_COHERENT_AMD
                                | vk::MemoryPropertyFlags::PROTECTED
                                | vk::MemoryPropertyFlags::LAZILY_ALLOCATED)
                                .intersects(mem.property_flags)
                            {
                                u & !(1 << i)
                            } else {
                                u
                            }
                        });
                let index = memory_types
                    .iter()
                    .zip(0u32..)
                    .find(|(t, _)| {
                        t.property_flags
                            .intersects(vk::MemoryPropertyFlags::from_raw(valid_memory_types))
                    })
                    .ok_or(ImportError::NoValidMemoryTypes)?
                    .1;
                let reqs = dev.raw_device().get_image_memory_requirements(image);
                let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(index)
                    .push_next(&mut external_fd_info)
                    .push_next(&mut dedicated);
                let mem = dev
                    .raw_device()
                    .allocate_memory(&alloc_info, None)
                    .inspect_err(|_| dev.raw_device().destroy_image(image, None))
                    .map_err(ImportError::VulkanMemoryAllocFailed)?;
                dev.raw_device()
                    .bind_image_memory(image, mem, 0)
                    .map_err(ImportError::VulkanImageMemoryBindFailed)?;

                Ok((image, mem))
            })
    }?;
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
        format: vulkan_to_wgpu(vulkan_format).ok_or(ImportError::WgpuIncompatibleFormat)?,
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
                    info!("dropping dmatex wgpu texture");
                    dev.wgpu_device().as_hal::<Vulkan, _, _>(move |dev| {
                        if let Some(dev) = dev {
                            dev.raw_device().free_memory(mem, None);
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
    })
}
