//! Procedural Atmospheric Scattering.
//!
//! This plugin implements [Hillaire's 2020 paper](https://sebh.github.io/publications/egsr2020.pdf)
//! on real-time atmospheric scattering. While it *will* work simply as a
//! procedural skybox, it also does much more. It supports dynamic time-of-
//! -day, multiple directional lights, and since it's applied as a post-processing
//! effect *on top* of the existing skybox, a starry skybox would automatically
//! show based on the time of day. Scattering in front of terrain (similar
//! to distance fog, but more complex) is handled as well, and takes into
//! account the directional light color and direction.
//!
//! Adding the [`Atmosphere`] component to a 3d camera will enable the effect,
//! which by default is set to look similar to Earth's atmosphere. See the
//! documentation on the component itself for information regarding its fields.
//!
//! Performance-wise, the effect should be fairly cheap since the LUTs (Look
//! Up Tables) that encode most of the data are small, and take advantage of the
//! fact that the atmosphere is symmetric. Performance is also proportional to
//! the number of directional lights in the scene. In order to tune
//! performance more finely, the [`AtmosphereSettings`] camera component
//! manages the size of each LUT and the sample count for each ray.
//!
//! Given how similar it is to [`crate::volumetric_fog`], it might be expected
//! that these two modules would work together well. However for now using both
//! at once is untested, and might not be physically accurate. These may be
//! integrated into a single module in the future.
//!
//! [Shadertoy]: https://www.shadertoy.com/view/slSXRW
//!
//! [Unreal Engine Implementation]: https://github.com/sebh/UnrealEngineSkyAtmosphere

mod node;
pub mod resources;

use bevy_app::{App, Plugin};
use bevy_asset::{load_internal_asset, Asset, AssetApp, Assets, Handle};
use bevy_color::{Color, ColorToComponents, LinearRgba};
use bevy_core_pipeline::core_3d::graph::Node3d;
use bevy_ecs::{component::require, resource::Resource};
use bevy_math::{UVec2, UVec3, Vec3};
use bevy_reflect::Reflect;
use bevy_render::{
    extract_component::UniformComponentPlugin,
    render_resource::{DownlevelFlags, ShaderType, SpecializedRenderPipelines},
    renderer::RenderDevice,
    settings::WgpuFeatures,
};
use bevy_render::{
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    render_graph::{RenderGraphApp, ViewNodeRunner},
    render_resource::{Shader, TextureFormat, TextureUsages},
    renderer::RenderAdapter,
    Render, RenderApp, RenderSet,
};

use bevy_core_pipeline::core_3d::{graph::Core3d, Camera3d};
use resources::{
    prepare_atmosphere_transforms, queue_render_sky_pipelines, AtmosphereTransforms,
    RenderSkyBindGroupLayouts,
};
use tracing::warn;

use crate::{light_consts::lux, DirectionalLight};

use self::{
    node::{AtmosphereLutsNode, AtmosphereNode, RenderSkyNode},
    resources::{
        prepare_atmosphere_bind_groups, prepare_atmosphere_textures, AtmosphereBindGroupLayouts,
        AtmosphereLutPipelines, AtmosphereSamplers,
    },
};

mod shaders {
    use bevy_asset::{weak_handle, Handle};
    use bevy_render::render_resource::Shader;

    pub const TYPES: Handle<Shader> = weak_handle!("ef7e147e-30a0-4513-bae3-ddde2a6c20c5");
    pub const FUNCTIONS: Handle<Shader> = weak_handle!("7ff93872-2ee9-4598-9f88-68b02fef605f");
    pub const BRUNETON_FUNCTIONS: Handle<Shader> =
        weak_handle!("e2dccbb0-7322-444a-983b-e74d0a08bcda");
    pub const BINDINGS: Handle<Shader> = weak_handle!("bcc55ce5-0fc4-451e-8393-1b9efd2612c4");

    pub const TRANSMITTANCE_LUT: Handle<Shader> =
        weak_handle!("a4187282-8cb1-42d3-889c-cbbfb6044183");
    pub const MULTISCATTERING_LUT: Handle<Shader> =
        weak_handle!("bde3a71a-73e9-49fe-a379-a81940c67a1e");
    pub const SKY_VIEW_LUT: Handle<Shader> = weak_handle!("f87e007a-bf4b-4f99-9ef0-ac21d369f0e5");
    pub const AERIAL_VIEW_LUT: Handle<Shader> =
        weak_handle!("a3daf030-4b64-49ae-a6a7-354489597cbe");
    pub const RENDER_SKY: Handle<Shader> = weak_handle!("09422f46-d0f7-41c1-be24-121c17d6e834");
}

#[doc(hidden)]
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, shaders::TYPES, "types.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, shaders::FUNCTIONS, "functions.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            shaders::BRUNETON_FUNCTIONS,
            "bruneton_functions.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(app, shaders::BINDINGS, "bindings.wgsl", Shader::from_wgsl);

        load_internal_asset!(
            app,
            shaders::TRANSMITTANCE_LUT,
            "transmittance_lut.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            shaders::MULTISCATTERING_LUT,
            "multiscattering_lut.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            shaders::SKY_VIEW_LUT,
            "sky_view_lut.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            shaders::AERIAL_VIEW_LUT,
            "aerial_view_lut.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            shaders::RENDER_SKY,
            "render_sky.wgsl",
            Shader::from_wgsl
        );

        app.init_asset::<ScatteringProfile>();
        app.world()
            .resource_mut::<Assets<ScatteringProfile>>()
            .insert(&ScatteringProfile::EARTH_HANDLE, ScatteringProfile::EARTH);

        app.register_type::<ScatteringProfile>()
            .register_type::<AtmosphereAuxLutSettings>()
            .add_plugins((
                ExtractComponentPlugin::<AtmosphereAuxLutSettings>::default(),
                UniformComponentPlugin::<AtmosphereAuxLutSettings>::default(),
            ));
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        let render_adapter = render_app.world().resource::<RenderAdapter>();
        let render_device = render_app.world().resource::<RenderDevice>();

        if !render_device
            .features()
            .contains(WgpuFeatures::DUAL_SOURCE_BLENDING)
        {
            warn!("AtmospherePlugin not loaded. GPU lacks support for dual-source blending.");
            return;
        }

        if !render_adapter
            .get_downlevel_capabilities()
            .flags
            .contains(DownlevelFlags::COMPUTE_SHADERS)
        {
            warn!("AtmospherePlugin not loaded. GPU lacks support for compute shaders.");
            return;
        }

        if !render_adapter
            .get_texture_format_features(TextureFormat::Rgba16Float)
            .allowed_usages
            .contains(TextureUsages::STORAGE_BINDING)
        {
            warn!("AtmospherePlugin not loaded. GPU lacks support: TextureFormat::Rgba16Float does not support TextureUsages::STORAGE_BINDING.");
            return;
        }

        render_app
            .init_resource::<AtmosphereBindGroupLayouts>()
            .init_resource::<RenderSkyBindGroupLayouts>()
            .init_resource::<AtmosphereSamplers>()
            .init_resource::<AtmosphereLutPipelines>()
            .init_resource::<AtmosphereTransforms>()
            .init_resource::<SceneUnits>()
            .init_resource::<SpecializedRenderPipelines<RenderSkyBindGroupLayouts>>()
            .add_systems(
                Render,
                (
                    configure_camera_depth_usages.in_set(RenderSet::ManageViews),
                    queue_render_sky_pipelines.in_set(RenderSet::Queue),
                    prepare_atmosphere_textures.in_set(RenderSet::PrepareResources),
                    prepare_atmosphere_transforms.in_set(RenderSet::PrepareResources),
                    prepare_atmosphere_bind_groups.in_set(RenderSet::PrepareBindGroups),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<AtmosphereLutsNode>>(
                Core3d,
                AtmosphereNode::RenderLuts,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    // END_PRE_PASSES -> RENDER_LUTS -> MAIN_PASS
                    Node3d::EndPrepasses,
                    AtmosphereNode::RenderLuts,
                    Node3d::StartMainPass,
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<RenderSkyNode>>(
                Core3d,
                AtmosphereNode::RenderSky,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::MainOpaquePass,
                    AtmosphereNode::RenderSky,
                    Node3d::MainTransparentPass,
                ),
            );
    }
}

#[derive(Copy, Clone, Debug, Resource)]
pub struct SceneUnits {
    pub to_meters: f32,
}

impl SceneUnits {
    pub const METERS: Self = Self { to_meters: 1.0 };
    pub const CENTIMETERS: Self = Self { to_meters: 1.0e+2 };
    pub const MILLIMETERS: Self = Self { to_meters: 1.0e+3 };
    pub const KILOMETERS: Self = Self { to_meters: 1.0e-3 };
}

impl Default for SceneUnits {
    fn default() -> Self {
        Self::METERS
    }
}

#[derive(Component)]
pub struct Planet {
    /// Radius of the planet
    ///
    /// units: m
    pub radius: f32,

    /// Altitude at which we consider the atmosphere to 'end' for our
    /// calculations
    ///
    /// units: m
    pub space_altitude: f32,

    /// An approximation of the average albedo (or color, roughly) of the
    /// planet's surface. This is used when calculating multiscattering.
    /// The alpha channel is ignored.
    ///
    /// units: N/A
    pub ground_albedo: LinearRgba,
}

impl Planet {
    pub const EARTH: Self = Self {
        radius: 6_360_000.0,
        space_altitude: 100_000.0,
        ground_albedo: LinearRgba::rgb(0.3, 0.3, 0.3),
    };
}

impl Default for Planet {
    fn default() -> Self {
        Self::EARTH
    }
}

#[derive(ShaderType)]
struct GpuPlanet {
    ground_albedo: Vec3,
    lower_radius: f32,
    upper_radius: f32,
}

impl From<Planet> for GpuPlanet {
    fn from(planet: Planet) -> Self {
        Self {
            ground_albedo: planet.ground_albedo.to_vec3(),
            lower_radius: planet.radius,
            upper_radius: planet.radius + planet.space_altitude,
        }
    }
}

/// An asset that describes how a volume scatters light.
///
/// Most atmospheric particles scatter and absorb light in two main ways:
///
/// Rayleigh scattering occurs among very small particles, like individual gas
/// molecules. It's wavelength dependent, and causes colors to separate out as
/// light travels through the atmosphere. These particles *don't* absorb light.
///
/// Mie scattering occurs among slightly larger particles, like dust and sea spray.
/// These particles *do* absorb light, but Mie scattering and absorption is
/// *wavelength independent*.
///
/// Ozone acts differently from the other two, and is special-cased because
/// it's very important to the look of Earth's atmosphere. It's wavelength
/// dependent, but only *absorbs* light. Also, while the density of particles
/// participating in Rayleigh and Mie scattering falls off roughly exponentially
/// from the planet's surface, ozone only exists in a band centered at a fairly
/// high altitude.
#[derive(Clone, Reflect, ShaderType, Asset)]
pub struct ScatteringProfile {
    /// The rate of falloff of rayleigh particulate with respect to altitude:
    /// optical density = exp(-rayleigh_density_exp_scale * altitude in meters).
    ///
    /// THIS VALUE MUST BE POSITIVE
    ///
    /// units: N/A
    pub rayleigh_density_exp_scale: f32,

    /// The scattering optical density of rayleigh particulate, or how
    /// much light it scatters per meter
    ///
    /// units: m^-1
    pub rayleigh_scattering: Vec3,

    /// The rate of falloff of mie particulate with respect to altitude:
    /// optical density = exp(-mie_density_exp_scale * altitude in meters)
    ///
    /// THIS VALUE MUST BE POSITIVE
    ///
    /// units: N/A
    pub mie_density_exp_scale: f32,

    /// The scattering optical density of mie particulate, or how much light
    /// it scatters per meter.
    ///
    /// units: m^-1
    pub mie_scattering: f32,

    /// The absorbing optical density of mie particulate, or how much light
    /// it absorbs per meter.
    ///
    /// units: m^-1
    pub mie_absorption: f32,

    /// The "asymmetry" of mie scattering, or how much light tends to scatter
    /// forwards, rather than backwards or to the side.
    ///
    /// domain: (-1, 1)
    /// units: N/A
    pub mie_asymmetry: f32, //the "asymmetry" value of the phase function, unitless. Domain: (-1, 1)

    /// The altitude at which the ozone layer is centered.
    ///
    /// units: m
    pub ozone_layer_altitude: f32,

    /// The width of the ozone layer
    ///
    /// units: m
    pub ozone_layer_width: f32,

    /// The optical density of ozone, or how much of each wavelength of
    /// light it absorbs per meter.
    ///
    /// units: m^-1
    pub ozone_absorption: Vec3,
}

impl ScatteringProfile {
    const EARTH_HANDLE: Handle<Self> = Handle::weak_from_u128(0x7A9B1D4114306F28C5B8A8DB5D555686);

    pub const EARTH: Self = Self {
        rayleigh_density_exp_scale: 1.0 / 8_000.0,
        rayleigh_scattering: Vec3::new(5.802e-6, 13.558e-6, 33.100e-6),
        mie_density_exp_scale: 1.0 / 1_200.0,
        mie_scattering: 3.996e-6,
        mie_absorption: 0.444e-6,
        mie_asymmetry: 0.8,
        ozone_layer_altitude: 25_000.0,
        ozone_layer_width: 30_000.0,
        ozone_absorption: Vec3::new(0.650e-6, 1.881e-6, 0.085e-6),
    };

    pub fn earth() -> Handle<Self> {
        Self::EARTH_HANDLE.clone()
    }

    pub fn with_density_multiplier(mut self, mult: f32) -> Self {
        self.rayleigh_scattering *= mult;
        self.mie_scattering *= mult;
        self.mie_absorption *= mult;
        self.ozone_absorption *= mult;
        self
    }
}

#[derive(Component)]
#[require(AtmosphereCoreLutSettings)]
pub struct Atmosphere(pub Handle<ScatteringProfile>);

impl Default for Atmosphere {
    fn default() -> Self {
        Self::earth()
    }
}

impl Atmosphere {
    pub fn earth() -> Self {
        Self(ScatteringProfile::earth())
    }
}

#[derive(Component, Default)]
pub enum AtmosphericScattering {
    LutBased(AtmosphereAuxLutSettings),
    RayMarched(AtmosphereRayMarchSettings),
}

impl Default for AtmosphericScattering {
    fn default() -> Self {
        Self::LutBased(Default::default())
    }
}

#[derive(Clone, Reflect, Component, ShaderType)]
pub struct AtmosphereCoreLutSettings {
    /// The size of the transmittance LUT
    pub transmittance_lut_size: UVec2,

    /// The size of the multiscattering LUT
    pub multiscattering_lut_size: UVec2,

    /// The number of points to sample along each ray when
    /// computing the transmittance LUT
    pub transmittance_lut_samples: u32,

    /// The number of rays to sample when computing each
    /// pixel of the multiscattering LUT
    pub multiscattering_lut_dirs: u32,

    /// The number of points to sample along each ray when
    /// computing the multiscattering LUT.
    pub multiscattering_lut_samples: u32,
}

impl Default for AtmosphereCoreLutSettings {
    fn default() -> Self {
        Self {
            transmittance_lut_size: UVec2::new(256, 128),
            multiscattering_lut_size: UVec2::new(32, 32),
            transmittance_lut_samples: 40,
            multiscattering_lut_dirs: 64,
            multiscattering_lut_samples: 20,
        }
    }
}

/// This component controls the resolution of the atmosphere LUTs, and
/// how many samples are used when computing them.
///
/// The transmittance LUT stores the transmittance from a point in the
/// atmosphere to the outer edge of the atmosphere in any direction,
/// parametrized by the point's radius and the cosine of the zenith angle
/// of the ray.
///
/// The multiscattering LUT stores the factor representing luminance scattered
/// towards the camera with scattering order >2, parametrized by the point's radius
/// and the cosine of the zenith angle of the sun.
///
/// The sky-view lut is essentially the actual skybox, storing the light scattered
/// towards the camera in every direction with a cubemap.
///
/// The aerial-view lut is a 3d LUT fit to the view frustum, which stores the luminance
/// scattered towards the camera at each point (RGB channels), alongside the average
/// transmittance to that point (A channel).
#[derive(Clone, Reflect, ShaderType)]
pub struct AtmosphereAuxLutSettings {
    /// The size of the sky-view LUT.
    pub sky_view_lut_size: UVec2,

    /// The size of the aerial-view LUT.
    pub aerial_view_lut_size: UVec3,

    /// The number of points to sample along each ray when
    /// computing the sky-view LUT.
    pub sky_view_lut_samples: u32,

    /// The number of points to sample for each slice along the z-axis
    /// of the aerial-view LUT.
    pub aerial_view_lut_samples: u32,

    /// The maximum distance from the camera to evaluate the
    /// aerial view LUT. The slices along the z-axis of the
    /// texture will be distributed linearly from the camera
    /// to this value.
    ///
    /// units: m
    pub aerial_view_lut_max_distance: f32,
}

impl Default for AtmosphereAuxLutSettings {
    fn default() -> Self {
        Self {
            sky_view_lut_size: UVec2::new(400, 200),
            sky_view_lut_samples: 16,
            aerial_view_lut_size: UVec3::new(32, 32, 32),
            aerial_view_lut_samples: 10,
            aerial_view_lut_max_distance: 3.2e4,
        }
    }
}

#[derive(Clone, Reflect, ShaderType)]
pub struct AtmosphereRayMarchSettings {
    sample_count: u32,
    jitter_strength: f32,
}

//TODO: find good values
impl Default for AtmosphereRayMarchSettings {
    fn default() -> Self {
        Self {
            sample_count: 16,
            jitter_strength: 0.2,
        }
    }
}

#[derive(Component)]
#[require(DirectionalLight(Self::default_light))]
pub struct Sun {
    /// The angular size (or diameter) of the sun when viewed from the surface of a planet.
    angular_size: f32,
}

impl Sun {
    pub const SOL: Self = Self {
        angular_size: 0.0174533,
    };

    pub fn default_light() -> DirectionalLight {
        DirectionalLight {
            color: Color::WHITE,
            illuminance: lux::RAW_SUNLIGHT,
            ..Default::default()
        }
    }
}

impl Default for Sun {
    fn default() -> Self {
        Self::SOL
    }
}
