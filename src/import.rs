use std::{
    os::fd::{AsFd, AsRawFd},
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
    log::{error, info, tracing, warn},
    platform::collections::HashMap,
    render::{
        RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetDependency as _, RenderAssets},
        renderer::RenderDevice,
        texture::GpuImage,
    },
    utils::default,
};
use drm_fourcc::DrmFourcc;
use thiserror::Error;
use wgpu::{
    TextureUsages, TextureViewDescriptor,
    hal::{MemoryFlags, TextureDescriptor, TextureUses, vulkan::Api as Vulkan},
};

use crate::{
    dmabuf::DmabufBuffer,
    format_mapping::{drm_fourcc_to_vk_format, get_drm_image_modifier_info, get_drm_modifiers},
    wgpu_init::vulkan_to_wgpu,
};

pub struct DmabufImportPlugin;

impl Plugin for DmabufImportPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        let handles = ImportedDmabufs(default());
        app.insert_resource(handles.clone());
        app.add_plugins(ExtractResourcePlugin::<ImportedDmabufs>::default());
        if let Some(renderapp) = app.get_sub_app_mut(RenderApp) {
            GpuImage::register_system(renderapp, do_stuff.in_set(RenderSet::PrepareAssets));
        } else {
            warn!("unable to init dmabuf importing!");
        }
    }
}
#[expect(clippy::type_complexity)]
#[derive(Resource, Clone, ExtractResource)]
pub struct ImportedDmabufs(Arc<Mutex<HashMap<Handle<Image>, (DmabufBuffer, bool)>>>);

impl ImportedDmabufs {
    pub fn replace(&self, handle: Handle<Image>, buf: DmabufBuffer) -> Option<DmabufBuffer> {
        self.0
            .lock()
            .unwrap()
            .insert(handle, (buf, false))
            .map(|(b, _)| b)
    }
}

fn do_stuff(
    mut render_asset: ResMut<RenderAssets<GpuImage>>,
    imported: Res<ImportedDmabufs>,
    device: Res<RenderDevice>,
) {
    for (handle, (buf, handled)) in imported.0.lock().unwrap().iter_mut() {
        if !*handled {
            match import_texture(&device, buf) {
                Ok(tex) => {
                    let Some(render_tex) = render_asset.get_mut(handle) else {
                        warn!("invalid texture handle");
                        continue;
                    };
                    render_tex.texture_view = tex
                        .create_view(&TextureViewDescriptor {
                            label: None,
                            format: Some(tex.format()),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            usage: Some(tex.usage()),
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            mip_level_count: Some(tex.mip_level_count()),
                            base_array_layer: 0,
                            array_layer_count: Some(tex.depth_or_array_layers()),
                        })
                        .into();
                    render_tex.size = tex.size();
                    render_tex.mip_level_count = tex.mip_level_count();
                    render_tex.texture = tex.into();
                }
                Err(err) => {
                    error!("failed to import dmabuf: {err}");
                }
            }
            *handled = true;
        }
    }
}

pub fn get_handle(
    images: &mut Assets<Image>,
    buf: &DmabufBuffer,
) -> Result<Handle<Image>, ImportError> {
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
    #[error("Format is not compatible with vulkan")]
    FormatInvalid,
    #[error("Unsupported Modifier for Format")]
    ModifierInvalid,
    #[error("Unable to create vulkan image: {0}")]
    VulkanImageCreationFailed(#[from] vk::Result),
    #[error("Unrecognized Fourcc/Format")]
    UnrecognizedFourcc(#[from] drm_fourcc::UnrecognizedFourcc),
}

fn get_imported_descriptor(buf: &DmabufBuffer) -> Result<wgpu::TextureDescriptor, ImportError> {
    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
    .ok_or(ImportError::FormatInvalid)?;
    info!("{vulkan_format:?}");
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
        format: vulkan_to_wgpu(vulkan_format).unwrap(), /* .ok_or(ImportError::FormatInvalid)? */
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

#[tracing::instrument(level = "debug", skip(device))]
fn import_texture(device: &RenderDevice, buf: &DmabufBuffer) -> Result<wgpu::Texture, ImportError> {
    let vulkan_format = drm_fourcc_to_vk_format(
        DrmFourcc::try_from(buf.format).map_err(ImportError::UnrecognizedFourcc)?,
    )
    .ok_or(ImportError::FormatInvalid)?;
    let (image, _mem) = unsafe {
        device
            .wgpu_device()
            .as_hal::<Vulkan, _, _>(|dev| -> Result<_, ImportError> {
                let dev = dev.unwrap();
                // let mem_properties = {
                //     dev.shared_instance()
                //         .raw_instance()
                //         .get_physical_device_memory_properties(dev.raw_physical_device())
                // };
                // let drm_modifier_dev = image_drm_format_modifier::Device::new(
                //     dev.shared_instance().raw_instance(),
                //     dev.raw_device(),
                // );
                let (format_properties, drm_format_properties) = get_drm_modifiers(
                    dev.shared_instance().raw_instance(),
                    dev.raw_physical_device(),
                    vulkan_format,
                );
                let used_modifier = drm_format_properties
                    .iter()
                    .inspect(|v| {
                        info!("{v:?}");
                    })
                    .find(|v| v.drm_format_modifier == buf.modifier)
                    .ok_or(ImportError::ModifierInvalid)?;

                let image_type = vk::ImageType::TYPE_2D;
                let usage_flags = vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST;
                let create_flags = vk::ImageCreateFlags::empty();
                let format_info = get_drm_image_modifier_info(
                    dev.shared_instance().raw_instance(),
                    dev.raw_physical_device(),
                    vulkan_format,
                    image_type,
                    usage_flags,
                    create_flags,
                    buf.modifier,
                )
                .unwrap();
                // .ok_or(ImportError::ModifierInvalid)?;

                // used_modifier.drm_format_modifier_tiling_features

                // let plane_layouts = &[vk::SubresourceLayout]

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

                // let mut external_fd_mem = vk::ImportMemoryFdInfoKHR::default().fd(buf.)

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

                let fd = buf.planes.first().unwrap().dmabuf_fd.as_raw_fd();
                info!(fd);
                let mut external_fd_info = vk::ImportMemoryFdInfoKHR::default()
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                    .fd(fd);

                let mem_properties = {
                    unsafe {
                        dev.shared_instance()
                            .raw_instance()
                            .get_physical_device_memory_properties(dev.raw_physical_device())
                    }
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
                    .unwrap()
                    .1;
                let reqs = dev.raw_device().get_image_memory_requirements(image);
                info!("reqs: {reqs:?}");
                let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
                let alloc_info = vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(index)
                    .push_next(&mut external_fd_info)
                    .push_next(&mut dedicated);
                let mem = dev.raw_device().allocate_memory(&alloc_info, None).unwrap();
                info!("test");
                // let info = vk::BindImageMemoryInfo::default().image(image).memory(mem);
                dev.raw_device().bind_image_memory(image, mem, 0).unwrap();

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
        format: vulkan_to_wgpu(vulkan_format).unwrap(),
        usage: TextureUses::COLOR_TARGET | TextureUses::PRESENT,
        memory_flags: MemoryFlags::empty(),
        view_formats: vec![],
    };
    let texture = unsafe {
        wgpu::hal::vulkan::Device::texture_from_raw(
            image,
            &descriptor,
            Some(Box::new(move || {
                // TODO: setup cleanup stuff
            })),
        )
    };
    let wgpu_desc = get_imported_descriptor(buf).unwrap();
    unsafe {
        Ok(device
            .wgpu_device()
            .create_texture_from_hal::<Vulkan>(texture, &wgpu_desc))
    }
}
