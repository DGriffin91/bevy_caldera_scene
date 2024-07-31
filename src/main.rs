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
    render::view::NoFrustumCulling,
    window::{PresentMode, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use camera_controller::{CameraController, CameraControllerPlugin};

use crate::light_consts::lux;

#[derive(FromArgs, Resource, Clone)]
/// Config
pub struct Args {
    /// disable bloom, AO, AA, shadows
    #[argh(switch)]
    minimal: bool,

    /// whether to disable frustum culling.
    #[argh(switch)]
    no_frustum_culling: bool,
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
        .add_systems(Update, (input, benchmark));
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
                illuminance: lux::OVERCAST_DAY,
                shadows_enabled: !args.minimal,
                shadow_depth_bias: 0.2,
                shadow_normal_bias: 0.2,
            },
            cascade_shadow_config: CascadeShadowConfigBuilder {
                num_cascades: 4,
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
