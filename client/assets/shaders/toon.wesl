// Ombrage « toon » comme celui de sprite_forge: lumière par paliers, hachures dans l'ombre moyenne, reflet net, contour sombre.
// `data.color` = couleur sRGB de la pièce; `data.params` = (lumineuse, force du reflet, opacité).
import bevy_pbr::{
    render::forward_io::VertexOutput,
    render::mesh_view_bindings::view,
};

struct Toon {
    color: vec4<f32>,
    params: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> toon: Toon;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let light = normalize(vec3<f32>(-0.45, 0.7, 0.55));
    let ndl = dot(n, light);
    // Hachures: damier diagonal d'un pixel sur deux, comme la trame du rendu des planches.
    let cell = floor(in.position.xy / 2.0);
    let hatch = (u32(cell.x + cell.y) % 2u) == 0u;
    var tone = 1.08;
    if ndl <= 0.5 { tone = 0.9; }
    if ndl <= 0.1 { tone = select(0.78, 0.7, hatch); }
    if ndl <= -0.25 { tone = select(0.6, 0.5, hatch); }
    if toon.params.x > 0.5 { tone = 1.15; }
    var rgb = toon.color.rgb * tone;
    // Reflet blanc net là où la normale fait face à la lumière.
    let half_vec = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
    if toon.params.y > 0.0 && toon.params.x < 0.5 && dot(n, half_vec) > 0.93 {
        rgb = rgb + (vec3<f32>(1.0) - rgb) * toon.params.y;
    }
    // Contour: là où la surface fuit vers l'arrière (vue orthographique, regard le long de l'axe de la caméra).
    let view_dir = normalize((view.world_from_view * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
    let facing = abs(dot(n, view_dir));
    if toon.params.x < 0.5 && facing < 0.2 {
        rgb = rgb * 0.35;
    }
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    // Les couleurs de sprite_forge sont déjà en sRGB; la cible attend du linéaire.
    return vec4<f32>(pow(rgb, vec3<f32>(2.2)), toon.params.z);
}
