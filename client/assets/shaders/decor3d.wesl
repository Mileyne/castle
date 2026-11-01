// Décor en 3D, éclairé par un soleil et par des lampes. Deux matières:
//  - le sol: la texture du bloc d'image (le sol plat garde exactement ses couleurs: ambiante + soleil = 1);
//  - les murs (`decor.style.x` = 1 pierre, 2 feuillage): une matière calculée ici, à partir de la position dans le monde, pour que la
//    face et le dessus d'un même mur s'accordent. Le dessin est quantifié au pixel de l'art (une tuile = `room.z` pixels).
// Le soleil vient du nord-ouest: les murs projettent leur ombre vers le sud-est, sur le sol devant eux.
import bevy_pbr::{
    render::{
        forward_io::VertexOutput,
        mesh_view_bindings::globals,
    },
};

const MAX_LIGHTS: u32 = 8u;
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
    // La seconde matière des murs (le feuillage), pour les niveaux où il y a de la pierre et des arbres.
    color_d: vec4<f32>,
    color_e: vec4<f32>,
    color_f: vec4<f32>,
    light_pos: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var samp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> decor: Decor;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var walls: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var walls_samp: sampler;

// La matière du mur que l'on dessine (1 pierre, 2 feuillage), lue sur la carte des murs: chaque tuile a la sienne.
var<private> wall_kind: f32 = 1.0;

// La lumière d'un point d'une surface de normale `nn`: l'ambiance du niveau (`style.z`, très faible dans le château) et un soleil (`style.w`,
// presque éteint à l'intérieur).
fn shade(nn: vec3<f32>) -> vec3<f32> {
    let sun = normalize(SUN);
    let ambient = decor.style.z;
    return vec3<f32>(ambient + (1.0 - ambient) / sun.y * max(dot(nn, sun), 0.0) * decor.style.w);
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

// Quelle matière de mur à ce point du monde 2D (x, y)? 0: pas de mur, 1: pierre, 2: feuillage. Hors de la salle: du feuillage si le niveau
// en a (le pourtour de la carte est une forêt épaisse), sinon de la pierre.
fn wall_kind_at(p: vec2<f32>) -> f32 {
    let t = p / decor.params.w + decor.room.xy * 0.5;
    if t.x < 0.0 || t.y < 0.0 || t.x >= decor.room.x || t.y >= decor.room.y {
        return select(1.0, 2.0, decor.style.y > 1.5);
    }
    let r = textureSampleLevel(walls, walls_samp, t / decor.room.xy, 0.0).r;
    if r > 0.9 {
        return 1.0;
    }
    if r > 0.5 {
        return 2.0;
    }
    return 0.0;
}

fn foliage_at(p: vec2<f32>) -> bool {
    return wall_kind_at(p) > 1.5;
}

// Part du soleil que le feuillage cache à un point du sol (0: rien, 1: tout), calculée sans la grille des tuiles: la couronne d'un arbre est
// un disque, et son ombre est la capsule que ce disque balaie quand on le monte de la base de la couronne (40 % de la hauteur) jusqu'au
// sommet. Le point est dans l'ombre s'il est près du segment que le rayon vers le soleil parcourt à cette hauteur, un peu plus qu'à un rayon
// d'un centre d'arbre; le bord est adouci. Résultat: une ombre arrondie et douce, au lieu d'escaliers de tuiles.
fn foliage_shadow(ground: vec2<f32>) -> f32 {
    let toward = vec2<f32>(SUN.x, -SUN.z) / SUN.y;
    let a = ground + toward * (decor.params.z * 0.4);
    let b = ground + toward * decor.params.z;
    let ab = b - a;
    let tile = decor.params.w;
    let base = floor((a + b) * 0.5 / tile + decor.room.xy * 0.5);
    var cover = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let center = (base + vec2<f32>(f32(dx), f32(dy)) + vec2<f32>(0.5) - decor.room.xy * 0.5) * tile;
            if foliage_at(center) {
                let k = clamp(dot(center - a, ab) / max(dot(ab, ab), 0.001), 0.0, 1.0);
                let d = length(center - (a + ab * k));
                cover = max(cover, 1.0 - smoothstep(tile * 0.45, tile * 0.78, d));
            }
        }
    }
    return cover;
}

// Part de lumière du soleil qui atteint un point du sol à l'air libre: on remonte le rayon vers le soleil, et un mur de pierre le coupe
// tant que le rayon est plus bas que lui (ombre à angles droits, comme la pierre); le feuillage a son ombre arrondie.
fn sun_open(ground: vec2<f32>) -> f32 {
    let toward = vec2<f32>(SUN.x, -SUN.z) / SUN.y;
    var shade = 0.0;
    for (var i = 1u; i <= SHADOW_STEPS; i = i + 1u) {
        let height = decor.params.z * f32(i) / f32(SHADOW_STEPS);
        let kind = wall_kind_at(ground + toward * height);
        if kind > 0.5 && kind < 1.5 {
            shade = 1.0;
            break;
        }
    }
    if decor.style.y > 1.5 {
        shade = max(shade, foliage_shadow(ground));
    }
    return mix(1.0, SHADOW_TONE, shade);
}

// Part de lumière du soleil qui atteint un point du sol. À l'intérieur d'un mur, le sol n'est pas dans son ombre; sous un arbre il prend la
// lumière du sol juste devant le bloc (un pas vers la caméra), pour qu'aucune limite ne marque le pied des troncs.
fn sunlight(ground: vec2<f32>) -> f32 {
    if wall_at(ground) {
        if foliage_at(ground) {
            let front = ground + camera_dir() * decor.params.w;
            if wall_at(front) {
                return SHADOW_TONE;
            }
            return sun_open(front);
        }
        return 1.0;
    }
    return sun_open(ground);
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

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Bruit de valeur lissé, en trois dimensions.
fn vnoise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(
        mix(hash31(i), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
        mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
        u.y,
    );
    let b = mix(
        mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
        mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
        u.y,
    );
    return mix(a, b, u.z);
}

// Le motif cellulaire en trois dimensions: le centre le plus proche parmi des centres jetés au hasard (un par case de `size`
// unités). Renvoie la distance à ce centre (en cases), un nombre propre à la case, et le vecteur vers le centre.
struct Cell {
    dist: f32,
    id: f32,
    toward: vec3<f32>,
};

fn cell_of(p: vec3<f32>, size: f32) -> Cell {
    let g = p / size;
    let i = floor(g);
    let f = fract(g);
    var best = Cell(8.0, 0.0, vec3<f32>(0.0));
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let c = vec3<f32>(f32(dx), f32(dy), f32(dz));
                let r = vec3<f32>(hash31(i + c), hash31(i + c + vec3<f32>(31.7, 17.3, 5.9)), hash31(i + c + vec3<f32>(7.1, 53.3, 23.7)));
                let to = c + r - f;
                let d = length(to);
                if d < best.dist {
                    best = Cell(d, hash31(i + c + vec3<f32>(5.1, 9.7, 41.3)), to);
                }
            }
        }
    }
    return best;
}

// Feuillage: des touffes arrondies, éclairées par le soleil, sombres entre elles. Le motif est dans l'espace (x, hauteur, y du monde 2D):
// il se poursuit d'une face au dessus et d'un côté à l'autre sans coupure, au lieu de changer à chaque arête du cube.
fn leaves(p: vec3<f32>) -> vec3<f32> {
    let px = pixel();
    let q = floor(p / px) * px;
    let big = cell_of(q, 8.0);
    // Le soleil vient du nord-ouest et d'en haut (x, hauteur, y): la touffe est éclairée du côté qui lui fait face.
    let sun = normalize(vec3<f32>(-0.45, 0.75, 0.5));
    let lit = clamp(0.5 - 1.0 * dot(big.toward, sun), 0.0, 1.0);
    var color = mix(decor.color_e.rgb, decor.color_d.rgb, clamp(big.id * 0.55 + lit * 0.6, 0.0, 1.0));
    // Entre deux touffes, c'est l'ombre.
    color = color * (1.0 - 0.55 * smoothstep(0.38, 0.72, big.dist));
    // Quelques feuilles claires sur la partie éclairée.
    if lit > 0.55 && hash31(q / px) > 0.93 {
        color = color * 1.35;
    }
    return color * (0.94 + 0.12 * hash31(q / px + vec3<f32>(3.0, 1.0, 2.0)));
}

// Herbe sous les arbres: des plaques et de petits brins, au pixel de l'art.
fn grass(p: vec2<f32>) -> vec3<f32> {
    let px = pixel();
    let q = floor(p / px) * px;
    let clumps = vnoise(q / 9.0) * 0.6 + vnoise(q / 3.1 + vec2<f32>(7.0, 3.0)) * 0.4;
    // Sous les arbres, dans l'ombre de la couronne: presque noir, avec quelques brins un peu moins sombres.
    var color = mix(vec3<f32>(0.0035, 0.0085, 0.0025), vec3<f32>(0.011, 0.022, 0.006), smoothstep(0.3, 0.7, clumps));
    let blade = hash21(q / px);
    if blade > 0.9 {
        color = color * 1.6;
    }
    return color;
}

// Y a-t-il beaucoup d'arbres derrière ce point (du côté opposé à la caméra)? 0: rien, on verrait le sol; 1: des arbres sur plusieurs
// tuiles (le bord d'un niveau), tout est sombre derrière. Sept points, jusqu'à près de trois tuiles de profondeur.
fn behind_density(p: vec2<f32>) -> f32 {
    let away = -camera_dir();
    let side = vec2<f32>(-away.y, away.x);
    let tile = decor.params.w;
    var count = 0.0;
    for (var k = 1; k <= 3; k = k + 1) {
        let at = p + away * (f32(k) * 0.9 * tile);
        if foliage_at(at) {
            count = count + 1.0;
        }
        if k == 2 {
            if foliage_at(at + side * tile * 0.8) {
                count = count + 1.0;
            }
            if foliage_at(at - side * tile * 0.8) {
                count = count + 1.0;
            }
        }
    }
    return count / 5.0;
}

// À quel point un point du sol est au pied d'un mur (0: loin, 1: contre lui): la part des points d'un anneau de 6 puis de 12 unités qui
// sont en mur. Sert à assombrir le sol au pied des arbres.
fn wall_proximity(p: vec2<f32>) -> f32 {
    var near = 0.0;
    for (var k = 0; k < 8; k = k + 1) {
        let angle = f32(k) * 0.785398;
        let dir = vec2<f32>(cos(angle), sin(angle));
        if foliage_at(p + dir * 6.0) {
            near = near + 0.6;
        }
        if foliage_at(p + dir * 12.0) {
            near = near + 0.4;
        }
    }
    return near / 8.0;
}

// Y a-t-il du vide (pas de mur) à ce point?
fn open_at(p: vec2<f32>) -> f32 {
    if wall_at(p) {
        return 0.0;
    }
    return 1.0;
}

// Du côté de quels bords exposés on est: un vecteur (x, y du monde 2D) qui pointe vers le vide, à `r` unités. Nul au coeur d'un mur.
fn exposed(p: vec2<f32>, r: f32) -> vec2<f32> {
    return vec2<f32>(
        open_at(p + vec2<f32>(r, 0.0)) - open_at(p - vec2<f32>(r, 0.0)),
        open_at(p + vec2<f32>(0.0, r)) - open_at(p - vec2<f32>(0.0, r)),
    );
}

// La normale arrondie d'un bloc de feuillage, pour l'éclairage: une couronne n'a pas d'arêtes vives. Le dessus penche vers le vide près
// de ses bords, le haut d'une face penche vers le haut, et les coins verticaux exposés penchent vers l'extérieur. L'éclairage passe
// ainsi progressivement d'une face à l'autre. `here` est (x, hauteur, y du monde 2D), `n` la normale de la face (le sud est +z).
fn rounded_normal(n: vec3<f32>, here: vec3<f32>, amount: f32) -> vec3<f32> {
    if n.y < -0.9 || here.y < 3.0 {
        return n;
    }
    if n.y > 0.9 {
        // Le dessus penche vers le vide, de plus en plus près du bord (distance au bord de la tuile, du côté qui est ouvert).
        let tile = decor.params.w;
        let tt = here.xz / tile + decor.room.xy * 0.5;
        let u = fract(tt.x) * tile;
        let v = fract(tt.y) * tile;
        let reach = tile * (0.1 + 0.4 * amount);
        let ex = open_at(here.xz + vec2<f32>(tile - u + 1.0, 0.0)) * (1.0 - smoothstep(0.0, reach, tile - u))
            - open_at(here.xz - vec2<f32>(u + 1.0, 0.0)) * (1.0 - smoothstep(0.0, reach, u));
        let ey = open_at(here.xz + vec2<f32>(0.0, tile - v + 1.0)) * (1.0 - smoothstep(0.0, reach, tile - v))
            - open_at(here.xz - vec2<f32>(0.0, v + 1.0)) * (1.0 - smoothstep(0.0, reach, v));
        return normalize(vec3<f32>(ex * 0.7 * amount, 1.0, -ey * 0.7 * amount));
    }
    let to_top = 1.0 - smoothstep(0.0, 10.0 * amount, decor.params.z - here.y);
    // Le long de la face (x pour le nord et le sud, y pour l'est et l'ouest), un peu à l'intérieur du mur.
    let tangent = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), abs(n.z) > 0.5);
    let inside = here.xz - vec2<f32>(n.x, -n.z) * 2.0;
    // Un arbre seul est un cylindre: la normale tourne vers le côté à mesure qu'on s'éloigne du milieu de la tuile, du côté qui donne
    // sur le vide. Entre deux arbres voisins, la face reste plate.
    let tile = decor.params.w;
    let tt = here.xz / tile + decor.room.xy * 0.5;
    let u = fract(select(tt.y, tt.x, abs(n.z) > 0.5)) * tile;
    let offset = (u - tile * 0.5) / (tile * 0.5);
    let side_open = open_at(inside + tangent * (select(-1.0, 1.0, offset > 0.0) * (select(u, tile - u, offset > 0.0) + 1.0)));
    let corner = sign(offset) * smoothstep(0.0, 1.0, abs(offset)) * side_open;
    let tangent_scene = vec3<f32>(tangent.x, 0.0, -tangent.y);
    // Au sommet de la face, la normale est celle du bord du dessus (à 45 degrés vers l'extérieur): les deux se raccordent sans bande.
    let rim = normalize(vec3<f32>(0.0, amount, 0.0) + 0.9 * n);
    return normalize(mix(n, rim, to_top) + tangent_scene * corner * 0.9 * amount);
}

// Distance, en unités, du point du dessus d'un mur aux bords de sa tuile qui donnent sur le vide: (x, y), 99 s'il n'y en a pas de ce côté.
fn edge_gaps(p: vec2<f32>) -> vec2<f32> {
    let tile = decor.params.w;
    let t = p / tile + decor.room.xy * 0.5;
    let u = fract(t.x) * tile;
    let v = fract(t.y) * tile;
    let dx = min(
        select(99.0, u, open_at(p - vec2<f32>(u + 1.0, 0.0)) > 0.5),
        select(99.0, tile - u, open_at(p + vec2<f32>(tile - u + 1.0, 0.0)) > 0.5),
    );
    let dy = min(
        select(99.0, v, open_at(p - vec2<f32>(0.0, v + 1.0)) > 0.5),
        select(99.0, tile - v, open_at(p + vec2<f32>(0.0, tile - v + 1.0)) > 0.5),
    );
    return vec2<f32>(dx, dy);
}

// Le coin du dessus d'un bloc de pierre est-il coupé? Un petit arrondi (`r` unités) aux coins exposés, sauf ceux qui font face à la
// caméra, qui se raccordent à la face. Rien pour les coins entre deux pierres.
fn corner_cut(p: vec2<f32>, r: f32) -> bool {
    let tile = decor.params.w;
    let gaps = edge_gaps(p);
    if gaps.x >= r || gaps.y >= r {
        return false;
    }
    let t = p / tile + decor.room.xy * 0.5;
    let local = (fract(t) - vec2<f32>(0.5)) * tile;
    // Le coin est du côté (signe de x, signe de y) du point dans sa tuile.
    if dot(sign(local), camera_dir()) > 0.3 {
        return false;
    }
    let cx = r - gaps.x;
    let cy = r - gaps.y;
    return cx * cx + cy * cy > r * r;
}

// Rayon de la couronne d'un arbre (un disque centré sur la tuile), et rayon des coins du bas d'une face.
const CROWN_RADIUS: f32 = 8.2;
const CROWN_ROUND: f32 = 9.0;

// Direction de la caméra sur le sol (x, y du monde 2D): elle est au sud pour un angle nul. `decor.room.w` est l'angle de la vue.
fn camera_dir() -> vec2<f32> {
    return vec2<f32>(sin(decor.room.w), -cos(decor.room.w));
}

// Le point `p` du dessus d'un mur de feuillage est-il hors de la couronne? La couronne d'un arbre est un disque, et la couronne de deux
// arbres voisins la réunion de leurs disques et de celui de leur jonction: la jonction est pleine, bien touffue, et seuls les bouts
// sont arrondis. Si `joined`, la moitié du côté de la caméra reste carrée: elle se raccorde sans col à la face, qui est plate (le contour
// est alors celui d'un tambour vu de biais: demi-cercle au fond, côtés droits devant).
fn crown_cut(p: vec2<f32>, joined: bool) -> bool {
    let tile = decor.params.w;
    let t = p / tile + decor.room.xy * 0.5;
    let local = (fract(t) - vec2<f32>(0.5)) * tile;
    if length(local) <= CROWN_RADIUS {
        return false;
    }
    if joined && dot(local, camera_dir()) > 0.0 {
        return false;
    }
    let center = p - local;
    let half = tile * 0.5;
    // Les disques des jonctions avec les tuiles voisines qui sont des murs (au milieu du bord commun).
    if local.x > 0.0 && wall_at(center + vec2<f32>(tile, 0.0)) && length(local - vec2<f32>(half, 0.0)) <= CROWN_RADIUS {
        return false;
    }
    if local.x < 0.0 && wall_at(center - vec2<f32>(tile, 0.0)) && length(local + vec2<f32>(half, 0.0)) <= CROWN_RADIUS {
        return false;
    }
    if local.y > 0.0 && wall_at(center + vec2<f32>(0.0, tile)) && length(local - vec2<f32>(0.0, half)) <= CROWN_RADIUS {
        return false;
    }
    if local.y < 0.0 && wall_at(center - vec2<f32>(0.0, tile)) && length(local + vec2<f32>(0.0, half)) <= CROWN_RADIUS {
        return false;
    }
    return true;
}

// Les coins du bas d'une face de feuillage (au bout d'un mur, sur le vide) sont coupés en arrondi: la couronne n'a pas d'angle droit.
// `base` est la hauteur du bas de la couronne à cet endroit.
fn face_bottom_cut(n: vec3<f32>, here: vec3<f32>, base: f32) -> bool {
    let tile = decor.params.w;
    let t = here.xz / tile + decor.room.xy * 0.5;
    let along_x = abs(n.z) > 0.5;
    let u = fract(select(t.y, t.x, along_x)) * tile;
    let tangent = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), along_x);
    let inside = here.xz - vec2<f32>(n.x, -n.z) * 2.0;
    let de = min(
        select(99.0, u, open_at(inside - tangent * (u + 1.0)) > 0.5),
        select(99.0, tile - u, open_at(inside + tangent * (tile - u + 1.0)) > 0.5),
    );
    let db = here.y - base;
    if de < CROWN_ROUND && db < CROWN_ROUND {
        let cx = CROWN_ROUND - de;
        let cy = CROWN_ROUND - db;
        return cx * cx + cy * cy > CROWN_ROUND * CROWN_ROUND;
    }
    return false;
}

// Un pixel de feuillage est-il présent? Au coeur du bois le feuillage est plein et touffu; en bordure il s'effiloche, par touffes,
// et laisse voir derrière. `ragged` creuse encore davantage (le bas de la couronne).
fn leaf_present(q: vec3<f32>, depth: f32, ragged: f32) -> bool {
    let px = pixel();
    let clump = vnoise3(q / 3.5) * 0.65 + hash31(floor(q / px)) * 0.35;
    let dense = smoothstep(0.35, 0.95, depth);
    return clump < mix(0.55, 1.15, dense) - ragged;
}

// Le sol de tuile: les coordonnées (x du monde décalé pour tomber sur les bords de tuile, y).
fn tile_origin() -> vec2<f32> {
    return decor.room.xy * 0.5 * decor.params.w;
}

// La face d'un mur (au sud): `fx` position le long du mur, `h` hauteur, `ground` le point du sol au pied de la face. Le résultat est la
// couleur, et en alpha 0 ce qui est vide (entre les troncs, trous du feuillage): on y voit à travers.
fn wall_face(fx: f32, h: f32, ground: vec2<f32>, n: vec3<f32>) -> vec4<f32> {
    if wall_kind < 1.5 {
        // Pierre: rangs de briques de 8 unités, de la largeur d'une demi-tuile.
        return vec4<f32>(bricks(vec2<f32>(fx, h), 8.0, 12.0, 3.0), 1.0);
    }
    // Arbres: des troncs au pied, puis le feuillage, qui est aussi celui du dessus.
    let px = pixel();
    let tile = decor.params.w;
    let column = floor(fx / tile);
    let across = fx - column * tile;
    let center = tile * 0.5 + (hash21(vec2<f32>(column, 1.0)) - 0.5) * 5.0;
    var canopy = decor.params.z * 0.4 + 7.0 * vnoise(vec2<f32>(fx / 5.0, 3.0));
    // À la jonction de deux arbres voisins, le feuillage descend plus bas: une masse touffue les relie.
    let along_x = abs(n.z) > 0.5;
    let tangent = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), along_x);
    let inside = ground - vec2<f32>(n.x, -n.z) * 2.0;
    let left_joined = open_at(inside - tangent * (across + 1.0)) < 0.5;
    let right_joined = open_at(inside + tangent * (tile - across + 1.0)) < 0.5;
    let hang = max(
        select(0.0, 1.0 - smoothstep(0.0, 6.0, across), left_joined),
        select(0.0, 1.0 - smoothstep(0.0, 6.0, tile - across), right_joined),
    );
    canopy = canopy - 9.0 * hang;
    if h >= canopy && face_bottom_cut(n, vec3<f32>(ground.x, h, ground.y), canopy) {
        return vec4<f32>(0.0);
    }
    // Le tronc, un peu plus large au pied (les racines) et qui s'effile; plus sombre en bas, avec des stries d'écorce et un flanc dans
    // l'ombre. Il monte jusqu'en haut: le feuillage le cache, sauf là où il est clairsemé.
    let flare = 1.0 - clamp(h / 10.0, 0.0, 1.0);
    let half_width = (2.2 + 0.7 * hash21(vec2<f32>(column, 2.0)) + 1.4 * flare) * mix(1.0, 0.7, clamp(h / decor.params.z, 0.0, 1.0));
    let side = (across - center) / half_width;
    if h >= canopy {
        // Le feuillage: plein au coeur d'un bois, effiloché en bordure, et plus clairsemé au bas de la couronne.
        let ragged = 0.55 * (1.0 - smoothstep(0.0, 12.0, h - canopy));
        let at = vec3<f32>(ground.x, h, ground.y);
        if leaf_present(at, 1.0, ragged) {
            return vec4<f32>(leaves(at), 1.0);
        }
    }
    // Le pied du tronc est arrondi (la base d'un cylindre vu de biais): ses coins du bas sont coupés en arc, au lieu d'une base droite posée
    // sur le bord du bloc.
    let foot = 4.5 * (1.0 - sqrt(max(0.0, 1.0 - side * side)));
    if abs(side) < 1.0 && h >= foot {
        let q = floor(fx / px);
        let streak = 0.78 + 0.34 * hash21(vec2<f32>(q, 5.0));
        let bark = decor.color_f.rgb * 1.5 * streak * mix(0.6, 1.0, clamp(h / canopy, 0.0, 1.0));
        return vec4<f32>(bark * (1.15 - 0.55 * smoothstep(-0.2, 1.0, side)), 1.0);
    }
    // Entre les troncs: rien, on voit le bois derrière puis l'herbe.
    return vec4<f32>(0.0);
}

// Le dessus d'un mur: `p` position sur le sol (x, y du monde 2D).
fn wall_top(p: vec2<f32>) -> vec4<f32> {
    if wall_kind < 1.5 {
        // Pierre: les mêmes appareils que la face, vus de dessus, avec un biseau clair le long des arêtes qui donnent sur le vide (la pierre
        // est taillée: son sommet n'est pas un carré plat), et des joints qui s'effacent un peu au bord (pierre usée).
        var color = bricks(p + tile_origin(), 8.0, 12.0, 9.0);
        let gaps = edge_gaps(p);
        let nearest = min(gaps.x, gaps.y);
        let wear = hash21(floor((p + tile_origin()) / pixel()) + vec2<f32>(5.0, 2.0));
        color = color * (1.0 + 0.34 * (1.0 - smoothstep(0.0, 3.2, nearest + 1.5 * wear)));
        // Au coin exposé, le biseau se referme: le coin est plus clair encore.
        if gaps.x < 4.0 && gaps.y < 4.0 {
            color = color * 1.12;
        }
        return vec4<f32>(color, 1.0);
    }
    // Le dessus de la couronne: touffu au coeur d'un bois, ajouré en bordure, et effiloché le long des bords qui donnent sur le vide.
    let at = vec3<f32>(p.x, decor.params.z, p.y);
    // Effiloché le long des bords exposés, sauf du côté de la caméra: là, le dessus se raccorde à la face sans creux sombre.
    let e = exposed(p, 4.0);
    let ragged = 0.4 * clamp(length(e - camera_dir() * max(dot(e, camera_dir()), 0.0)), 0.0, 1.0);
    if !leaf_present(at, 1.0, ragged) {
        // Le long d'un bord exposé, un creux du feuillage est une touffe dans l'ombre (le trou laisserait voir l'intérieur du bloc et
        // ferait une bande sombre le long de l'arête); ailleurs, c'est un vrai vide.
        if ragged > 0.01 {
            return vec4<f32>(leaves(at) * 0.35, 1.0);
        }
        return vec4<f32>(0.0);
    }
    return vec4<f32>(leaves(at), 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    // Position dans le monde 2D (x, hauteur, y): le nord est -z dans la scène.
    let here = vec3<f32>(in.world_position.x, in.world_position.y, -in.world_position.z);
    var base = vec3<f32>(0.0);
    var tone = shade(n);
    // Le plancher de lumière des faces qui tournent le dos au soleil: à l'extérieur 0,62, dans le château 0,29, dans les bois à la nuit
    // tombante environ 0,12 (le décalage fixe de 0,17 vaut pour un soleil franc et disparaît quand il s'éteint).
    let fill = decor.style.z + 0.17 * clamp((decor.style.w - 0.08) / 0.12, 0.0, 1.0);
    if decor.style.x > 0.5 {
        // Un cube de mur: la même matière sur ses six faces, celle de sa tuile.
        let off = tile_origin();
        let outward = select(vec2<f32>(0.0), vec2<f32>(n.x, -n.z), abs(n.y) < 0.9);
        wall_kind = max(wall_kind_at(here.xz - outward * 2.0), 1.0);
        if wall_kind > 1.5 {
            // Le feuillage est éclairé comme une masse arrondie, pas comme un cube, avec le même plancher de lumière sur le dessus et
            // sur les faces (sinon le bord du dessus, penché vers l'ombre, fait une bande sombre).
            tone = max(shade(rounded_normal(n, here, 1.0)), vec3<f32>(fill));
        } else {
            // La pierre a un sommet biseauté: l'éclairage s'arrondit un peu aux arêtes.
            tone = shade(rounded_normal(n, here, 0.4));
        }
        if n.y > 0.9 {
            if here.y < 3.0 {
                // Le fond derrière la première rangée de murs: de l'herbe sous les arbres, sinon la même roche, à plat et plus sombre.
                if wall_kind > 1.5 {
                    // Le sous-bois suit le contour arrondi de la couronne.
                    if crown_cut(here.xz, false) {
                        discard;
                    }
                    // Le sous-bois dense n'existe qu'en bordure de forêt, quand il y a beaucoup d'arbres derrière. Pour un arbre seul ou une
                    // rangée, on ne pose aucune texture de plus: on voit le sol du jeu à travers le feuillage.
                    if behind_density(here.xz) < 0.4 {
                        discard;
                    }
                    base = grass(here.xz);
                } else {
                    base = wall_top(here.xz).rgb * 0.5;
                }
            } else {
                let top = wall_top(here.xz);
                if top.a < 0.5 || (wall_kind > 1.5 && crown_cut(here.xz, true)) || (wall_kind < 1.5 && corner_cut(here.xz, 3.5)) {
                    discard;
                }
                base = top.rgb;
            }
        } else if n.y < -0.9 {
            base = vec3<f32>(0.0);
        } else {
            // Un côté: le long du mur (x pour le nord et le sud, y pour l'est et l'ouest) et en hauteur.
            let along = select(here.z + off.y, here.x + off.x, abs(n.z) > 0.5);
            let face = wall_face(along, here.y, here.xz, n);
            if face.a < 0.5 {
                discard;
            }
            base = face.rgb;
            // Les côtés qui tournent le dos au soleil gardent une lumière de remplissage; plus sombres au pied qu'en haut.
            tone = max(tone, vec3<f32>(fill)) * mix(0.7, 1.0, clamp(here.y / decor.params.z, 0.0, 1.0));
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
            // Sous les arbres, le sol devient très sombre au pied des troncs (l'ombre et le sous-bois).
            if decor.style.y > 1.5 {
                // Plus sombre encore quand il y a d'autres arbres derrière (le bord du niveau); moins pour un arbre seul ou une rangée.
                let foot = mix(0.5, 0.12, smoothstep(0.2, 0.8, behind_density(here.xz)));
                tone = tone * mix(1.0, foot, smoothstep(0.0, 0.7, wall_proximity(here.xz)));
            }
        }
    }
    // Les lampes (lanternes des héros, torches): une tache de lumière chaude, qui vacille un peu.
    let count = u32(decor.params.y);
    for (var i = 0u; i < count && i < MAX_LIGHTS; i = i + 1u) {
        let lamp = decor.light_pos[i];
        let color = decor.light_color[i];
        let to_lamp = vec3<f32>(lamp.x, lamp.y, lamp.z) - here;
        let dist = length(to_lamp);
        let flicker = 1.0 + 0.05 * sin(globals.time * 11.0 + f32(i) * 2.3) + 0.03 * sin(globals.time * 23.0 + f32(i) * 5.1);
        let falloff = clamp(1.0 - dist / (lamp.w * flicker), 0.0, 1.0);
        // Normale dans le même repère (x, hauteur, y du monde 2D): le sud devient -y.
        let facing = max(dot(vec3<f32>(n.x, n.y, -n.z), to_lamp / max(dist, 0.001)), 0.0);
        tone = tone + color.rgb * color.w * falloff * falloff * (0.35 + 0.65 * facing) * 0.9;
    }
    return vec4<f32>(base * tone, 1.0);
}
