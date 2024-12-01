#define_import_path bevy_pbr::atmosphere::functions

#import bevy_pbr::atmosphere::{
    types::Atmosphere,
    bindings::{
        atmosphere, settings, view, lights, transmittance_lut, transmittance_lut_sampler, 
        multiscattering_lut, multiscattering_lut_sampler, sky_view_lut, sky_view_lut_sampler,
        aerial_view_lut, aerial_view_lut_sampler, atmosphere_transforms
    },
    bruneton_functions::{
        transmittance_lut_r_mu_to_uv, transmittance_lut_uv_to_r_mu, 
        ray_intersects_ground, distance_to_top_atmosphere_boundary, 
        distance_to_bottom_atmosphere_boundary
    },
}

// NOTE FOR CONVENTIONS: 
// r:
//   radius, or distance from planet center 
//
// altitude:
//   distance from planet **surface**
//
// mu:
//   cosine of the zenith angle of a ray with
//   respect to the planet normal
//
// atmosphere space: 
//   abbreviated as "as" (contrast with vs, cs, ws), this space is similar
//   to view space, but with the camera positioned horizontally on the planet
//   surface, so the horizon is a horizontal line centered vertically in the
//   frame. This enables the non-linear latitude parametrization the paper uses 
//   to concentrate detail near the horizon 


// CONSTANTS

const PI: f32 = 3.141592653589793238462;
const TAU: f32 = 6.283185307179586476925;
const FRAC_PI: f32 = 0.31830988618379067153; // 1 / π
const FRAC_3_16_PI: f32 = 0.0596831036594607509; // 3 / (16π)
const FRAC_4_PI: f32 = 0.07957747154594767; // 1 / (4π)

// LUT UV PARAMATERIZATIONS

fn multiscattering_lut_r_mu_to_uv(r: f32, mu: f32) -> vec2<f32> {
    let u = 0.5 + 0.5 * mu;
    let v = saturate((r - atmosphere.bottom_radius) / (atmosphere.top_radius - atmosphere.bottom_radius)); //TODO
    return vec2(u, v);
}

fn multiscattering_lut_uv_to_r_mu(uv: vec2<f32>) -> vec2<f32> {
    let r = mix(atmosphere.bottom_radius, atmosphere.top_radius, uv.y);
    let mu = uv.x * 2 - 1;
    return vec2(r, mu);
}

// We squash the ray direction towards the horizon in order to emulate the paper's
// non-linear latitude parametrization, though we use a cubemap instead.

fn sky_view_lut_squash_ray_dir(ray_dir_as: vec3<f32>) -> vec3<f32> {
    let new_y = sqrt(abs(ray_dir_as.y)) * sign(ray_dir_as.y);
    return normalize(vec3(ray_dir_as.x, new_y, ray_dir_as.z));
}

fn sky_view_lut_unsquash_ray_dir(ray_dir_as: vec3<f32>) -> vec3<f32> {
    let abs_y = abs(ray_dir_as.y);
    let new_y = abs_y * abs_y * sign(ray_dir_as.y);
    return normalize(vec3(ray_dir_as.x, new_y, ray_dir_as.z));
}


// LUT SAMPLING

fn sample_transmittance_lut(r: f32, mu: f32) -> vec3<f32> {
    let uv = transmittance_lut_r_mu_to_uv(r, mu);
    return textureSampleLevel(transmittance_lut, transmittance_lut_sampler, uv, 0.0).rgb;
}

fn sample_multiscattering_lut(r: f32, mu: f32) -> vec3<f32> {
    let uv = multiscattering_lut_r_mu_to_uv(r, mu);
    return textureSampleLevel(multiscattering_lut, multiscattering_lut_sampler, uv, 0.0).rgb;
}

fn sample_sky_view_lut(ray_dir_as: vec3<f32>) -> vec3<f32> {
    let ray_dir_as_squashed = sky_view_lut_squash_ray_dir(ray_dir_as);
    return textureSampleLevel(sky_view_lut, sky_view_lut_sampler, ray_dir_as_squashed, 0.0).rgb;
}

//RGB channels: total inscattered light along the camera ray to the current sample.
//A channel: average transmittance across all wavelengths to the current sample.
fn sample_aerial_view_lut(ndc: vec3<f32>) -> vec4<f32> {
    return textureSampleLevel(aerial_view_lut, aerial_view_lut_sampler, vec3(ndc_to_uv(ndc.xy), ndc.z), 0.0);
}

// PHASE FUNCTIONS

fn rayleigh(neg_LdotV: f32) -> f32 {
    return FRAC_3_16_PI * (1 + (neg_LdotV * neg_LdotV));
}

fn henyey_greenstein(neg_LdotV: f32) -> f32 {
    let g = atmosphere.mie_asymmetry;
    let denom = 1.0 + g * g - 2.0 * g * neg_LdotV;
    return FRAC_4_PI * (1.0 - g * g) / (denom * sqrt(denom));
}

// ATMOSPHERE SAMPLING

struct AtmosphereSample {
    /// units: km^-1
    rayleigh_scattering: vec3<f32>,

    /// units: km^-1
    mie_scattering: f32,

    /// the sum of scattering and absorption. Since the phase function doesn't
    /// matter for this, we combine rayleigh and mie extinction to a single 
    //  value.
    //
    /// units: km^-1
    extinction: vec3<f32>
}

/// Samples atmosphere optical densities at a given radius
fn sample_atmosphere(r: f32) -> AtmosphereSample {
    let altitude = r - atmosphere.bottom_radius;

    // atmosphere values at altitude
    let mie_density = exp(-atmosphere.mie_density_exp_scale * altitude);
    let rayleigh_density = exp(-atmosphere.rayleigh_density_exp_scale * altitude);
    var ozone_density: f32 = max(0.0, 1.0 - (abs(altitude - atmosphere.ozone_layer_altitude) / (atmosphere.ozone_layer_width * 0.5)));

    let mie_scattering = mie_density * atmosphere.mie_scattering;
    let mie_absorption = mie_density * atmosphere.mie_absorption;
    let mie_extinction = mie_scattering + mie_absorption;

    let rayleigh_scattering = rayleigh_density * atmosphere.rayleigh_scattering;
    // no rayleigh absorption
    // rayleigh extinction is the sum of scattering and absorption

    // ozone doesn't contribute to scattering
    let ozone_absorption = ozone_density * atmosphere.ozone_absorption;

    var sample: AtmosphereSample;
    sample.rayleigh_scattering = rayleigh_scattering;
    sample.mie_scattering = mie_scattering;
    sample.extinction = rayleigh_scattering + mie_extinction + ozone_absorption;

    return sample;
}


/// evaluates equation 3 in the paper, called L_scat, which gives the total single-order scattering towards the view at a single point
fn sample_local_inscattering(local_atmosphere: AtmosphereSample, transmittance_to_sample: vec3<f32>, ray_dir: vec3<f32>, local_r: f32, local_up: vec3<f32>) -> vec3<f32> {
    var rayleigh_scattering = vec3(0.0);
    var mie_scattering = vec3(0.0);
    for (var light_i: u32 = 0u; light_i < lights.n_directional_lights; light_i++) {
        let light = &lights.directional_lights[light_i];

        let mu_light = dot((*light).direction_to_light, local_up);

        // -(L . V) == (L . -V). -V here is our ray direction, which points away from the view 
        // instead of towards it (as is the convention)
        let neg_LdotV = dot((*light).direction_to_light, ray_dir);

        // phase functions give the proportion of light
        // scattered towards the camera for each scattering type
        let rayleigh_phase = rayleigh(neg_LdotV);
        let mie_phase = henyey_greenstein(neg_LdotV);

        let transmittance_to_light = sample_transmittance_lut(local_r, mu_light);
        let shadow_factor = transmittance_to_light * f32(!ray_intersects_ground(local_r, mu_light));

        //Additive factor from the multiscattering LUT
        let psi_ms = sample_multiscattering_lut(local_r, mu_light);

        // Note transmittance_to_sample vs shadow_factor: 
        // A light ray travelling from the sun to the camera follows a
        // two-segment path (assuming single scattering). Transmittance_to_sample 
        // handles the transmittance between the view and the sample position, while 
        // the shadow factor handles the transmittance between the sample position and 
        // the light itself. We check if the ray intersects the ground for the shadow
        // factor *only*, because we assume our primary rays never go below ground.
        rayleigh_scattering += (transmittance_to_sample * shadow_factor * rayleigh_phase + psi_ms) * (*light).color.rgb;
        mie_scattering += (transmittance_to_sample * shadow_factor * mie_phase + psi_ms) * (*light).color.rgb;
    }
    return (local_atmosphere.rayleigh_scattering * rayleigh_scattering + local_atmosphere.mie_scattering * mie_scattering) * view.exposure;
}

//TODO: make pr to specify light angular size on struct itself
const SUN_ANGULAR_SIZE: f32 = 0.00436332; //angular radius of sun in radians

fn sample_sun_illuminance(ray_dir_ws: vec3<f32>, transmittance: vec3<f32>) -> vec3<f32> {
    var sun_illuminance = vec3(0.0);
    for (var light_i: u32 = 0u; light_i < lights.n_directional_lights; light_i++) {
        let light = &lights.directional_lights[light_i];
        let neg_LdotV = dot((*light).direction_to_light, ray_dir_ws);
        let angle_to_light = acos(neg_LdotV);
        sun_illuminance += (*light).color.rgb * f32(angle_to_light <= SUN_ANGULAR_SIZE);
    }
    return sun_illuminance * transmittance * view.exposure;
}

// TRANSFORM UTILITIES

fn max_atmosphere_distance(r: f32, mu: f32) -> f32 {
    let t_top = distance_to_top_atmosphere_boundary(r, mu);
    let t_bottom = distance_to_bottom_atmosphere_boundary(r, mu);
    let hits = ray_intersects_ground(r, mu);
    return mix(t_top, t_bottom, f32(hits));
}

/// Assuming y=0 is the planet ground, returns the view altitude in kilometers
fn view_radius() -> f32 {
    return view.world_position.y * settings.scene_units_to_km + atmosphere.bottom_radius;
}

// We assume the `up` vector at the view position is the y axis, since the world is locally flat/level.
// t = distance along view ray (km)
//NOTE: this means that if your world is actually spherical, this will be wrong.
fn get_local_up(r: f32, t: f32, ray_dir: vec3<f32>) -> vec3<f32> {
    return normalize(vec3(0.0, r, 0.0) + t * ray_dir);
}

// given a ray starting at radius r, with mu = cos(zenith angle),
// and a t = distance along the ray, gives the new radius at point t
fn get_local_r(r: f32, mu: f32, t: f32) -> f32 {
    return sqrt(t * t + 2.0 * r * mu * t + r * r);
}

// Convert uv [0.0 .. 1.0] coordinate to ndc space xy [-1.0 .. 1.0]
fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
}

/// Convert ndc space xy coordinate [-1.0 .. 1.0] to uv [0.0 .. 1.0]
fn ndc_to_uv(ndc: vec2<f32>) -> vec2<f32> {
    return ndc * vec2(0.5, -0.5) + vec2(0.5);
}

/// Convert a ndc space position to world space
fn position_ndc_to_world(ndc_pos: vec3<f32>) -> vec3<f32> {
    let world_pos = view.world_from_clip * vec4(ndc_pos, 1.0);
    return world_pos.xyz / world_pos.w;
}


/// Convert in 
fn direction_atmosphere_to_world(atmo_dir: vec3<f32>) -> vec3<f32> {
    let world_dir = atmosphere_transforms.world_from_atmosphere * vec4(atmo_dir, 0.0);
    return world_dir.xyz;
}

/// Convert a view space direction to world space
fn direction_view_to_world(view_dir: vec3<f32>) -> vec3<f32> {
    let world_dir = view.world_from_view * vec4(view_dir, 0.0);
    return world_dir.xyz;
}

/// Convert ndc depth to linear view z. 
/// Note: Depth values in front of the camera will be negative as -z is forward
fn depth_ndc_to_view_z(ndc_depth: f32) -> f32 {
    let view_pos = view.view_from_clip * vec4(0.0, 0.0, ndc_depth, 1.0);
    return view_pos.z / view_pos.w;
}