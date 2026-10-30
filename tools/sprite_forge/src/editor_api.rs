//! Ce que l'éditeur voit de la génération: la liste des modèles, leurs réglages possibles, un aperçu rendu avec les
//! mêmes fonctions que les planches du jeu, et les tuiles telles que le générateur les produit.

use std::collections::BTreeMap;

use crate::heroes::{CLASSES, Class, Pose};
use crate::monsters::{MONSTERS, Monster};
use crate::props::{PROPS, Prop};
use crate::render::{Part, render};
use crate::shapes::Theme;
use crate::tuning::{self, Tuning, hex_of};
use crate::{CELL, SUPERSAMPLE, TILE_CELL, tiles, to_rgba8};

/// Côté d'une image de sprite, en pixels.
pub const SPRITE_PX: usize = CELL;
/// Nombre de directions de vue des personnages.
pub const DIRECTION_COUNT: usize = crate::DIRECTIONS;
/// Nombre de régions du château.
pub const REGIONS: usize = tiles::REGIONS;

/// Un modèle dont l'éditeur sait régler et montrer le rendu.
#[derive(Clone, Debug)]
pub struct ModelInfo {
    /// Clé des réglages (`hero:Knight`...).
    pub key: String,
    pub label: &'static str,
    pub group: &'static str,
    /// Vrai pour un personnage (8 directions); faux pour un objet, toujours vu de face.
    pub oriented: bool,
}

enum Model {
    Hero(Class),
    Monster(Monster),
    Prop(Prop),
}

fn hero_label(class: Class) -> &'static str {
    match class {
        Class::Knight => "Chevalier",
        Class::Sorcerer => "Ensorceleur",
        Class::Ranger => "Rôdeur",
        Class::Forgebraise => "Forgebraise",
    }
}

fn monster_label(monster: Monster) -> &'static str {
    match monster {
        Monster::Skeleton => "Squelette",
        Monster::Bat => "Chauve-souris",
        Monster::Goblin => "Gobelin",
        Monster::Orc => "Orque",
        Monster::Ghoul => "Goule",
        Monster::Wolf => "Loup",
    }
}

fn prop_label(prop: Prop) -> &'static str {
    match prop {
        Prop::Generator => "Générateur de monstres",
        Prop::Turret => "Tourelle",
        Prop::Coin => "Pièce",
        Prop::Food => "Nourriture",
        Prop::Key => "Clé",
        Prop::Lever => "Levier",
        Prop::Rune => "Rune",
        Prop::Mechanism => "Mécanisme",
        Prop::TrapArmed => "Piège armé",
        Prop::TrapOff => "Piège désarmé",
        Prop::DoorLocked => "Porte à clé",
        Prop::DoorRoyal => "Porte royale",
        Prop::DoorRoyalEmblem => "Emblème de la porte royale",
        Prop::Stairs => "Escalier de sortie",
        Prop::Potion => "Potion de régénération",
        Prop::WallRune => "Panneau de rune (violet, Ensorceleur)",
        Prop::WallGearsJammed => "Mécanisme grippé (orange, Forgebraise)",
        Prop::WallGearsFixed => "Mécanisme réparé (orange, Forgebraise)",
        Prop::WallTargetEmpty => "Support de cible vide (vert, Rôdeur)",
        Prop::WallTargetMounted => "Cible garnie (vert, Rôdeur)",
        Prop::WallTargetHit => "Cible touchée (vert, Rôdeur)",
        Prop::Portcullis => "Herse (une moitié)",
        Prop::GateClosed => "Barrière du sentier (fermée)",
        Prop::GateOpen => "Barrière du sentier (ouverte)",
        Prop::BridgeBroken => "Pont brisé",
        Prop::BridgeRepaired => "Pont réparé",
        Prop::DrawbridgeRaised => "Pont-levis levé",
        Prop::DrawbridgeLowered => "Pont-levis baissé",
        Prop::CaptiveCage => "Prisonnier en cage",
        Prop::Ingredient => "Ingrédient (violet)",
        Prop::GearPart => "Pièce de rechange (orange)",
        Prop::TargetDisc => "Disque de cible (vert)",
    }
}

/// Tous les modèles, regroupés: héros, monstres, objets.
pub fn models() -> Vec<ModelInfo> {
    let mut all = Vec::new();
    for class in CLASSES {
        all.push(ModelInfo { key: tuning::hero_key(class), label: hero_label(class), group: "Héros", oriented: true });
    }
    for monster in MONSTERS {
        all.push(ModelInfo { key: tuning::monster_key(monster), label: monster_label(monster), group: "Monstres", oriented: true });
    }
    for prop in PROPS {
        all.push(ModelInfo { key: tuning::prop_key(prop), label: prop_label(prop), group: "Objets", oriented: false });
    }
    all
}

fn parse(key: &str) -> Option<Model> {
    CLASSES
        .iter()
        .find(|c| tuning::hero_key(**c) == key)
        .map(|c| Model::Hero(*c))
        .or_else(|| MONSTERS.iter().find(|m| tuning::monster_key(**m) == key).map(|m| Model::Monster(*m)))
        .or_else(|| PROPS.iter().find(|p| tuning::prop_key(**p) == key).map(|p| Model::Prop(*p)))
}

/// Moment de l'animation à montrer. La valeur va de 0 à 1 (phase du cycle, ou progression du coup).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anim {
    Idle(f32),
    Walk(f32),
    Attack(f32),
}

fn pose(anim: Anim) -> Pose {
    match anim {
        Anim::Idle(t) => Pose::Idle(t),
        Anim::Walk(t) => Pose::Walk(t),
        Anim::Attack(t) => Pose::Attack(t),
    }
}

fn phase(anim: Anim) -> f32 {
    match anim {
        Anim::Idle(t) | Anim::Walk(t) | Anim::Attack(t) => t,
    }
}

fn build_parts(model: &Model, anim: Anim, theme: Theme) -> Vec<Part> {
    match model {
        Model::Hero(class) => crate::heroes::build(*class, pose(anim), theme),
        Model::Monster(monster) => crate::monsters::build(*monster, pose(anim), theme),
        Model::Prop(prop) => crate::props::build(*prop, phase(anim), theme),
    }
}

/// Réglages de proportions d'un modèle avec leur valeur d'origine (`dims.<nom>` pour le corps, `face.<nom>` pour le
/// visage). Vide pour les objets.
pub fn model_params(key: &str, theme: Theme) -> Vec<(String, f32)> {
    let Some(model) = parse(key) else {
        return Vec::new();
    };
    tuning::without_tuning(|| {
        let mut params = Vec::new();
        match model {
            Model::Hero(class) => {
                params.extend(crate::heroes::dims_of(class, theme).fields().map(|(n, v)| (format!("dims.{n}"), v)));
                if theme == Theme::Medieval {
                    params.extend(crate::heroes_medieval::face_fields(class).into_iter().map(|(n, v)| (format!("face.{n}"), v)));
                    params.extend(crate::heroes_medieval::gear_fields(class).into_iter().map(|(n, v)| (format!("gear.{n}"), v)));
                }
            }
            Model::Monster(Monster::Skeleton) => {
                params.extend(crate::monsters::skeleton_dims().fields().map(|(n, v)| (format!("dims.{n}"), v)));
            }
            Model::Monster(Monster::Goblin) => {
                params.extend(crate::monsters_more::goblin_dims().fields().map(|(n, v)| (format!("dims.{n}"), v)));
            }
            Model::Monster(Monster::Orc) => {
                params.extend(crate::monsters_more::orc_dims().fields().map(|(n, v)| (format!("dims.{n}"), v)));
            }
            Model::Monster(Monster::Ghoul) => {
                params.extend(crate::monsters_more::ghoul_dims().fields().map(|(n, v)| (format!("dims.{n}"), v)));
            }
            Model::Monster(Monster::Bat | Monster::Wolf) | Model::Prop(_) => {}
        }
        params
    })
}

/// Matières d'un modèle: chaque couleur d'origine avec son nombre de parties, les plus utilisées d'abord.
pub fn model_materials(key: &str, theme: Theme) -> Vec<(u32, usize)> {
    let Some(model) = parse(key) else {
        return Vec::new();
    };
    tuning::without_tuning(|| {
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        // Au repos pour le décompte; marche et coup pour ne rien oublier (effets, armes).
        for (anim, count) in [(Anim::Idle(0.0), true), (Anim::Walk(0.3), false), (Anim::Attack(0.5), false)] {
            for part in build_parts(&model, anim, theme) {
                let entry = counts.entry(hex_of(part.color)).or_insert(0);
                if count {
                    *entry += 1;
                }
            }
        }
        let mut list: Vec<(u32, usize)> = counts.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        list
    })
}

/// Rend un aperçu (`SPRITE_PX` x `SPRITE_PX`, RGBA) avec les réglages donnés. `highlight` montre en magenta toutes les
/// parties d'une matière (désignée par sa couleur d'origine).
pub fn render_model(key: &str, theme: Theme, anim: Anim, direction: usize, tuned: &Tuning, highlight: Option<u32>) -> Vec<u8> {
    render_model_detail(key, theme, anim, direction, tuned, highlight, 1)
}

/// Comme `render_model`, mais à `detail` fois la résolution (image de `SPRITE_PX * detail` pixels de côté): plus de
/// netteté pour travailler un visage ou un accessoire. Le jeu, lui, n'utilise que la résolution 1.
pub fn render_model_detail(
    key: &str,
    theme: Theme,
    anim: Anim,
    direction: usize,
    tuned: &Tuning,
    highlight: Option<u32>,
    detail: usize,
) -> Vec<u8> {
    let detail = detail.max(1);
    let Some(model) = parse(key) else {
        return vec![0; SPRITE_PX * SPRITE_PX * detail * detail * 4];
    };
    let mut active = tuned.clone();
    if let Some(color) = highlight {
        active.models.entry(key.to_string()).or_default().materials.entry(color).or_default().color = Some(0xff00ff);
    }
    tuning::set_active(active);
    let parts = build_parts(&model, anim, theme);
    // Même cadrage que les planches: les objets sont vus de face, les plus grands un peu plus petits.
    let mut cam = match model {
        Model::Prop(prop) => crate::prop_camera(prop),
        _ => crate::camera(direction),
    };
    let factor = detail as f32;
    cam.size *= detail;
    cam.scale *= factor;
    cam.feet_y *= factor;
    let image = to_rgba8(&render(&parts, &cam, SUPERSAMPLE));
    tuning::set_active(Tuning::default());
    image
}

/// Un groupe de tuiles de la planche: un grand motif de 4x4 tuiles (sol, dessus de mur), une bande de 4 tuiles (face
/// de mur) ou une tuile décorée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileGroup {
    Floor(usize),
    Top(usize),
    Face(usize),
    Decor(usize),
}

impl TileGroup {
    /// Tous les groupes d'une région, dans l'ordre de la planche.
    pub fn all() -> Vec<TileGroup> {
        let mut all = Vec::new();
        all.extend((0..tiles::FLOOR_PATTERNS).map(TileGroup::Floor));
        all.extend((0..tiles::TOP_PATTERNS).map(TileGroup::Top));
        all.extend((0..tiles::FACE_PATTERNS).map(TileGroup::Face));
        all.extend((0..tiles::DECOR_FACES).map(TileGroup::Decor));
        all
    }

    pub fn label(self) -> String {
        match self {
            TileGroup::Floor(v) => format!("Sol {}", v + 1),
            TileGroup::Top(v) => format!("Dessus de mur {}", v + 1),
            TileGroup::Face(v) => format!("Face de mur {}", v + 1),
            TileGroup::Decor(v) => format!("Face décorée {}", v + 1),
        }
    }

    /// Largeur et hauteur du groupe, en tuiles.
    pub fn size_in_tiles(self) -> (usize, usize) {
        match self {
            TileGroup::Floor(_) | TileGroup::Top(_) => (tiles::MACRO, tiles::MACRO),
            TileGroup::Face(_) => (tiles::MACRO, 1),
            TileGroup::Decor(_) => (1, 1),
        }
    }

    /// Colonne de la planche de la tuile (`x`, `y`) du groupe.
    pub fn column(self, x: usize, y: usize) -> usize {
        let within = y * tiles::MACRO + x;
        match self {
            TileGroup::Floor(v) => v * tiles::MACRO * tiles::MACRO + within,
            TileGroup::Top(v) => tiles::TOP_START + v * tiles::MACRO * tiles::MACRO + within,
            TileGroup::Face(v) => tiles::FACE_START + v * tiles::MACRO + x,
            TileGroup::Decor(v) => tiles::DECOR_START + v,
        }
    }
}

/// Image d'un groupe de tuiles (RGBA).
pub struct TileArt {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Le groupe tel que le générateur le dessine, sans aucune retouche.
pub fn tile_base(theme: Theme, region: usize, group: TileGroup) -> TileArt {
    let macro_px = tiles::MACRO * TILE_CELL;
    let (pixels, width, height) = match group {
        TileGroup::Floor(v) => (crate::render_floor(theme, region, v), macro_px, macro_px),
        TileGroup::Top(v) => (crate::render_roof(theme, region, v), macro_px, macro_px),
        TileGroup::Face(v) => {
            // La bande de la face occupe le centre du grand carré rendu.
            let image = crate::render_face(theme, region, v);
            let top = (macro_px - TILE_CELL) / 2;
            (image[top * macro_px..(top + TILE_CELL) * macro_px].to_vec(), macro_px, TILE_CELL)
        }
        TileGroup::Decor(v) => (crate::render_decor(theme, region, tiles::DECOR_START + v), TILE_CELL, TILE_CELL),
    };
    TileArt { width, height, rgba: to_rgba8(&pixels) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_renders_and_lists_materials() {
        let _serial = tuning::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for info in models() {
            let materials = model_materials(&info.key, Theme::Medieval);
            assert!(!materials.is_empty(), "{} sans matière", info.key);
            let image = render_model(&info.key, Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None);
            assert_eq!(image.len(), SPRITE_PX * SPRITE_PX * 4);
            assert!(image.chunks(4).any(|p| p[3] > 0), "{} invisible", info.key);
        }
    }

    #[test]
    fn tuning_changes_the_render_and_default_does_not() {
        let _serial = tuning::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = "hero:Knight";
        let plain = render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None);
        assert_eq!(plain, render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None));
        let (color, _) = model_materials(key, Theme::Medieval)[0];
        let mut tuned = Tuning::default();
        tuned.models.entry(key.into()).or_default().materials.entry(color).or_default().color = Some(0x00ff00);
        assert_ne!(plain, render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &tuned, None));
        // Une proportion réglée change aussi le rendu.
        let mut taller = Tuning::default();
        let (name, base) = model_params(key, Theme::Medieval).into_iter().find(|(n, _)| n == "dims.thigh").unwrap();
        taller.models.entry(key.into()).or_default().params.insert(name, base * 1.5);
        assert_ne!(plain, render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &taller, None));
        // Et les réglages ne restent pas actifs après l'aperçu.
        assert_eq!(plain, render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None));
    }

    #[test]
    fn headgear_can_be_removed_and_detail_renders_larger() {
        let _serial = tuning::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for key in ["hero:Knight", "hero:Sorcerer", "hero:Ranger", "hero:Forgebraise"] {
            let params = model_params(key, Theme::Medieval);
            assert_eq!(params.iter().find(|(n, _)| n == "gear.hat").map(|(_, v)| *v), Some(1.0), "{key}");
            let worn = render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None);
            let mut bare = Tuning::default();
            bare.models.entry(key.into()).or_default().params.insert("gear.hat".into(), 0.0);
            let bare = render_model(key, Theme::Medieval, Anim::Idle(0.0), 6, &bare, None);
            let opaque = |image: &[u8]| image.chunks(4).filter(|p| p[3] > 0).count();
            assert_ne!(worn, bare, "{key}: retirer le couvre-chef change l'image");
            assert!(opaque(&worn) > opaque(&bare), "{key}: sans couvre-chef la silhouette est plus petite");
        }
        // Les monstres n'ont pas ce réglage.
        for key in ["monster:Orc", "monster:Skeleton"] {
            assert!(model_params(key, Theme::Medieval).iter().all(|(n, _)| n != "gear.hat"), "{key}");
        }
        let big = render_model_detail("hero:Knight", Theme::Medieval, Anim::Idle(0.0), 6, &Tuning::default(), None, 4);
        assert_eq!(big.len(), SPRITE_PX * 4 * SPRITE_PX * 4 * 4);
    }

    #[test]
    fn tile_groups_cover_the_sheet_columns_once() {
        let mut seen = std::collections::BTreeSet::new();
        for group in TileGroup::all() {
            let (w, h) = group.size_in_tiles();
            for y in 0..h {
                for x in 0..w {
                    assert!(seen.insert(group.column(x, y)), "colonne en double: {group:?} {x},{y}");
                }
            }
        }
        assert_eq!(seen.len(), tiles::COLUMNS);
        assert_eq!(*seen.last().unwrap(), tiles::COLUMNS - 1);
    }

    #[test]
    fn tile_base_has_the_size_of_its_group() {
        for group in TileGroup::all() {
            let (w, h) = group.size_in_tiles();
            let art = tile_base(Theme::Medieval, 1, group);
            assert_eq!((art.width, art.height), (w * TILE_CELL, h * TILE_CELL), "{group:?}");
            assert_eq!(art.rgba.len(), art.width * art.height * 4);
            assert!(art.rgba.chunks(4).any(|p| p[3] > 0), "{group:?} vide");
        }
    }
}
