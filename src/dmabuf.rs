use std::os::fd::{AsRawFd, RawFd};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zbus::zvariant;

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
    pub dmabuf_fd: ZbusRawFd,
    // plane_id: u32,
    pub offset: u32,
    pub stride: i32,
}

#[derive(Debug)]
pub struct ZbusRawFd(RawFd);
impl AsRawFd for ZbusRawFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}
impl From<RawFd> for ZbusRawFd {
    fn from(value: RawFd) -> Self {
        Self(value)
    }
}
use zvariant::{Basic, Signature, Type};
macro_rules! fd_impl {
    ($i:ty) => {
        impl Basic for $i {
            const SIGNATURE_CHAR: char = 'h';
            const SIGNATURE_STR: &'static str = "h";
        }

        impl Type for $i {
            const SIGNATURE: &'static Signature = &Signature::Fd;
        }
    };
}
impl Serialize for ZbusRawFd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for ZbusRawFd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = i32::deserialize(deserializer)?;

        Ok(ZbusRawFd(raw))
    }
}

fd_impl!(ZbusRawFd);
