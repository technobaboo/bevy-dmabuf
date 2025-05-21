use bevy::prelude::*;
use bevy_dmabuf::wgpu_init::add_dmabuf_init_plugin;

fn main() -> AppExit {
    App::new()
        .add_plugins(add_dmabuf_init_plugin(DefaultPlugins))
        .run()
}
