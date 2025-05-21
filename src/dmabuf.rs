use zbus::zvariant::{self, OwnedFd};

#[derive(Debug, serde::Serialize, serde::Deserialize, zvariant::Type)]
pub struct DmabufBuffer {
    pub planes: Vec<DmabufPlane>,
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
pub struct DmabufPlane {
    pub dmabuf_fd: OwnedFd,
    // plane_id: u32,
    pub offset: u32,
    pub stride: i32,
}
