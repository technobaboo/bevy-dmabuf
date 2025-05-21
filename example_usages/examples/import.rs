use std::sync::{Mutex, mpsc};

use bevy::{
    DefaultPlugins,
    app::{App, AppExit, PostUpdate, Startup},
    asset::{Assets, Handle},
    color::Color,
    core_pipeline::core_3d::Camera3d,
    ecs::{
        resource::Resource,
        system::{Commands, Res, ResMut},
    },
    image::Image,
    math::{
        Quat, Vec3,
        primitives::{Circle, Cuboid},
    },
    pbr::{MeshMaterial3d, PointLight, StandardMaterial},
    render::mesh::{Mesh, Mesh3d},
    transform::components::Transform,
    utils::default,
};
use bevy_dmabuf::{
    dmabuf::DmabufBuffer,
    import::{DmabufImportPlugin, ImportedDmabufs, get_handle},
    wgpu_init::add_dmabuf_init_plugin,
};
use example_usages::TestInterface;

#[tokio::main]
async fn main() -> AppExit {
    let (tx, rx) = mpsc::channel();
    let _conn = zbus::connection::Builder::session()
        .unwrap()
        .name("dev.schmarni.bevy_dmabuf.example")
        .unwrap()
        .serve_at(
            "/dev/schmarni/bevy_dmabuf",
            TestInterface { dmabuf_channel: tx },
        )
        .unwrap()
        .build()
        .await
        .unwrap();

    App::new()
        .insert_resource(Receiver(rx.into()))
        .add_plugins(add_dmabuf_init_plugin(DefaultPlugins))
        .add_plugins(DmabufImportPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, update_buf)
        .run()
}
#[expect(clippy::too_many_arguments)]
fn update_buf(
    handle: Option<Res<BufHandle>>,
    bufs: Res<ImportedDmabufs>,
    mut receiv: ResMut<Receiver>,
    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    if let Some(buf) = receiv.0.get_mut().unwrap().try_iter().last() {
        if let Some((handle, mat_handle)) = handle.as_ref().map(|v| &v.0) {
            materials.get_mut(mat_handle);
            drop_buf(bufs.replace(handle.clone(), buf));
        } else {
            let handle = get_handle(&mut images, &buf).unwrap();
            drop_buf(bufs.replace(handle.clone(), buf));
            let mat_handle = materials.add(StandardMaterial {
                base_color_texture: Some(handle.clone()),
                ..default()
            });
            // cube
            cmds.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(mat_handle.clone()),
                Transform::from_xyz(0.0, 0.5, 0.0),
            ));
            commands.insert_resource(BufHandle((handle, mat_handle)));
        }
    }
}

// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn drop_buf(buf: Option<DmabufBuffer>) {
    if let Some(buf) = buf {
        buf.planes
            .into_iter()
            .for_each(|p| std::mem::forget(p.dmabuf_fd));
    }
}

#[derive(Resource)]
struct Receiver(Mutex<mpsc::Receiver<DmabufBuffer>>);
#[derive(Resource)]
struct BufHandle((Handle<Image>, Handle<StandardMaterial>));
