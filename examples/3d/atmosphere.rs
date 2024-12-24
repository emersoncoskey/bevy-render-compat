//! This example showcases pbr atmospheric scattering

use std::f32::consts::PI;

use bevy::{
    pbr::{Atmosphere, AtmosphereSettings, CascadeShadowConfigBuilder},
    prelude::*,
};
use bevy_internal::core_pipeline::tonemapping::Tonemapping;
use light_consts::lux;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup_camera_fog, setup_terrain_scene))
        .add_systems(Update, rotate_sun)
        .add_systems(Update, translate_camera)
        .run();
}

fn setup_camera_fog(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Tonemapping::AcesFitted,
        Transform::from_xyz(-1.2, 0.15, 0.0).looking_at(Vec3::Y * 0.1, Vec3::Y),
        Atmosphere::EARTH,
        AtmosphereSettings {
            scene_units_to_km: 1.0,
            ..Default::default()
        },
    ));
}

fn setup_terrain_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Configure a properly scaled cascade shadow map for this scene (defaults are too large, mesh units are in km)
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        first_cascade_far_bound: 0.3,
        maximum_distance: 3.0,
        ..default()
    }
    .build();

    // Sun
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            illuminance: lux::AMBIENT_DAYLIGHT,
            ..default()
        },
        Transform::from_xyz(1.0, -1.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        cascade_shadow_config,
    ));

    // Terrain
    commands.spawn(SceneRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/terrain/Mountains.gltf"),
    )));
}

fn rotate_sun(mut sun: Single<&mut Transform, With<DirectionalLight>>, time: Res<Time>) {
    sun.rotate_z(time.delta_secs() * PI / 30.0);
}

fn translate_camera(mut camera: Single<&mut Transform, With<Camera>>, time: Res<Time>) {
    // camera.translation.y += time.delta_secs() * 0.2;
    if time.elapsed_secs() < 5.0 {
        return;
    }
    camera.rotate_z(time.delta_secs() * PI / 30.0);
}