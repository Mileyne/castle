// Décor en 3D, éclairé par un soleil et par des lampes. Deux matières:
//  - le sol: la texture du bloc d'image (le sol plat garde exactement ses couleurs: ambiante + soleil = 1);
//  - les murs (`decor.style.x` = 1 pierre, 2 feuillage): une matière calculée ici, à partir de la position dans le monde, pour que la
//    face et le dessus d'un même mur s'accordent. Le dessin est quantifié au pixel de l'art (une tuile = `room.z` pixels).
// Le soleil vient du nord-ouest: les murs projettent leur ombre vers le sud-est, sur le sol devant eux.
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
    // Largeur et hauteur de la salle en tuiles; pixels d'une tuile dans l'art.
    room: vec4<f32>,
    style: vec4<f32>,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    color_c: vec4<f32>,
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

// Taille d'un pixel de l'art, en unités du monde.
fn pixel() -> f32 {
    return decor.params.w / decor.room.z;
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

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

// Bruit de valeur lissé.
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

// Briques en appareil décalé: `p` en unités du monde (alignées sur les tuiles), `course` hauteur d'un rang, `width` largeur d'une
// brique. Joint d'un pixel, arête haute éclairée, flanc droit dans l'ombre, grain.
fn bricks(p: vec2<f32>, course: f32, width: f32, seed: f32) -> vec3<f32> {
    let px = pixel();
    let q = floor(p / px) * px;
    let row = floor(q.y / course);
    let shifted = q.x + select(0.0, width * 0.5, (i32(row) & 1) != 0);
    let col = floor(shifted / width);
    let lx = shifted - col * width;
    let ly = q.y - row * course;
    var color = mix(decor.color_b.rgb, decor.color_a.rgb, hash21(vec2<f32>(col, row) + seed));
    color = color * (0.93 + 0.14 * hash21(q / px + seed * 7.0));
    if ly >= course - px {
        color = color * 1.18;
    }
    if lx >= width - px {
        color = color * 0.84;
    }
    if lx < px || ly < px {
        color = decor.color_c.rgb;
    }
    return color;
}

// Feuillage: des touffes en deux échelles, avec des reflets sur les plus claires.
fn leaves(p: vec2<f32>) -> vec3<f32> {
    let px = pixel();
    let q = floor(p / px) * px;
    let n = vnoise(q / 5.0) * 0.6 + vnoise(q / 2.3 + vec2<f32>(11.0, 5.0)) * 0.4;
    var color = mix(decor.color_b.rgb, decor.color_a.rgb, smoothstep(0.32, 0.68, n));
    if n > 0.74 {
        color = color * 1.28;
    }
    return color * (0.94 + 0.12 * hash21(q / px));
}

// Le sol de tuile: les coordonnées (x du monde décalé pour tomber sur les bords de tuile, y).
fn tile_origin() -> vec2<f32> {
    return decor.room.xy * 0.5 * decor.params.w;
}

// La face d'un mur (au sud): `fx` position le long du mur, `h` hauteur.
fn wall_face(fx: f32, h: f32) -> vec3<f32> {
    if decor.style.x < 1.5 {
        // Pierre: rangs de briques de 8 unités, de la largeur d'une demi-tuile.
        return bricks(vec2<f32>(fx, h), 8.0, 12.0, 3.0);
    }
    // Arbres: des troncs au pied, puis le feuillage, qui est aussi celui du dessus.
    let px = pixel();
    let tile = decor.params.w;
    let column = floor(fx / tile);
    let across = fx - column * tile;
    let center = tile * 0.5 + (hash21(vec2<f32>(column, 1.0)) - 0.5) * 5.0;
    let canopy = decor.params.z * 0.55 + 5.0 * vnoise(vec2<f32>(fx / 7.0, 3.0));
    if h >= canopy {
        return leaves(vec2<f32>(fx, h));
    }
    if abs(across - center) < 2.8 + 0.7 * hash21(vec2<f32>(column, 2.0)) {
        let q = floor(fx / px);
        let bark = decor.color_c.rgb * (0.78 + 0.34 * hash21(vec2<f32>(q, 5.0)));
        return bark * mix(0.55, 1.0, clamp(h / canopy, 0.0, 1.0));
    }
    // Entre les troncs: l'ombre sous les branches.
    return decor.color_b.rgb * 0.5;
}

// Le dessus d'un mur: `p` position sur le sol (x, y du monde 2D).
fn wall_top(p: vec2<f32>) -> vec3<f32> {
    if decor.style.x < 1.5 {
        // Pierre: dalles d'une tuile, de la même pierre que la face.
        var color = bricks(p + tile_origin(), decor.params.w, decor.params.w, 9.0);
        // Le rebord de devant (côté sud, quand le sol est devant) reprend la lumière de l'arête haute de la face.
        let t = p / decor.params.w + decor.room.xy * 0.5;
        let south_edge = fract(t.y) * decor.params.w;
        if south_edge < 2.0 * pixel() && !wall_at(p - vec2<f32>(0.0, decor.params.w)) {
            color = color * 1.22;
        }
        return color;
    }
    return leaves(p + tile_origin());
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let sun = normalize(SUN);
    // Position non étirée (x, hauteur, y du monde 2D).
    let here = vec3<f32>(in.world_position.x, in.world_position.y, -in.world_position.z * sin_elevation());
    var base = vec3<f32>(0.0);
    var tone = vec3<f32>(AMBIENT + (1.0 - AMBIENT) / sun.y * max(dot(n, sun), 0.0));
    if decor.style.x > 0.5 {
        // Un mur.
        if n.z > 0.5 {
            base = wall_face(here.x + tile_origin().x, here.y);
            // La face tourne le dos au soleil: une lumière de remplissage, et plus sombre au pied qu'en haut.
            tone = vec3<f32>(0.8) * mix(0.65, 1.0, clamp(here.y / decor.params.z, 0.0, 1.0));
        } else {
            base = wall_top(here.xz);
        }
        base = base * decor.tint.rgb;
    } else {
        let sample = textureSample(tex, samp, in.uv);
        if sample.a < 0.5 {
            discard;
        }
        base = sample.rgb * decor.tint.rgb;
        if n.y > 0.9 && here.y < 1.0 {
            // Le sol: l'ombre des murs.
            tone = tone * sunlight(here.xz);
        }
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
