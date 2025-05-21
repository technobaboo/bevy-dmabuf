use std::ops::Deref;

use bevy::{
    asset::RenderAssetUsages,
    image::{TextureFormatPixelInfo as _, Volume as _},
    prelude::*,
    render::{
        RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetDependency as _, RenderAssets},
        render_resource::Texture,
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use wgpu::TextureViewDescriptor;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, update_mat)
        .add_plugins(ExtractResourcePlugin::<ImageHandle>::default());
    if let Some(renderapp) = app.get_sub_app_mut(RenderApp) {
        GpuImage::register_system(renderapp, do_stuff.in_set(RenderSet::PrepareAssets));
    } else {
        warn!("unable to init dmabuf importing!");
    }
    app.run()
}

fn update_mat(mut materials: ResMut<Assets<StandardMaterial>>, mat: Res<MatHandle>) {
    materials.get_mut(&mat.0);
}

fn do_stuff(
    mut cmds: Commands,
    mut render_asset: ResMut<RenderAssets<GpuImage>>,
    textures: Option<ResMut<Textures>>,
    dev: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    handle: Res<ImageHandle>,
) {
    if textures.is_none() {
        let byte_len = DESCRIPTOR.format.pixel_size() * DESCRIPTOR.size.volume();
        let data1 = [255, 255, 255, 255]
            .iter()
            .copied()
            .cycle()
            .take(byte_len)
            .collect::<Vec<_>>();
        let data2 = [0, 0, 0, 255]
            .iter()
            .copied()
            .cycle()
            .take(byte_len)
            .collect::<Vec<_>>();
        let tex = Textures(
            dev.create_texture_with_data(
                &queue,
                &DESCRIPTOR,
                wgpu::util::TextureDataOrder::default(),
                &data1,
            ),
            dev.create_texture_with_data(
                &queue,
                &DESCRIPTOR,
                wgpu::util::TextureDataOrder::default(),
                &data2,
            ),
        );
        cmds.insert_resource(tex.clone());
    }
    if let Some(mut textures) = textures {
        let Some(render_tex) = render_asset.get_mut(&handle.0) else {
            warn!("invalid texture handle");
            return;
        };
        info!("setting texture! :3");
        let tex = textures.get().deref().clone();
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
}

#[derive(Resource, Clone)]
struct Textures(Texture, Texture);
impl Textures {
    fn get(&mut self) -> Texture {
        std::mem::swap(&mut self.0, &mut self.1);
        self.0.clone()
    }
}
#[derive(Resource, Clone, ExtractResource)]
struct ImageHandle(Handle<Image>);
#[derive(Resource, Clone, ExtractResource)]
struct MatHandle(Handle<StandardMaterial>);

const DESCRIPTOR: wgpu::TextureDescriptor = wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d {
        width: 512,
        height: 512,
        depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::TEXTURE_BINDING,
    view_formats: &[],
};

// set up a simple 3D scene
fn setup(
    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let desc = DESCRIPTOR;
    let handle = images.add(Image::new_fill(
        desc.size,
        desc.dimension,
        &[255, 0, 255, 255],
        desc.format,
        RenderAssetUsages::RENDER_WORLD,
    ));
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
    // circular base
    cmds.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // light
    cmds.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    cmds.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    cmds.insert_resource(ImageHandle(handle));
    cmds.insert_resource(MatHandle(mat_handle));
}
