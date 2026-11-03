// L'eau d'un niveau: une surface plate dessinée tuile par tuile, dont tout l'aspect vient du shader. Des vagues (somme de sinus et de bruit) en
// donnent la normale, qui sert à un reflet du ciel (de plus en plus fort quand on la voit en rase-motte, comme à la première personne), à des
// éclats du soleil, à des lignes claires qui courent dessus et à des étincelles; plus près des rives elle est plus claire et bordée d'écume.
// Les rives de chaque tuile (ouest, est, nord, sud) sont codées dans ses coordonnées de texture: `uv = (u + 2 * ouest_est, v + 2 * nord_sud)`,
// où `ouest_est` vaut 1 (rive à l'ouest) + 2 (rive à l'est) et `nord_sud` 1 (nord) + 2 (sud). 
import bevy_pbr::{
    render::{
        forward_io::VertexOutput,
        mesh_view_bindings::{globals, view},
    },
};

// Les mêmes réglages de lumière que le sol (voir `decor3d.wesl`): la teinte du niveau, l'ambiance et le soleil, et les lampes (lanternes des
// héros, torches). L'eau ne brille pas d'elle-même: elle est éclairée comme tout le reste, donc sombre dans le noir.
struct Water {
    tint: vec4<f32>,
    // x: nombre de lampes; z: ambiance du niveau; w: soleil.
    params: vec4<f32>,
    light_pos: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> water: Water;

const TILE: f32 = 16.0;
// Les pixels de l'art du sol: 48 par tuile de 16 unités, soit trois par unité. L'eau est dessinée par pixels de cette taille, pour être
// raccord avec le sol, les murs et les ponts.
const PIXELS_PER_UNIT: f32 = 3.0;
// Direction du soleil du décor (x, hauteur, z: le sud est positif), la même que celle du sol.
const SUN: vec3<f32> = vec3<f32>(-0.45, 0.75, -0.5);
const MAX_LIGHTS: u32 = 8u;

// La lumière d'une surface de normale `nn`: l'ambiance du niveau et le soleil, comme `shade` du sol.
fn shade(nn: vec3<f32>) -> vec3<f32> {
    let sun = normalize(SUN);
    let ambient = water.params.z;
    return vec3<f32>(ambient + (1.0 - ambient) / sun.y * max(dot(nn, sun), 0.0) * water.params.w);
}

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

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

// La hauteur de l'eau au point `p` (en unités du monde) à l'instant `t`: trois trains de vagues qui avancent dans des directions
// différentes, et un bruit lent qui les irrégularise.
fn height(p: vec2<f32>, t: f32) -> f32 {
    var h = 0.0;
    h = h + sin(dot(p, vec2<f32>(0.90, 0.40)) * 0.55 + t * 1.3) * 0.50;
    h = h + sin(dot(p, vec2<f32>(-0.30, 1.00)) * 0.80 + t * 1.7 + 1.7) * 0.35;
    h = h + sin(dot(p, vec2<f32>(0.60, -0.80)) * 1.35 + t * 2.4 + 4.1) * 0.20;
    h = h + (noise(p * 0.22 + vec2<f32>(t * 0.15, -t * 0.10)) - 0.5) * 0.8;
    return h;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    // Tout est calculé sur la grille des pixels du sol: l'eau a le même grain que le reste du décor.
    let p = floor(in.world_position.xz * PIXELS_PER_UNIT) / PIXELS_PER_UNIT;

    // Les rives de cette tuile, et la distance (en unités du monde) à la plus proche.
    let kx = floor(in.uv.x * 0.5);
    let ky = floor(in.uv.y * 0.5);
    let u = in.uv.x - 2.0 * kx;
    let w = in.uv.y - 2.0 * ky;
    let west = (kx % 2.0) >= 1.0;
    let east = kx >= 2.0;
    let north = (ky % 2.0) >= 1.0;
    let south = ky >= 2.0;
    var shore = 99.0;
    if (west) { shore = min(shore, u * TILE); }
    if (east) { shore = min(shore, (1.0 - u) * TILE); }
    if (north) { shore = min(shore, w * TILE); }
    if (south) { shore = min(shore, (1.0 - w) * TILE); }
    shore = floor(shore * PIXELS_PER_UNIT) / PIXELS_PER_UNIT;

    // La normale de la surface, d'après la pente des vagues.
    let e = 0.5;
    let h = height(p, t);
    let hx = height(p + vec2<f32>(e, 0.0), t) - h;
    let hz = height(p + vec2<f32>(0.0, e), t) - h;
    let n = normalize(vec3<f32>(-hx * 1.7, 1.0, -hz * 1.7));
    let v = normalize(view.world_position.xyz - in.world_position.xyz);
    let sun = normalize(SUN);

    // Le fond: clair et vert près des rives, profond et bleu au large (des couleurs sourdes, comme celles du sol).
    let deep = vec3<f32>(0.020, 0.090, 0.190);
    let shallow = vec3<f32>(0.080, 0.300, 0.380);
    var base = mix(shallow, deep, smoothstep(0.0, 18.0, shore));
    // Des lignes claires qui courent sur l'eau, comme la lumière filtrée par les vagues.
    let warp = h * 2.4 + noise(p * 0.45 + t * 0.2) * 2.0;
    let lines = pow(1.0 - abs(sin(warp * 3.14159)), 9.0);
    base = base + vec3<f32>(0.08, 0.20, 0.24) * lines * 0.7;

    // Le reflet du ciel: plus on regarde à plat, plus l'eau est un miroir (un ciel sombre, qui n'éclaire pas).
    let fresnel = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 3.0);
    let sky = vec3<f32>(0.10, 0.16, 0.34);
    var color = mix(base, sky, fresnel * 0.7);
    // Les éclats du soleil sur les vagues, et des étincelles discrètes.
    color = color + vec3<f32>(1.0, 0.96, 0.86) * pow(max(dot(reflect(-v, n), sun), 0.0), 80.0) * 1.2;
    let twinkle = hash(floor(p * 1.3) + floor(t * 2.5));
    color = color + vec3<f32>(0.85, 0.95, 1.0) * smoothstep(0.985, 1.0, twinkle) * 0.5;
    // L'écume sur les rives: une frange qui avance et recule avec les vagues.
    let reach = 2.0 + 1.3 * (0.5 + 0.5 * sin(t * 1.6 + p.x * 0.5 + p.y * 0.4)) + noise(p * 0.8 + t * 0.3) * 1.6;
    let foam = 1.0 - smoothstep(0.4, reach, shore);
    color = mix(color, vec3<f32>(0.78, 0.86, 0.90), clamp(foam, 0.0, 1.0) * 0.75);

    // L'éclairage, comme celui du sol: ambiance et soleil du niveau, puis les lampes (lanternes des héros, torches). Dans le repère des
    // lampes (x, hauteur, y du monde 2D), le sud est -y.
    var tone = shade(vec3<f32>(0.0, 1.0, 0.0));
    let here = vec3<f32>(in.world_position.x, in.world_position.y, -in.world_position.z);
    let count = u32(water.params.x);
    for (var i = 0u; i < count && i < MAX_LIGHTS; i = i + 1u) {
        let lamp = water.light_pos[i];
        let lamp_color = water.light_color[i];
        let to_lamp = vec3<f32>(lamp.x, lamp.y, lamp.z) - here;
        let dist = length(to_lamp);
        let flicker = 1.0 + 0.05 * sin(globals.time * 11.0 + f32(i) * 2.3) + 0.03 * sin(globals.time * 23.0 + f32(i) * 5.1);
        let falloff = clamp(1.0 - dist / (lamp.w * flicker), 0.0, 1.0);
        let facing = max(dot(vec3<f32>(n.x, n.y, -n.z), to_lamp / max(dist, 0.001)), 0.0);
        tone = tone + lamp_color.rgb * lamp_color.w * falloff * falloff * (0.35 + 0.65 * facing) * 0.9;
    }
    return vec4<f32>(color * water.tint.rgb * tone, 1.0);
}
