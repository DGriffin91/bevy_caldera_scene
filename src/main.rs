// Press B for benchmark.
// Preferably after frame time is reading consistently, rust-analyzer has calmed down, and with locked gpu clocks.

use std::{f32::consts::PI, time::Instant};

mod camera_controller;

use argh::FromArgs;
use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
    },
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::{CascadeShadowConfigBuilder, ScreenSpaceAmbientOcclusionBundle},
    prelude::*,
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        texture::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
        view::NoFrustumCulling,
    },
    window::{PresentMode, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use camera_controller::{CameraController, CameraControllerPlugin};

use crate::light_consts::lux;

const UNIQUE_MESH_QTY: usize = 24182;
const MESH_INSTANCE_QTY: usize = 35689;
#[derive(FromArgs, Resource, Clone)]
/// Config
pub struct Args {
    /// disable bloom, AO, AA, shadows
    #[argh(switch)]
    minimal: bool,

    /// whether to disable frustum culling.
    #[argh(switch)]
    no_frustum_culling: bool,

    /// assign randomly generated materials to each unique mesh (mesh instances also share materials)
    #[argh(switch)]
    random_materials: bool,

    /// quantity of unique textures sets to randomly select from. (A texture set being: base_color, normal, roughness)
    #[argh(option, default = "0")]
    texture_count: u32,
}

pub fn main() {
    let args: Args = argh::from_env();

    let mut app = App::new();

    app.insert_resource(args.clone())
        .insert_resource(Msaa::Off)
        // Using just rgb here for bevy 0.13 compat
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::Immediate,
                resolution: WindowResolution::new(1920.0, 1080.0).with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin,
            CameraControllerPlugin,
            TemporalAntiAliasPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (assign_rng_materials, input, benchmark));
    if args.no_frustum_culling {
        app.add_systems(Update, add_no_frustum_culling);
    }

    app.run();
}

#[derive(Component)]
pub struct PostProcScene;

#[derive(Component)]
pub struct GrifLight;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, args: Res<Args>) {
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("hotel_01.glb#Scene0"),
            transform: Transform::from_scale(Vec3::splat(0.01)),
            ..default()
        },
        PostProcScene,
    ));

    // Sun
    commands
        .spawn(DirectionalLightBundle {
            transform: Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                PI * -0.35,
                PI * -0.13,
                0.0,
            )),
            directional_light: DirectionalLight {
                // Using just rgb here for bevy 0.13 compat
                color: Color::rgb(1.0, 0.87, 0.78),
                illuminance: lux::FULL_DAYLIGHT,
                shadows_enabled: !args.minimal,
                shadow_depth_bias: 0.2,
                shadow_normal_bias: 0.2,
            },
            cascade_shadow_config: CascadeShadowConfigBuilder {
                num_cascades: 3,
                minimum_distance: 0.1,
                maximum_distance: 80.0,
                first_cascade_far_bound: 5.0,
                overlap_proportion: 0.2,
            }
            .into(),
            ..default()
        })
        .insert(GrifLight);

    // Camera
    let mut cam = commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            transform: CAM_POS_1,
            projection: Projection::Perspective(PerspectiveProjection {
                fov: std::f32::consts::PI / 3.0,
                near: 0.1,
                far: 1000.0,
                aspect_ratio: 1.0,
            }),
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 1000.0,
        },
        CameraController::default().print_controls(),
    ));
    if !args.minimal {
        cam.insert((
            BloomSettings {
                intensity: 0.02,
                ..default()
            },
            TemporalAntiAliasBundle::default(),
        ))
        .insert(ScreenSpaceAmbientOcclusionBundle::default());
    }
}

// Go though each unique mesh and randomly generate a material.
// Each unique so instances are maintained.
pub fn assign_rng_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    meshes: Res<Assets<Mesh>>,
    mesh_instances: Query<(Entity, &Handle<Mesh>)>,
    args: Res<Args>,
    mut done: Local<bool>,
) {
    // TODO figure out a better way to reliably figure out things are done loading
    let all_meshes_loaded = meshes.len() == UNIQUE_MESH_QTY;
    let all_mesh_instances_loaded = mesh_instances.iter().len() == MESH_INSTANCE_QTY;

    if !args.random_materials || *done || !all_meshes_loaded || !all_mesh_instances_loaded {
        return;
    }

    let base_color_textures = (0..args.texture_count)
        .map(|i| {
            images.add(generate_random_compressed_texture_with_mipmaps(
                2048, false, i,
            ))
        })
        .collect::<Vec<_>>();
    let normal_textures = (0..args.texture_count)
        .map(|i| {
            images.add(generate_random_compressed_texture_with_mipmaps(
                2048,
                false,
                i + 1024,
            ))
        })
        .collect::<Vec<_>>();
    let roughness_textures = (0..args.texture_count)
        .map(|i| {
            images.add(generate_random_compressed_texture_with_mipmaps(
                2048,
                true,
                i + 2048,
            ))
        })
        .collect::<Vec<_>>();

    for (i, (mesh_h, _mesh)) in meshes.iter().enumerate() {
        let mut base_color_texture = None;
        let mut normal_texture = None;
        let mut roughness_texture = None;

        if !base_color_textures.is_empty() {
            base_color_texture = Some(base_color_textures[i % base_color_textures.len()].clone());
        }
        if !normal_textures.is_empty() {
            normal_texture = Some(normal_textures[i % normal_textures.len()].clone());
        }
        if !roughness_textures.is_empty() {
            roughness_texture = Some(roughness_textures[i % roughness_textures.len()].clone());
        }

        let unique_material = materials.add(StandardMaterial {
            base_color: Color::srgb(
                hash_noise(i as u32, 0, 0),
                hash_noise(i as u32, 0, 1),
                hash_noise(i as u32, 0, 2),
            ),
            base_color_texture,
            normal_map_texture: normal_texture,
            metallic_roughness_texture: roughness_texture,
            ..default()
        });
        for (entity, mesh_instance_h) in mesh_instances.iter() {
            if mesh_instance_h.id() == mesh_h {
                commands.entity(entity).insert(unique_material.clone());
            }
        }
    }

    *done = true;
}

fn generate_random_compressed_texture_with_mipmaps(size: u32, bc4: bool, seed: u32) -> Image {
    let data = (0..calculate_bcn_image_size_with_mips(size, if bc4 { 8 } else { 16 }))
        .map(|i| uhash(i, seed) as u8)
        .collect::<Vec<_>>();

    Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size: Extent3d {
                width: size,
                height: size,
                ..default()
            },
            dimension: TextureDimension::D2,
            format: if bc4 {
                TextureFormat::Bc4RUnorm
            } else {
                TextureFormat::Bc7RgbaUnormSrgb
            },
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..default()
        }),

        data,
        ..Default::default()
    }
}

const CAM_POS_1: Transform = Transform {
    translation: Vec3::new(-20.147331, 16.818098, 42.806145),
    rotation: Quat::from_array([-0.22917402, -0.34915298, -0.08848568, 0.9042908]),
    scale: Vec3::ONE,
};

const CAM_POS_2: Transform = Transform {
    translation: Vec3::new(1.6168646, 1.8304176, -5.846825),
    rotation: Quat::from_array([-0.0007061247, -0.99179053, 0.12775362, -0.005481863]),
    scale: Vec3::ONE,
};

const CAM_POS_3: Transform = Transform {
    translation: Vec3::new(23.97184, 1.8938808, 30.568554),
    rotation: Quat::from_array([-0.0013945175, 0.4685419, 0.00073959737, 0.8834399]),
    scale: Vec3::ONE,
};

fn input(input: Res<ButtonInput<KeyCode>>, mut camera: Query<&mut Transform, With<Camera>>) {
    let Ok(mut transform) = camera.get_single_mut() else {
        return;
    };
    if input.just_pressed(KeyCode::KeyI) {
        info!("{:?}", transform);
    }
    if input.just_pressed(KeyCode::Digit1) {
        *transform = CAM_POS_1
    }
    if input.just_pressed(KeyCode::Digit2) {
        *transform = CAM_POS_2
    }
    if input.just_pressed(KeyCode::Digit3) {
        *transform = CAM_POS_3
    }
}

fn benchmark(
    input: Res<ButtonInput<KeyCode>>,
    mut camera: Query<&mut Transform, With<Camera>>,
    materials: Res<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    has_std_mat: Query<&Handle<StandardMaterial>>,
    has_mesh: Query<&Handle<Mesh>>,
    mut bench_started: Local<Option<Instant>>,
    mut bench_frame: Local<u32>,
    mut count_per_step: Local<u32>,
    time: Res<Time>,
) {
    if input.just_pressed(KeyCode::KeyB) && bench_started.is_none() {
        *bench_started = Some(Instant::now());
        *bench_frame = 0;
        // Try to render for around 2s or at least 30 frames per step
        *count_per_step = ((2.0 / time.delta_seconds()) as u32).max(30);
        println!(
            "Starting Benchmark with {} frames per step",
            *count_per_step
        );
    }
    if bench_started.is_none() {
        return;
    }
    let Ok(mut transform) = camera.get_single_mut() else {
        return;
    };
    if *bench_frame == 0 {
        *transform = CAM_POS_1
    } else if *bench_frame == *count_per_step {
        *transform = CAM_POS_2
    } else if *bench_frame == *count_per_step * 2 {
        *transform = CAM_POS_3
    } else if *bench_frame == *count_per_step * 3 {
        let elapsed = bench_started.unwrap().elapsed().as_secs_f32();
        println!(
            "Benchmark avg cpu frame time: {:.2}ms",
            (elapsed / *bench_frame as f32) * 1000.0
        );
        println!(
            "Meshes: {}\nMesh Instances: {}\nMaterials: {}\nMaterial Instances: {}",
            meshes.len(),
            has_mesh.iter().len(),
            materials.len(),
            has_std_mat.iter().len(),
        );
        *bench_started = None;
        *bench_frame = 0;
        *transform = CAM_POS_1;
    }
    *bench_frame += 1;
}

pub fn add_no_frustum_culling(
    mut commands: Commands,
    convert_query: Query<Entity, (Without<NoFrustumCulling>, With<Handle<StandardMaterial>>)>,
) {
    for entity in convert_query.iter() {
        commands.entity(entity).insert(NoFrustumCulling);
    }
}

#[inline(always)]
pub fn uhash(a: u32, b: u32) -> u32 {
    let mut x = (a.overflowing_mul(1597334673).0) ^ (b.overflowing_mul(3812015801).0);
    // from https://nullprogram.com/blog/2018/07/31/
    x = x ^ (x >> 16);
    x = x.overflowing_mul(0x7feb352d).0;
    x = x ^ (x >> 15);
    x = x.overflowing_mul(0x846ca68b).0;
    x = x ^ (x >> 16);
    x
}

#[inline(always)]
pub fn unormf(n: u32) -> f32 {
    n as f32 * (1.0 / 0xffffffffu32 as f32)
}

#[inline(always)]
pub fn hash_noise(x: u32, y: u32, z: u32) -> f32 {
    let urnd = uhash(x, (y << 11) + z);
    unormf(urnd)
}

// BC7 block is 16 bytes, BC4 block is 8 bytes
fn calculate_bcn_image_size_with_mips(size: u32, block_size: u32) -> u32 {
    let mut total_size = 0;
    let mut mip_size = size;
    while mip_size > 4 {
        let num_blocks = mip_size / 4; // Round up
        let mip_level_size = num_blocks * num_blocks * block_size;
        total_size += mip_level_size;
        mip_size = (mip_size / 2).max(1);
    }
    total_size
}
