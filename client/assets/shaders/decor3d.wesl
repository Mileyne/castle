// Décor en 3D: la texture du bloc, éclairée par un soleil et par des lampes. Le sol plat garde exactement ses couleurs
// (ambiante + soleil = 1). Le soleil vient du nord-ouest: les faces des murs tournées vers la caméra n'en reçoivent pas, et les murs
// projettent leur ombre vers le sud-est, sur le sol devant eux.
import bevy_pbr::{
    render::forward_io::VertexOutput,
};

const MAX_LIGHTS: u32 = 8u;
const AMBIENT: f32 = 0.45;
// Direction du soleil (x, hauteur, z: le sud est positif).
const SUN: vec3<f32> = vec3<f32>(-0.45, 0.75, -0.5);
const SHADOW_STEPS: u32 = 6u;
const SHADOW_TONE: f32 = 0.55;

struct Decor {
    tint: vec4<f32>,
    // x: étirement du sol en profondeur; y: nombre de lampes; z: hauteur des murs; w: côté d'une tuile.
    params: vec4<f32>,
    // Largeur et hauteur de la salle en tuiles.
    room: vec4<f32>,
    light_pos: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var samp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> decor: Decor;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var walls: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var walls_samp: sampler;

fn sin_elevation() -> f32 {
    return 1.0 / decor.params.x;
}

// Y a-t-il un mur à ce point du monde 2D (x, y)? Hors de la salle, c'est un mur.
fn wall_at(p: vec2<f32>) -> bool {
    let t = p / decor.params.w + decor.room.xy * 0.5;
    if t.x < 0.0 || t.y < 0.0 || t.x >= decor.room.x || t.y >= decor.room.y {
        return true;
    }
    return textureSampleLevel(walls, walls_samp, t / decor.room.xy, 0.0).r > 0.5;
}

// Part de lumière du soleil qui atteint un point du sol: on remonte le rayon vers le soleil, et un mur le coupe tant que le rayon est
// plus bas que lui.
fn sunlight(ground: vec2<f32>) -> f32 {
    // Le sol à l'intérieur d'un mur n'est pas dans son ombre.
    if wall_at(ground) {
        return 1.0;
    }
    let toward = vec2<f32>(SUN.x, -SUN.z) / SUN.y;
    for (var i = 1u; i <= SHADOW_STEPS; i = i + 1u) {
        let height = decor.params.z * f32(i) / f32(SHADOW_STEPS);
        if wall_at(ground + toward * height) {
            return SHADOW_TONE;
        }
    }
    return 1.0;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(tex, samp, in.uv);
    if sample.a < 0.5 {
        discard;
    }
    let base = sample.rgb * decor.tint.rgb;
    let n = normalize(in.world_normal);
    let sun = normalize(SUN);
    // Position non étirée (x, hauteur, y du monde 2D).
    let here = vec3<f32>(in.world_position.x, in.world_position.y, -in.world_position.z * sin_elevation());
    // Le sol plat reçoit 0,75 du soleil: 0,45 + 0,733 x 0,75 = 1.
    var tone = vec3<f32>(AMBIENT + (1.0 - AMBIENT) / sun.y * max(dot(n, sun), 0.0));
    if n.y > 0.9 && here.y < 1.0 {
        // Le sol: l'ombre des murs.
        tone = tone * sunlight(here.xz);
    }
    if n.z > 0.5 {
        // Une face de mur: son art est déjà ombré; un peu plus sombre au pied qu'en haut.
        tone = vec3<f32>(0.95) * mix(0.75, 1.0, clamp(here.y / decor.params.z, 0.0, 1.0));
    }
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
