// Un sprite du jeu (héros, monstre, grand motif) dessiné dans la scène 3D du décor, pour que les murs puissent le cacher. `rect` est la
// zone de la texture à montrer (u0, v0, u1, v1; échangés pour un sprite retourné), `tint` la teinte du sprite.
import bevy_pbr::{
    render::forward_io::VertexOutput,
};

struct Billboard {
    rect: vec4<f32>,
    tint: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var samp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> billboard: Billboard;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = mix(billboard.rect.xy, billboard.rect.zw, in.uv);
    let color = textureSample(tex, samp, p) * billboard.tint;
    if color.a < 0.01 {
        discard;
    }
    return color;
}
