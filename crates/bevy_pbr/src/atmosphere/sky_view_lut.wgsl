#import bevy_pbr::{
    mesh_view_types::Lights,
    atmosphere::{
        types::{Atmosphere, AtmosphereSettings},
        bindings::{atmosphere, view, settings, lights},
        functions::{
            view_radius, max_atmosphere_distance, direction_atmosphere_to_world,
            sky_view_lut_uv_to_zenith_azimuth, zenith_azimuth_to_ray_dir,
            raymarch_atmosphere,
        },
    }
}

#import bevy_render::{
    view::View,
    maths::HALF_PI,
}
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(13) var sky_view_lut_out: texture_storage_2d<rgba16float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) idx: vec3<u32>) {
    let uv = vec2<f32>(idx.xy) / vec2<f32>(settings.sky_view_lut_size);

    let r = view_radius();
    var zenith_azimuth = sky_view_lut_uv_to_zenith_azimuth(r, uv);

    let ray_dir_as = zenith_azimuth_to_ray_dir(zenith_azimuth.x, zenith_azimuth.y);
    let ray_dir_ws = direction_atmosphere_to_world(ray_dir_as);

    let t_max = max_atmosphere_distance(r, ray_dir_ws.y);
    let sample_count = mix(1.0, f32(settings.sky_view_lut_samples), clamp(t_max * 0.01, 0.0, 1.0));
    
    let result = raymarch_atmosphere(r, ray_dir_ws, t_max, sample_count);
    textureStore(sky_view_lut_out, idx.xy, vec4(result.inscattering, 1.0));
}
