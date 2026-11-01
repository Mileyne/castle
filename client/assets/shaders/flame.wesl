// Une flamme de torche: un panneau tourné vers la caméra, où une forme de goutte est déformée par du bruit qui monte et qui s'agite.
// Le coeur est blanc-jaune, puis orange, puis rouge, et la pointe s'éteint. `params.x` est un décalage propre à chaque torche, pour qu'elles
// ne battent pas toutes ensemble. Le résultat s'ajoute à l'image (la flamme éclaire ce qu'elle recouvre, elle ne le cache pas).
import bevy_pbr::{
    render::{
        forward_io::VertexOutput,
        mesh_view_bindings::globals,
    },
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: vec4<f32>;

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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // x de -1 à 1 au travers de la flamme, h de 0 (la base) à 1 (la pointe).
    let x = (in.uv.x - 0.5) * 2.0;
    let h = 1.0 - in.uv.y;
    let t = globals.time * 1.0 + params.x;
    // Deux bruits qui montent à des vitesses différentes: l'un fait onduler la flamme d'un côté à l'autre, l'autre grignote ses bords.
    let sway = (noise(vec2<f32>(x * 1.3 + params.x * 3.0, h * 2.2 - t * 3.4)) - 0.5) * 0.9 * h;
    let bite = noise(vec2<f32>(x * 3.2 + 5.0, h * 4.6 - t * 6.2 + params.x));
    let flicker = 0.9 + 0.1 * sin(t * 17.0) + 0.05 * sin(t * 41.0 + params.x * 5.0);
    // Une goutte: large près de la base (mais pas jusqu'au bord), et qui s'effile vers la pointe.
    let width = mix(0.72, 0.05, pow(h, 1.15)) * flicker;
    let across = 1.0 - abs(x + sway) / max(width, 0.001);
    let body = across - bite * 0.42 * h - 0.04;
    var alpha = smoothstep(0.0, 0.32, body);
    // La base s'éteint doucement dans le support, la pointe dans le vide.
    alpha = alpha * smoothstep(0.0, 0.08, h) * (1.0 - smoothstep(0.7, 1.0, h));
    // La chaleur: forte au centre et en bas.
    let heat = clamp(body * 1.5 + (1.0 - h) * 0.7 - 0.25, 0.0, 1.0);
    var color = mix(vec3<f32>(0.80, 0.07, 0.01), vec3<f32>(1.0, 0.50, 0.07), smoothstep(0.05, 0.5, heat));
    color = mix(color, vec3<f32>(1.0, 0.93, 0.58), smoothstep(0.55, 0.95, heat));
    // Le mélange additif de Bevy ajoute la couleur telle quelle: on la donne déjà multipliée par l'opacité.
    return vec4<f32>(color * 1.25 * alpha, alpha);
}
