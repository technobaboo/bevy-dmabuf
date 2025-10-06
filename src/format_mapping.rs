// pub fn drm_fourcc_to_vk_format(drm_format: drm_fourcc::DrmFourcc) -> Option<vk::Format> {
//     use drm_fourcc::DrmFourcc as D;
//     use vk::Format as F;
//     Some(match drm_format {
//         D::Abgr1555 | D::Xbgr1555 => F::R5G5B5A1_UNORM_PACK16,
//         D::Abgr2101010 | D::Xbgr2101010 => F::A2B10G10R10_UNORM_PACK32,
//         D::Abgr4444 | D::Xbgr4444 => F::A4B4G4R4_UNORM_PACK16,
//         D::Abgr8888 | D::Xbgr8888 => F::R8G8B8A8_UNORM,
//         D::Argb1555 | D::Xrgb1555 => F::A1R5G5B5_UNORM_PACK16,
//         D::Argb2101010 | D::Xrgb2101010 => F::A2R10G10B10_UNORM_PACK32,
//         D::Argb4444 | D::Xrgb4444 => F::B4G4R4A4_UNORM_PACK16,
//         D::Argb8888 | D::Xrgb8888 => F::B8G8R8A8_UNORM,
//         D::Bgr565 => F::B5G6R5_UNORM_PACK16,
//         D::Bgr888 => F::B8G8R8_UNORM,
//         D::Bgr888_a8 => F::B8G8R8A8_UNORM,
//         D::Bgra4444 | D::Bgrx4444 => F::B4G4R4A4_UNORM_PACK16,
//         D::Bgra5551 | D::Bgrx5551 => F::B5G5R5A1_UNORM_PACK16,
//         D::Bgra8888 | D::Bgrx8888 => F::B8G8R8A8_UNORM,
//         D::R16 => F::R16_UNORM,
//         D::R8 => F::R8_UNORM,
//         D::Rg1616 => F::R16G16_UNORM,
//         D::Rg88 => F::R8G8_UNORM,
//         D::Rgb565 => F::R5G6B5_UNORM_PACK16,
//         D::Rgb888 => F::R8G8B8_UNORM,
//         D::Rgb888_a8 => F::R8G8B8A8_UNORM,
//         D::Rgba4444 | D::Rgbx4444 => F::R4G4B4A4_UNORM_PACK16,
//         D::Rgba5551 | D::Rgbx5551 => F::R5G5B5A1_UNORM_PACK16,
//         D::Rgba8888 | D::Rgbx8888 => F::R8G8B8A8_UNORM,
//         _ => return None,
//     })
// }

// pub fn vk_format_to_drm_fourcc(vk_format: vk::Format) -> Option<drm_fourcc::DrmFourcc> {
//     use drm_fourcc::DrmFourcc as D;
//     use vk::Format as F;
//     Some(match vk_format {
//         F::A2B10G10R10_UNORM_PACK32 | F::A2B10G10R10_SINT_PACK32 => D::Abgr2101010,
//         F::A2R10G10B10_UNORM_PACK32 | F::A2R10G10B10_SINT_PACK32 => D::Argb2101010,
//         F::B8G8R8_UNORM | F::B8G8R8_SINT => D::Bgr888,
//         F::R8G8B8A8_UNORM | F::R8G8B8A8_SINT => D::Rgba8888,
//         F::R8G8B8_UNORM | F::R8G8B8_SINT => D::Rgb888,
//         F::B8G8R8A8_UNORM | F::B8G8R8A8_SINT => D::Bgra8888,
//         F::R16_UNORM | F::R16_SINT => D::R16,
//         F::R8_UNORM | F::R8_SINT => D::R8,
//         F::R16G16_UNORM | F::R16G16_SINT => D::Rg1616,
//         F::R8G8_UNORM | F::R8G8_SINT => D::Rg88,

//         F::A4B4G4R4_UNORM_PACK16 => D::Abgr4444,
//         F::A1R5G5B5_UNORM_PACK16 => D::Argb1555,
//         F::B5G6R5_UNORM_PACK16 => D::Bgr565,
//         F::B4G4R4A4_UNORM_PACK16 => D::Bgra4444,
//         F::B5G5R5A1_UNORM_PACK16 => D::Bgra5551,
//         F::R5G6B5_UNORM_PACK16 => D::Rgb565,
//         F::R4G4B4A4_UNORM_PACK16 => D::Rgba4444,
//         F::R5G5B5A1_UNORM_PACK16 => D::Rgba5551,
//         _ => return None,
//     })
// }

// pub fn vk_format_to_srgb(vk_format: vk::Format) -> Option<vk::Format> {
//     use vk::Format as F;
//     Some(match vk_format {
//         F::R8_UNORM => F::R8_SRGB,
//         F::R8G8_UNORM => F::R8G8_SRGB,
//         F::R8G8B8_UNORM => F::R8G8B8_SRGB,
//         F::B8G8R8_UNORM => F::B8G8R8_SRGB,
//         F::R8G8B8A8_UNORM => F::R8G8B8A8_SRGB,
//         F::B8G8R8A8_UNORM => F::B8G8R8A8_SRGB,
//         F::A8B8G8R8_UNORM_PACK32 => F::A8B8G8R8_SRGB_PACK32,
//         F::BC1_RGB_UNORM_BLOCK => F::BC1_RGB_SRGB_BLOCK,
//         F::BC1_RGBA_UNORM_BLOCK => F::BC1_RGBA_SRGB_BLOCK,
//         F::BC2_UNORM_BLOCK => F::BC2_SRGB_BLOCK,
//         F::BC3_UNORM_BLOCK => F::BC3_SRGB_BLOCK,
//         F::BC7_UNORM_BLOCK => F::BC7_SRGB_BLOCK,
//         F::ETC2_R8G8B8_UNORM_BLOCK => F::ETC2_R8G8B8_SRGB_BLOCK,
//         F::ETC2_R8G8B8A1_UNORM_BLOCK => F::ETC2_R8G8B8A1_SRGB_BLOCK,
//         F::ETC2_R8G8B8A8_UNORM_BLOCK => F::ETC2_R8G8B8A8_SRGB_BLOCK,
//         _ => return None,
//     })
// }

/// Converts a DRM FourCC format directly to a wgpu TextureFormat.
/// This function combines the conversion logic from drm_fourcc_to_vk_format and vulkan_to_wgpu
/// to eliminate the need for Vulkan as an intermediate step.
pub fn fourcc_to_wgpu(drm_format: drm_fourcc::DrmFourcc) -> Option<wgpu::TextureFormat> {
    use drm_fourcc::DrmFourcc as D;
    use wgpu::TextureFormat as Tf;

    Some(match drm_format {
        // Basic single-channel formats
        D::R8 => Tf::R8Unorm,
        D::R16 => Tf::R16Unorm,

        // Dual-channel formats
        D::Rg88 => Tf::Rg8Unorm,
        D::Rg1616 => Tf::Rg16Unorm,

        // Packed 16-bit formats (map to closest available formats)
        D::Abgr1555 | D::Xbgr1555 => Tf::Rgba8Unorm, // Promote to 32-bit
        D::Argb1555 | D::Xrgb1555 => Tf::Bgra8Unorm, // Promote to 32-bit
        D::Abgr4444 | D::Xbgr4444 => Tf::Rgba8Unorm, // Promote to 32-bit
        D::Argb4444 | D::Xrgb4444 => Tf::Bgra8Unorm, // Promote to 32-bit
        D::Bgra4444 | D::Bgrx4444 => Tf::Bgra8Unorm, // Promote to 32-bit
        D::Bgra5551 | D::Bgrx5551 => Tf::Bgra8Unorm, // Promote to 32-bit
        D::Rgba4444 | D::Rgbx4444 => Tf::Rgba8Unorm, // Promote to 32-bit
        D::Rgba5551 | D::Rgbx5551 => Tf::Rgba8Unorm, // Promote to 32-bit

        // 16-bit RGB565 formats (promote to 32-bit since wgpu lacks these)
        D::Bgr565 => Tf::Bgra8Unorm, // Add alpha channel
        D::Rgb565 => Tf::Rgba8Unorm, // Add alpha channel

        // 24-bit formats (promote to 32-bit since wgpu doesn't support 24-bit)
        D::Rgb888 => Tf::Rgba8Unorm, // Add alpha channel
        D::Bgr888 => Tf::Bgra8Unorm, // Add alpha channel

        // 32-bit formats - main target formats
        D::Rgba8888 | D::Rgbx8888 => Tf::Rgba8Unorm,
        D::Bgra8888 | D::Bgrx8888 => Tf::Bgra8Unorm,
        D::Argb8888 | D::Xrgb8888 => Tf::Bgra8Unorm, // ARGB maps to BGRA
        D::Abgr8888 | D::Xbgr8888 => Tf::Rgba8Unorm, // ABGR maps to RGBA

        // Special 32-bit formats
        D::Rgb888_a8 => Tf::Rgba8Unorm,
        D::Bgr888_a8 => Tf::Bgra8Unorm,

        // 10-bit formats
        D::Argb2101010 | D::Xrgb2101010 => Tf::Rgb10a2Unorm,
        D::Abgr2101010 | D::Xbgr2101010 => Tf::Rgb10a2Unorm,

        _ => return None,
    })
}
