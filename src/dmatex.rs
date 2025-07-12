use zbus::zvariant::{self, OwnedFd};

/// Dmabuf Backed Texture
#[derive(Debug, serde::Serialize, serde::Deserialize, zvariant::Type)]
pub struct Dmatex {
    pub planes: Vec<DmatexPlane>,
    pub res: Resolution,
    pub modifier: u64,
    pub format: u32,
    pub flip_y: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, zvariant::Type, Copy, Clone)]
pub struct Resolution {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, zvariant::Type)]
pub struct DmatexPlane {
    pub dmabuf_fd: OwnedFd,
    pub offset: u32,
    pub stride: i32,
}
