use std::{fs::File, os::fd::{AsFd as _, OwnedFd}};

use bevy::{prelude::*, render::camera::RenderTarget};
use bevy_dmabuf::wgpu_init::add_dmabuf_init_plugin;
use zbus::zvariant::Fd;

fn main() -> AppExit {
    App::new()
        .add_plugins(add_dmabuf_init_plugin(DefaultPlugins))
        .add_systems(Startup, test)
        .run()
}

fn test(assets: Res<Assets<Image>>, mut cmds: Commands) {
    let cam = Camera {
        target: RenderTarget::Image(assets.reserve_handle()),
        ..default()
    };
    cmds.spawn((Camera3d::default(), cam));
}

pub struct TestInterface {
    dmabuf: OwnedFd,
    semaphore: OwnedFd,
}
#[zbus::interface(name = "dev.schmarni.bevy_dmabuf.example")]
impl TestInterface {
    fn dmabuf(&self) -> Fd {
        Fd::Borrowed(self.dmabuf.as_fd())
    }
    fn semaphore(&self) -> Fd {
        Fd::Borrowed(self.semaphore.as_fd())
    }
}
