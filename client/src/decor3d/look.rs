//! L'allure d'un niveau en 3D: la matière de chaque tuile de mur (pierre ou feuillage: le niveau 3, le chemin du château, a des murs de
//! pierre et des arbres), les couleurs des deux matières, l'ambiance de lumière (sombre dans le château), les torches et les lampes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::prelude::*;
use castle_shared::{LevelDef, Region, Room, TILE_SIZE, Tile};
use serde::{Deserialize, Serialize};

use super::{MAX_LIGHTS, wall_style};

/// Les couleurs (linéaires) des deux matières de murs d'un niveau: [première variante, seconde variante, joints ou écorce].
#[derive(Clone, Copy)]
pub struct Palette {
    pub stone: [Vec4; 3],
    pub foliage: [Vec4; 3],
}

/// Comment une catégorie de modèles (héros, ennemis, objets) réagit à la lumière: `gain` est sa capacité à capter la lumière (il
/// multiplie ses couleurs: 1 est normal, 3 la fait paraître trois fois plus éclairée, sans changer ses teintes), et `glow` sa
/// capacité à rayonner: la part de ses propres couleurs (sommets, tissage, peau) qu'il émet, quelle que soit la lumière reçue (0: aucune,
/// 0,3: il garde environ un tiers de ses couleurs dans le noir complet).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Response {
    pub gain: f32,
    pub glow: f32,
}

impl Default for Response {
    fn default() -> Self {
        Self { gain: 1.0, glow: 0.0 }
    }
}

/// Les réglages de lumière d'un niveau: l'ambiance (lumière ambiante et soleil), les lanternes des héros, et la réponse à la lumière des
/// héros, des ennemis et des objets.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelLight {
    pub ambient: f32,
    pub sun: f32,
    /// Multiplicateurs de l'intensité et du rayon des lanternes des héros.
    pub lantern_intensity: f32,
    pub lantern_radius: f32,
    pub hero: Response,
    pub enemy: Response,
    pub object: Response,
    /// Couleur de la lumière ambiante: multiplie le sol, les murs, l'eau et la lumière ambiante des modèles (blanc: aucune teinte).
    pub tint: [f32; 3],
    /// Couleur (linéaire) des lanternes des héros.
    pub lantern: [f32; 3],
    /// Couleur (linéaire) de la lumière des torches.
    pub torch: [f32; 3],
}

/// Couleur de lumière sans teinte, des lanternes et des torches.
pub const NO_TINT: [f32; 3] = [1.0, 1.0, 1.0];
pub const LANTERN_COLOR: [f32; 3] = [1.0, 0.87, 0.68];
pub const TORCH_COLOR: [f32; 3] = [1.0, 0.42, 0.10];

impl Default for LevelLight {
    fn default() -> Self {
        Self::base(Region::Forest)
    }
}

impl LevelLight {
    /// Les valeurs d'une région quand aucun réglage n'est enregistré: l'ambiance du code, des lanternes normales, des héros qui captent la
    /// lumière deux fois et demie (ils portent la lumière: ils doivent rester lisibles), les ennemis et les objets sans réglage.
    pub fn base(region: Region) -> Self {
        let mood = Mood::of(region);
        Self {
            ambient: mood.ambient,
            sun: mood.sun,
            lantern_intensity: 1.0,
            lantern_radius: 1.0,
            hero: Response { gain: 2.5, glow: 0.0 },
            enemy: Response::default(),
            object: Response::default(),
            tint: NO_TINT,
            lantern: LANTERN_COLOR,
            // À la forge, les torches sont rouges.
            torch: if region == Region::Forge { [1.0, 0.16, 0.04] } else { TORCH_COLOR },
        }
    }

    pub fn mood(&self) -> Mood {
        Mood { ambient: self.ambient, sun: self.sun }
    }
}

/// Les réglages du niveau en cours: ce que le jeu lit partout (ambiance, lanternes, matériaux des modèles). Le jeu la change à chaque
/// niveau; le programme `lumiere` la change quand on règle ou qu'on passe à un autre niveau.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ActiveLight(pub LevelLight);

/// Les réglages de lumière du jeu, que le programme `lumiere` (`./lumiere.sh`) modifie en direct et enregistre dans
/// `client/assets/lighting.ron`. Le jeu les lit au lancement; sans fichier, ce sont les valeurs du code.
///
/// Chaque niveau peut avoir ses réglages (`levels`). Un niveau sans réglages prend ceux de sa région (`moods`, sinon `Mood::of`), avec
/// les lanternes et les réponses communes (`lantern_*`, `hero`, `enemy`, `object`): c'est le format d'avant les niveaux, qui reste lu.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightTuning {
    /// Les réglages propres à un niveau (son numéro, de 1 à 12).
    pub levels: BTreeMap<u32, LevelLight>,
    /// Par région (son nom): (lumière ambiante, soleil). Une région absente garde `Mood::of`.
    pub moods: BTreeMap<String, (f32, f32)>,
    pub lantern_intensity: f32,
    pub lantern_radius: f32,
    pub hero: Response,
    pub enemy: Response,
    pub object: Response,
}

impl Default for LightTuning {
    fn default() -> Self {
        let base = LevelLight::base(Region::Forest);
        Self {
            levels: BTreeMap::new(),
            moods: BTreeMap::new(),
            lantern_intensity: base.lantern_intensity,
            lantern_radius: base.lantern_radius,
            hero: base.hero,
            enemy: base.enemy,
            object: base.object,
        }
    }
}

impl LightTuning {
    /// L'ambiance d'une région: celle des réglages, sinon celle du code.
    fn region_mood(&self, region: Region) -> Mood {
        self.moods.get(&format!("{region:?}")).map_or_else(|| Mood::of(region), |(ambient, sun)| Mood { ambient: *ambient, sun: *sun })
    }

    /// Les réglages d'un niveau: les siens s'il en a, sinon ceux de sa région.
    pub fn light(&self, level: u32, region: Region) -> LevelLight {
        if let Some(light) = self.levels.get(&level) {
            return *light;
        }
        let mood = self.region_mood(region);
        LevelLight {
            ambient: mood.ambient,
            sun: mood.sun,
            lantern_intensity: self.lantern_intensity,
            lantern_radius: self.lantern_radius,
            hero: self.hero,
            enemy: self.enemy,
            object: self.object,
            ..LevelLight::base(region)
        }
    }

    /// Fixe les réglages propres d'un niveau.
    pub fn set_level(&mut self, level: u32, light: LevelLight) {
        self.levels.insert(level, light);
    }

    /// Le fichier de réglages du dépôt (celui que le programme `lumiere` écrit).
    pub fn file() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lighting.ron"))
    }

    /// Lit les réglages: à côté du programme (version livrée), sinon dans le dépôt; à défaut, les valeurs du code.
    pub fn load() -> Self {
        let mut places = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                places.push(dir.join("assets/lighting.ron"));
                if let Some(up) = dir.parent() {
                    places.push(up.join("Resources/assets/lighting.ron"));
                }
            }
        }
        places.push(Self::file());
        places.into_iter().find_map(|place| ron::from_str(&std::fs::read_to_string(place).ok()?).ok()).unwrap_or_default()
    }

    /// Enregistre les réglages dans le dépôt, et rend le chemin du fichier.
    pub fn save(&self) -> std::io::Result<PathBuf> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(std::io::Error::other)?;
        let path = Self::file();
        std::fs::write(&path, text)?;
        Ok(path)
    }
}

/// L'ambiance de lumière d'un niveau: la lumière ambiante, et la force du soleil. À l'extérieur, un jour ordinaire; dans le château, presque
/// plus de soleil et une ambiance très sombre, où seules les lampes (héros, torches) éclairent.
#[derive(Resource, Clone, Copy)]
pub struct Mood {
    pub ambient: f32,
    pub sun: f32,
}

impl Mood {
    pub fn of(region: Region) -> Self {
        match region {
            // Dans le château, plus aucun soleil: seules les lampes (lanternes des héros, torches) éclairent, sur une ambiance très sombre.
            // Les jardins sont dans l'enceinte: pas de soleil non plus.
            Region::Crypts | Region::Library | Region::Forge | Region::Tower | Region::Royal | Region::Gardens => Self { ambient: 0.12, sun: 0.0 },
            // À l'extérieur, le soleil se lève d'un niveau à l'autre: 0,1 dans la forêt (niveau 1), 0,25 dans le marais (niveau 2), 0,5 sur le
            // chemin du château (niveaux 3 et 4, qui partagent la région).
            Region::Forest => Self { ambient: 0.0, sun: 0.1 },
            Region::Marsh => Self { ambient: 0.0, sun: 0.25 },
            Region::Approach => Self { ambient: 0.38, sun: 0.5 },
        }
    }

    /// 0 à ciel ouvert, 1 dans le noir du château.
    pub fn darkness(self) -> f32 {
        ((0.45 - self.ambient) / 0.33).clamp(0.0, 1.0)
    }
}

impl Default for Mood {
    fn default() -> Self {
        Self::of(Region::Forest)
    }
}

/// Le décor d'un niveau de la campagne, si le niveau est bien celui de la carte (sinon la région du niveau vaut pour toutes les tuiles).
pub fn level_of(room: &Room, level: u32) -> Option<&'static LevelDef> {
    let def = castle_shared::level_def(level);
    (def.size == room.size()).then_some(def)
}

/// La matière de chaque tuile, dans l'ordre du masque des murs (une ligne après l'autre, la plus au sud d'abord): 0 pas de mur, 1 pierre,
/// 2 feuillage.
pub fn wall_kinds(room: &Room, region: Region, def: Option<&LevelDef>) -> Vec<u8> {
    let size = room.size();
    (0..size.y as i32)
        .flat_map(|y| (0..size.x as i32).map(move |x| (x, y)))
        .map(|(x, y)| {
            if room.tile(x, y) != Tile::Wall {
                return 0;
            }
            wall_style(def.map_or(region, |def| def.region_at(x, y))).0 as u8
        })
        .collect()
}

/// Les couleurs des deux matières: celles de la région du niveau et de la première région de l'autre sorte que le niveau emprunte.
pub fn palette(region: Region, room: &Room, def: Option<&LevelDef>) -> Palette {
    let mut stone = None;
    let mut foliage = None;
    let mut note = |region: Region| {
        let (kind, colors) = wall_style(region);
        let slot = if kind > 1.5 { &mut foliage } else { &mut stone };
        slot.get_or_insert(colors);
    };
    note(region);
    if let Some(def) = def {
        let size = room.size();
        for y in 0..size.y as i32 {
            for x in 0..size.x as i32 {
                if room.tile(x, y) == Tile::Wall {
                    note(def.region_at(x, y));
                }
            }
        }
    }
    Palette {
        stone: stone.unwrap_or_else(|| wall_style(Region::Approach).1),
        foliage: foliage.unwrap_or_else(|| wall_style(Region::Forest).1),
    }
}

/// Hauteur de la flamme d'une torche sur son mur, en unités du monde.
pub const TORCH_HEIGHT: f32 = 18.0;

/// Une torche fixée sur le flanc d'un mur de pierre.
#[derive(Clone, Copy, Debug)]
pub struct Torch {
    /// Où est la flamme sur le sol (monde 2D), un peu en avant du mur.
    pub at: Vec2,
    /// La direction du mur vers le sol (monde 2D).
    pub normal: Vec2,
}

/// Les torches du niveau.
#[derive(Resource, Default)]
pub struct Torches(pub Vec<Torch>);

fn hash(x: i32, y: i32, d: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ (d as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Où poser des torches: sur les flancs des murs de pierre qui donnent sur du sol, quelques-unes seulement et jamais trop près l'une de
/// l'autre. Le choix ne dépend que de la carte: tous les clients posent les mêmes.
pub fn torch_sites(room: &Room, kinds: &[u8]) -> Vec<Torch> {
    const SPACING: f32 = 7.0;
    let size = room.size();
    let mut found: Vec<(IVec2, Torch)> = Vec::new();
    for y in 0..size.y as i32 {
        for x in 0..size.x as i32 {
            if kinds[(y as u32 * size.x + x as u32) as usize] != 1 {
                continue;
            }
            for (d, (dx, dy)) in [(0, -1), (1, 0), (-1, 0), (0, 1)].into_iter().enumerate() {
                let (nx, ny) = (x + dx, y + dy);
                let inside = nx >= 0 && ny >= 0 && nx < size.x as i32 && ny < size.y as i32;
                if !inside || room.tile(nx, ny) != Tile::Floor || hash(x, y, d as i32) % 26 != 0 {
                    continue;
                }
                let tile = IVec2::new(x, y);
                if found.iter().any(|(other, _)| other.as_vec2().distance(tile.as_vec2()) < SPACING) {
                    continue;
                }
                let normal = Vec2::new(dx as f32, dy as f32);
                let wall = room.tile_center(x as u32, y as u32);
                found.push((tile, Torch { at: wall + normal * (TILE_SIZE / 2.0 + 1.5), normal }));
            }
        }
    }
    found.into_iter().map(|(_, torch)| torch).collect()
}

/// Les lampes à donner au shader: position (x, hauteur, y du monde 2D, rayon) et couleur (rvb, intensité).
#[derive(Resource, Clone, Default)]
pub struct Lights {
    pub pos: [Vec4; MAX_LIGHTS],
    pub color: [Vec4; MAX_LIGHTS],
    pub count: f32,
}

/// Les lanternes des héros (le premier est celui que l'on dirige), puis les torches les plus proches de `focus`. Dans le noir du château,
/// les lanternes sont plus grandes et plus vives: c'est elles qui font voir.
pub fn lights(heroes: &[Vec2], torches: &[Torch], focus: Vec2, light: &LevelLight) -> Lights {
    let dark = light.mood().darkness();
    let mut pos = [Vec4::ZERO; MAX_LIGHTS];
    let mut color = [Vec4::ZERO; MAX_LIGHTS];
    let mut count = 0;
    for (i, hero) in heroes.iter().take(4).enumerate() {
        let weight = if i == 0 { 1.0 } else { 0.6 };
        pos[count] = Vec4::new(hero.x, super::LANTERN_HEIGHT, hero.y, (95.0 + 39.0 * dark) * light.lantern_radius);
        color[count] = Vec3::from(light.lantern).extend((0.2125 + 0.1875 * dark) * light.lantern_intensity * weight);
        count += 1;
    }
    let mut near: Vec<(f32, &Torch)> = torches.iter().map(|torch| (torch.at.distance(focus), torch)).filter(|(d, _)| *d < 300.0).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, torch) in near.into_iter().take(MAX_LIGHTS - count) {
        // La lampe est un peu en avant du mur, pour éclairer le mur et le sol.
        let at = torch.at + torch.normal * 4.0;
        pos[count] = Vec4::new(at.x, super::TORCH_LIGHT_HEIGHT, at.y, 110.0);
        color[count] = Vec3::from(light.torch).extend(1.6);
        count += 1;
    }
    Lights { pos, color, count: count as f32 }
}

/// La lumière qui arrive sur un point du monde, pour les sprites plats qui ne passent pas par le shader du décor: l'ambiance du niveau
/// (très faible la nuit), plus ce que donnent les lampes (lanternes des héros, torches). Rien d'autre ne les éclaire. `at` est
/// (x, hauteur, y du monde 2D); `flat` est vrai pour un sprite couché sur le sol (il reçoit le soleil, comme le sol), faux pour un sprite
/// debout (face à la caméra, il ne reçoit que le plancher de lumière des faces dans l'ombre).
pub fn light_on(at: Vec3, flat: bool, lights: &Lights, mood: Mood) -> Vec3 {
    let sky = if flat {
        mood.ambient + (1.0 - mood.ambient) * mood.sun
    } else {
        mood.ambient + 0.17 * ((mood.sun - 0.08) / 0.12).clamp(0.0, 1.0)
    };
    let mut tone = Vec3::splat(sky);
    for i in 0..(lights.count as usize).min(MAX_LIGHTS) {
        let (lamp, color) = (lights.pos[i], lights.color[i]);
        let to_lamp = Vec3::new(lamp.x, lamp.y, lamp.z) - at;
        let dist = to_lamp.length().max(0.001);
        let falloff = (1.0 - dist / lamp.w).clamp(0.0, 1.0);
        // Un sprite couché reçoit la lumière selon la hauteur de la lampe au-dessus de lui; un sprite debout, en moyenne la moitié.
        let facing = if flat { (to_lamp.y / dist).max(0.0) } else { 0.5 };
        tone += color.truncate() * color.w * falloff * falloff * (0.35 + 0.65 * facing) * 0.9;
    }
    tone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_lighting_colours_the_elves_green_and_the_forge_red() {
        let tuning: LightTuning = ron::from_str(&std::fs::read_to_string(LightTuning::file()).unwrap()).unwrap();
        let elves = tuning.light(6, Region::Marsh);
        assert!(elves.tint[1] > elves.tint[0] && elves.tint[1] > elves.tint[2]);
        let forge = tuning.light(15, Region::Forge);
        assert!(forge.tint[0] > forge.tint[2] && forge.torch[0] > 3.0 * forge.torch[1]);
        assert_eq!(tuning.light(1, Region::Forest).tint, NO_TINT);
    }

    #[test]
    fn the_tuning_survives_being_written_and_read_back() {
        let mut tuning = LightTuning::default();
        let mut level = tuning.light(3, Region::Approach);
        level.sun = 0.5;
        level.hero.gain = 3.0;
        level.lantern_radius = 1.3;
        tuning.set_level(3, level);
        let text = ron::ser::to_string_pretty(&tuning, ron::ser::PrettyConfig::default()).unwrap();
        let back: LightTuning = ron::from_str(&text).unwrap();
        assert_eq!(back, tuning);
        assert_eq!(back.light(3, Region::Approach).sun, 0.5);
        // Un niveau sans réglage propre prend l'ambiance de sa région, celle du code.
        assert_eq!(back.light(5, Region::Crypts).ambient, Mood::of(Region::Crypts).ambient);
        assert_eq!(back.light(5, Region::Crypts).sun, 0.0);
    }

    #[test]
    fn a_partial_file_fills_in_the_missing_values() {
        let back: LightTuning = ron::from_str("(hero: (gain: 4.0))").unwrap();
        assert_eq!(back.hero.gain, 4.0);
        assert_eq!(back.hero.glow, 0.0);
        assert_eq!(back.lantern_intensity, 1.0);
        assert_eq!(back.enemy, Response::default());
        // Le format d'avant les niveaux (réglages par région, lanternes et réponses communes) s'applique à tous les niveaux sans réglage propre.
        let old: LightTuning = ron::from_str("(moods: {\"Forest\": (0.0, 0.1)}, lantern_intensity: 2.0, hero: (gain: 1.0))").unwrap();
        let level = old.light(1, Region::Forest);
        assert_eq!((level.ambient, level.sun, level.lantern_intensity, level.hero.gain), (0.0, 0.1, 2.0, 1.0));
    }
}
