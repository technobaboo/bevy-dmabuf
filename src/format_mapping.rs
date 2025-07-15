use ash::vk::{
    self, DrmFormatModifierProperties2EXT, FormatProperties, FormatProperties2,
    ImageFormatProperties,
};
use tracing::error;

pub fn get_drm_modifiers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
) -> (FormatProperties, Vec<vk::DrmFormatModifierProperties2EXT>) {
    let mut drm_modifier_list_len = vk::DrmFormatModifierPropertiesList2EXT::default();
    unsafe {
        instance.get_physical_device_format_properties2(
            physical_device,
            format,
            &mut FormatProperties2::default().push_next(&mut drm_modifier_list_len),
        );
    }
    let buf_len = drm_modifier_list_len
        .drm_format_modifier_count
        .try_into()
        .unwrap_or(usize::MAX);
    let mut buf = vec![DrmFormatModifierProperties2EXT::default(); buf_len];

    let mut drm_modifier_list =
        vk::DrmFormatModifierPropertiesList2EXT::default().drm_format_modifier_properties(&mut buf);
    let mut format_properties = FormatProperties2::default().push_next(&mut drm_modifier_list);
    unsafe {
        instance.get_physical_device_format_properties2(
            physical_device,
            format,
            &mut format_properties,
        );
    }
    let format_properties = format_properties.format_properties;
    let written_buf_len = drm_modifier_list
        .drm_format_modifier_count
        .try_into()
        .unwrap_or(usize::MAX);
    buf.truncate(written_buf_len);
    (format_properties, buf)
}

pub fn get_drm_image_modifier_info(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
    image_type: vk::ImageType,
    usage: vk::ImageUsageFlags,
    flags: vk::ImageCreateFlags,
    modifier: u64,
) -> Option<ImageFormatProperties> {
    let mut drm_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .drm_format_modifier(modifier);
    let image_format_info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(image_type)
        .usage(usage)
        .flags(flags)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .push_next(&mut drm_info);
    let mut properties = vk::ImageFormatProperties2::default();
    unsafe {
        match instance.get_physical_device_image_format_properties2(
            physical_device,
            &image_format_info,
            &mut properties,
        ) {
            Ok(_) => {}
            Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED) => return None,
            Err(err) => {
                error!("failed to get format properties: {err}");
                return None;
            }
        };
    }

    Some(properties.image_format_properties)
}

pub fn drm_fourcc_to_vk_format(drm_format: drm_fourcc::DrmFourcc) -> Option<vk::Format> {
    use drm_fourcc::DrmFourcc as D;
    use vk::Format as F;
    Some(match drm_format {
        D::Abgr1555 | D::Xbgr1555 => F::R5G5B5A1_UNORM_PACK16,
        D::Abgr2101010 | D::Xbgr2101010 => F::A2B10G10R10_UNORM_PACK32,
        D::Abgr4444 | D::Xbgr4444 => F::A4B4G4R4_UNORM_PACK16,
        D::Abgr8888 | D::Xbgr8888 => F::R8G8B8A8_UNORM,
        D::Argb1555 | D::Xrgb1555 => F::A1R5G5B5_UNORM_PACK16,
        D::Argb2101010 | D::Xrgb2101010 => F::A2R10G10B10_UNORM_PACK32,
        D::Argb4444 | D::Xrgb4444 => F::B4G4R4A4_UNORM_PACK16,
        D::Argb8888 | D::Xrgb8888 => F::B8G8R8A8_UNORM,
        D::Bgr565 => F::B5G6R5_UNORM_PACK16,
        D::Bgr888 => F::B8G8R8_UNORM,
        D::Bgr888_a8 => F::B8G8R8A8_UNORM,
        D::Bgra4444 | D::Bgrx4444 => F::B4G4R4A4_UNORM_PACK16,
        D::Bgra5551 | D::Bgrx5551 => F::B5G5R5A1_UNORM_PACK16,
        D::Bgra8888 | D::Bgrx8888 => F::B8G8R8A8_UNORM,
        D::R16 => F::R16_UNORM,
        D::R8 => F::R8_UNORM,
        D::Rg1616 => F::R16G16_UNORM,
        D::Rg88 => F::R8G8_UNORM,
        D::Rgb565 => F::R5G6B5_UNORM_PACK16,
        D::Rgb888 => F::R8G8B8_UNORM,
        D::Rgb888_a8 => F::R8G8B8A8_UNORM,
        D::Rgba4444 | D::Rgbx4444 => F::R4G4B4A4_UNORM_PACK16,
        D::Rgba5551 | D::Rgbx5551 => F::R5G5B5A1_UNORM_PACK16,
        D::Rgba8888 | D::Rgbx8888 => F::R8G8B8A8_UNORM,
        _ => return None,
    })
}

pub fn vk_format_to_drm_fourcc(vk_format: vk::Format) -> Option<drm_fourcc::DrmFourcc> {
    use drm_fourcc::DrmFourcc as D;
    use vk::Format as F;
    Some(match vk_format {
        F::A2B10G10R10_UNORM_PACK32 | F::A2B10G10R10_SINT_PACK32 => D::Abgr2101010,
        F::A2R10G10B10_UNORM_PACK32 | F::A2R10G10B10_SINT_PACK32 => D::Argb2101010,
        F::B8G8R8_UNORM | F::B8G8R8_SINT => D::Bgr888,
        F::R8G8B8A8_UNORM | F::R8G8B8A8_SINT => D::Rgba8888,
        F::R8G8B8_UNORM | F::R8G8B8_SINT => D::Rgb888,
        F::B8G8R8A8_UNORM | F::B8G8R8A8_SINT => D::Bgra8888,
        F::R16_UNORM | F::R16_SINT => D::R16,
        F::R8_UNORM | F::R8_SINT => D::R8,
        F::R16G16_UNORM | F::R16G16_SINT => D::Rg1616,
        F::R8G8_UNORM | F::R8G8_SINT => D::Rg88,

        F::A4B4G4R4_UNORM_PACK16 => D::Abgr4444,
        F::A1R5G5B5_UNORM_PACK16 => D::Argb1555,
        F::B5G6R5_UNORM_PACK16 => D::Bgr565,
        F::B4G4R4A4_UNORM_PACK16 => D::Bgra4444,
        F::B5G5R5A1_UNORM_PACK16 => D::Bgra5551,
        F::R5G6B5_UNORM_PACK16 => D::Rgb565,
        F::R4G4B4A4_UNORM_PACK16 => D::Rgba4444,
        F::R5G5B5A1_UNORM_PACK16 => D::Rgba5551,
        _ => return None,
    })
}

pub fn vk_format_to_srgb(vk_format: vk::Format) -> Option<vk::Format> {
    use vk::Format as F;
    Some(match vk_format {
        F::R8_UNORM => F::R8_SRGB,
        F::R8G8_UNORM => F::R8G8_SRGB,
        F::R8G8B8_UNORM => F::R8G8B8_SRGB,
        F::B8G8R8_UNORM => F::B8G8R8_SRGB,
        F::R8G8B8A8_UNORM => F::R8G8B8A8_SRGB,
        F::B8G8R8A8_UNORM => F::B8G8R8A8_SRGB,
        F::A8B8G8R8_UNORM_PACK32 => F::A8B8G8R8_SRGB_PACK32,
        F::BC1_RGB_UNORM_BLOCK => F::BC1_RGB_SRGB_BLOCK,
        F::BC1_RGBA_UNORM_BLOCK => F::BC1_RGBA_SRGB_BLOCK,
        F::BC2_UNORM_BLOCK => F::BC2_SRGB_BLOCK,
        F::BC3_UNORM_BLOCK => F::BC3_SRGB_BLOCK,
        F::BC7_UNORM_BLOCK => F::BC7_SRGB_BLOCK,
        F::ETC2_R8G8B8_UNORM_BLOCK => F::ETC2_R8G8B8_SRGB_BLOCK,
        F::ETC2_R8G8B8A1_UNORM_BLOCK => F::ETC2_R8G8B8A1_SRGB_BLOCK,
        F::ETC2_R8G8B8A8_UNORM_BLOCK => F::ETC2_R8G8B8A8_SRGB_BLOCK,
        _ => return None,
    })
}
