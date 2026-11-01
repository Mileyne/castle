// Brume posée sur le sol de la scène 3D: des nappes qui dérivent lentement, ancrées dans le monde (elles ne suivent pas la caméra), dessinées par le processeur
// graphique. `data` = (échelle: unités du monde par motif, vent x, vent y, densité maximale); `params.x` = nombre de couches de bruit; `params.y` = luminosité de la brume (elle n'émet pas de lumière: elle ne fait que renvoyer celle du décor).
import bevy_pbr::{
    render::{
        forward_io::VertexOutput,
        mesh_view_bindings::globals,
    },
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> data: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> params: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

// Bruit de valeur lissé.
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p: vec2<f32>, layers: i32) -> f32 {
    var sum = 0.0;
    var amplitude = 0.5;
    var q = p;
    for (var i = 0; i < layers; i = i + 1) {
        sum = sum + amplitude * noise(q);
        q = q * 2.03 + vec2<f32>(17.0, 9.0);
        amplitude = amplitude * 0.5;
    }
    return sum;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let layers = i32(params.x);
    let t = globals.time;
    // Position sur le sol dans le monde 2D (x vers la droite, y vers le nord = -z de la scène 3D).
    let world = vec2<f32>(in.world_position.x, -in.world_position.z) / data.x;
    // Deux nappes qui dérivent à des vitesses différentes: la brume se déforme au lieu de glisser d'un bloc.
    let a = fbm(world + vec2<f32>(data.y, data.z) * t, layers);
    let b = fbm(world * 0.6 + vec2<f32>(-data.y, data.z * 0.5) * t * 1.4 + vec2<f32>(31.0, 7.0), layers);
    let cloud = smoothstep(0.3, 0.8, a * 0.6 + b * 0.55);
    return vec4<f32>(vec3<f32>(0.80, 0.87, 0.88) * params.y, cloud * data.w);
}
