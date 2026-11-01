// Pluie: des traits fins qui tombent en biais, dessinés par le processeur graphique (aucune entité par goutte).
// `data` = (côté d'une cellule, vent, vitesse de chute, opacité). Le maillage fait 2400 x 1600 unités autour de la caméra.
import bevy_sprite_render::mesh2d::{
    vertex_output::VertexOutput,
    view_bindings::globals,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> data: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

// Une couche de pluie: une goutte par cellule d'une grille qui suit la chute.
fn layer(p: vec2<f32>, down: vec2<f32>, side: vec2<f32>, speed: f32, seed: f32) -> f32 {
    let cell = data.x;
    let a = dot(p, side);
    let b = dot(p, down) - speed * globals.time;
    let id = floor(vec2<f32>(a, b) / cell);
    let h1 = hash(id + seed);
    let h2 = hash(id + seed + 17.0);
    let h3 = hash(id + seed + 41.0);
    // Position de la goutte dans sa cellule (assez loin des bords pour que le trait ne déborde pas).
    let center = vec2<f32>((0.1 + 0.8 * h1) * cell, (0.2 + 0.6 * h2) * cell);
    let local = vec2<f32>(a, b) - id * cell - center;
    let across = 1.0 - smoothstep(0.17, 0.33, abs(local.x));
    let along = 1.0 - smoothstep(3.0, 3.5, abs(local.y));
    return across * along * (0.38 + 0.25 * h3);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<f32>((in.uv.x - 0.5) * 2400.0, (0.5 - in.uv.y) * 1600.0);
    let fall = vec2<f32>(-data.y, -data.z);
    let speed = length(fall);
    let down = fall / speed;
    // `side` est perpendiculaire à la chute.
    let side = vec2<f32>(-down.y, down.x);
    let rain = max(layer(p, down, side, speed * 0.85, 0.0), layer(p, down, side, speed * 1.15, 91.0));
    return vec4<f32>(0.72, 0.82, 1.0, rain * data.w);
}
