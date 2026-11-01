// Décor en 3D: la texture du bloc, éclairée par un soleil et par des lampes. Le sol plat garde exactement ses couleurs
// (ambiante + soleil = 1), les faces tournées vers le sud sont plus sombres.
import bevy_pbr::{
    render::forward_io::VertexOutput,
};

const MAX_LIGHTS: u32 = 8u;

struct Decor {
    tint: vec4<f32>,
    // x: étirement du sol en profondeur; y: nombre de lampes.
    params: vec4<f32>,
    light_pos: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var samp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> decor: Decor;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(tex, samp, in.uv).rgb * decor.tint.rgb;
    let n = normalize(in.world_normal);
    let sun = normalize(vec3<f32>(-0.4, 0.8, 0.45));
    // Le sol plat reçoit 0,8 du soleil: 0,45 + 0,69 x 0,8 = 1.
    var tone = vec3<f32>(0.45 + 0.69 * max(dot(n, sun), 0.0));
    // Position non étirée (x, hauteur, y du monde 2D).
    let here = vec3<f32>(in.world_position.x, in.world_position.y, -in.world_position.z * sin_elevation());
    let count = u32(decor.params.y);
    for (var i = 0u; i < count && i < MAX_LIGHTS; i = i + 1u) {
        let lamp = decor.light_pos[i];
        let to_lamp = vec3<f32>(lamp.x, lamp.y, lamp.z) - here;
        let dist = length(to_lamp);
        let falloff = clamp(1.0 - dist / lamp.w, 0.0, 1.0);
        // Normale dans le même repère (x, hauteur, y du monde 2D): le sud devient -y.
        let facing = max(dot(vec3<f32>(n.x, n.y, -n.z), to_lamp / max(dist, 0.001)), 0.0);
        tone = tone + decor.light_color[i].rgb * falloff * falloff * facing * 0.9;
    }
    return vec4<f32>(base * tone, 1.0);
}

fn sin_elevation() -> f32 {
    return 1.0 / decor.params.x;
}
