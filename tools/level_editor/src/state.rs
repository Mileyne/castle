//! L'état de l'édition: le niveau en cours, son historique, et les opérations de peinture.

use std::path::PathBuf;

use bevy::prelude::*;
use castle_shared::{DecorKind, LevelDef, Region, RoomKind, level_file};

use super::items::Item;

/// Nombre de niveaux gardés pour « Annuler ».
const HISTORY: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Wall,
    Floor,
    /// Eau: infranchissable, sauf sur un pont.
    Water,
    /// Pont: une tuile praticable sur l'eau.
    Bridge,
    /// Pose des éléments de décor au sol (herbes, cailloux, flaques...).
    Decor,
    Place,
    /// Sélectionne un élément (ses réglages et ses liens s'affichent à droite) et le déplace en glissant.
    Move,
    Erase,
    Zone,
}

impl Tool {
    /// Les outils qui peignent les tuiles (pinceau, rectangle, remplissage).
    pub fn paints(self) -> bool {
        matches!(self, Tool::Wall | Tool::Floor | Tool::Water | Tool::Bridge)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Brush,
    Rect,
    Fill,
}

/// Les tuiles que l'outil Matière repeint.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StyleTarget {
    Both,
    Floor,
    Walls,
}

/// Ce qu'un coup de pinceau dépose sur une tuile.
#[derive(Clone, Copy)]
pub enum Paint {
    /// Le terrain (`#`, `.`, `~`, `=`), avec, quand il y en a une, la matière (région) que prennent aussi les tuiles.
    Terrain(char, Option<Region>),
    /// La matière seule: le terrain ne change pas.
    Style(Region, StyleTarget),
}

/// Les réglages des outils, et ce que l'utilisateur est en train de faire avec.
#[derive(Resource)]
pub struct Tools {
    pub tool: Tool,
    pub shape: Shape,
    pub brush: i32,
    pub item: Item,
    pub zone_kind: RoomKind,
    /// La matière que prennent les tuiles peintes (mur, sol): celle de la région, ou celle du niveau si `None`.
    pub material: Option<Region>,
    /// Peindre la matière seule, sans changer le terrain (mur ou sol).
    pub style_only: bool,
    pub decor_kind: DecorKind,
    pub show_links: bool,
    /// La tuile où le geste en cours a commencé (rectangle, zone).
    pub drag: Option<(i32, i32)>,
    /// Dernière tuile peinte pendant un trait (pour relier les points sans trou).
    pub last_paint: Option<(i32, i32)>,
    /// Élément en cours de déplacement.
    pub moving: Option<usize>,
    /// Zone en cours de redimensionnement: son rang et les bords saisis (gauche, droite, bas, haut).
    pub resizing: Option<(usize, [bool; 4])>,
    pub last_decor: Option<Vec2>,
    /// Le point du sol sous le pointeur, en tuiles (`y` vers le haut).
    pub hover: Option<Vec2>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            tool: Tool::Move,
            shape: Shape::Brush,
            brush: 1,
            item: Item::Monster(castle_shared::MonsterKind::Skeleton),
            zone_kind: RoomKind::Combat,
            material: None,
            style_only: false,
            decor_kind: DecorKind::Grass,
            show_links: true,
            drag: None,
            last_paint: None,
            moving: None,
            resizing: None,
            last_decor: None,
            hover: None,
        }
    }
}

/// Où sont les panneaux à l'écran (en points): egui ne les compte pas comme des zones, parce qu'ils sont posés sur son fond, donc
/// ni la caméra ni les outils ne doivent réagir au pointeur qui est dessus.
#[derive(Resource, Default)]
pub struct PanelRects(pub Vec<bevy_egui::egui::Rect>);

impl PanelRects {
    pub fn contain(&self, cursor: Vec2) -> bool {
        self.0.iter().any(|rect| rect.contains(bevy_egui::egui::pos2(cursor.x, cursor.y)))
    }
}

/// Le niveau en cours d'édition.
#[derive(Resource)]
pub struct Editing {
    pub dir: PathBuf,
    pub level: u32,
    pub def: Option<LevelDef>,
    pub message: String,
    /// Modifié depuis le dernier enregistrement.
    pub dirty: bool,
    pub undo: Vec<LevelDef>,
    pub redo: Vec<LevelDef>,
    pub problems: Vec<String>,
    /// L'élément sélectionné (rang dans `things`).
    pub picked: Option<usize>,
    /// Le décor 3D ne montre plus le niveau tel qu'il est: il faut le refaire.
    pub stale: bool,
    /// La caméra doit se recentrer sur le niveau.
    pub recenter: bool,
}

impl Editing {
    pub fn new(dir: PathBuf, level: u32) -> Self {
        let mut editing = Self {
            dir,
            level,
            def: None,
            message: String::new(),
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            problems: Vec::new(),
            picked: None,
            stale: true,
            recenter: true,
        };
        editing.load(level);
        editing
    }

    fn path(&self, level: u32) -> PathBuf {
        self.dir.join(level_file(level))
    }

    pub fn load(&mut self, level: u32) {
        self.level = level;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.picked = None;
        self.stale = true;
        self.recenter = true;
        let text = std::fs::read_to_string(self.path(level)).unwrap_or_default();
        match LevelDef::parse(&text) {
            Ok(def) => {
                self.problems = def.validate();
                self.def = Some(def);
                self.message.clear();
            }
            Err(err) => {
                self.def = None;
                self.problems.clear();
                self.message = format!("Niveau {level}: {err}");
            }
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(def) = &self.def else {
            return Ok(());
        };
        if !self.dirty {
            return Ok(());
        }
        std::fs::write(self.path(self.level), def.to_ron()).map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(())
    }

    /// Garde l'état avant une modification, pour « Annuler ».
    pub fn checkpoint(&mut self) {
        if let Some(def) = &self.def {
            self.undo.push(def.clone());
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Le niveau a changé: à enregistrer, à vérifier, à redessiner.
    pub fn edited(&mut self) {
        self.dirty = true;
        self.stale = true;
        if let Some(def) = &self.def {
            self.problems = def.validate();
        }
    }

    pub fn undo(&mut self) {
        if let (Some(previous), Some(def)) = (self.undo.pop(), self.def.as_mut()) {
            self.redo.push(std::mem::replace(def, previous));
            self.picked = None;
            self.edited();
        }
    }

    pub fn redo(&mut self) {
        if let (Some(next), Some(def)) = (self.redo.pop(), self.def.as_mut()) {
            self.undo.push(std::mem::replace(def, next));
            self.picked = None;
            self.edited();
        }
    }

    /// Peint une tuile. Vrai si elle a changé.
    pub fn paint_tile(&mut self, paint: Paint, x: i32, y: i32) -> bool {
        let Some(def) = self.def.as_mut() else {
            return false;
        };
        if !in_map(def, x, y) {
            return false;
        }
        match paint {
            Paint::Style(region, target) => {
                let wall = cell(def, x, y) == '#';
                if def.region_at(x, y) == region || (target == StyleTarget::Floor && wall) || (target == StyleTarget::Walls && !wall) {
                    return false;
                }
                def.set_region_at(x, y, region);
                true
            }
            Paint::Terrain(c, region) => {
                let row = (def.size.y as i32 - 1 - y) as usize;
                let mut chars: Vec<char> = def.rows[row].chars().collect();
                let mut changed = false;
                if chars[x as usize] != c {
                    chars[x as usize] = c;
                    def.rows[row] = chars.into_iter().collect();
                    changed = true;
                }
                if let Some(region) = region.filter(|r| def.region_at(x, y) != *r) {
                    def.set_region_at(x, y, region);
                    changed = true;
                }
                changed
            }
        }
    }

    /// Peint un carré de côté `size` centré (autant que possible) sur la tuile.
    pub fn paint_brush(&mut self, paint: Paint, x: i32, y: i32, size: i32) -> bool {
        let (lo, hi) = (size / 2, size - 1 - size / 2);
        let mut changed = false;
        for dy in -lo..=hi {
            for dx in -lo..=hi {
                changed |= self.paint_tile(paint, x + dx, y + dy);
            }
        }
        changed
    }

    pub fn paint_rect(&mut self, paint: Paint, from: (i32, i32), to: (i32, i32)) -> bool {
        let mut changed = false;
        for y in from.1.min(to.1)..=from.1.max(to.1) {
            for x in from.0.min(to.0)..=from.0.max(to.0) {
                changed |= self.paint_tile(paint, x, y);
            }
        }
        changed
    }

    /// Remplit la zone d'une même nature (et, pour la matière, d'une même région) autour de la tuile.
    pub fn paint_fill(&mut self, paint: Paint, x: i32, y: i32) -> bool {
        let Some(def) = self.def.as_ref() else {
            return false;
        };
        // On remplit ce qui est de la même nature, et de la même matière quand la peinture en change.
        let styled = matches!(paint, Paint::Style(..) | Paint::Terrain(_, Some(_)));
        let key = |def: &LevelDef, x: i32, y: i32| (cell(def, x, y), styled.then(|| def.region_at(x, y)));
        let from = key(def, x, y);
        let already = match paint {
            Paint::Style(region, _) => from.1 == Some(region),
            Paint::Terrain(c, None) => from.0 == c,
            Paint::Terrain(c, Some(region)) => from.0 == c && from.1 == Some(region),
        };
        if already || !in_map(def, x, y) {
            return false;
        }
        let mut tiles = Vec::new();
        let mut stack = vec![(x, y)];
        let mut seen = std::collections::HashSet::new();
        while let Some((cx, cy)) = stack.pop() {
            if !in_map(def, cx, cy) || key(def, cx, cy) != from || !seen.insert((cx, cy)) {
                continue;
            }
            tiles.push((cx, cy));
            stack.extend([(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
        }
        let mut changed = false;
        for (cx, cy) in tiles {
            changed |= self.paint_tile(paint, cx, cy);
        }
        changed
    }
}

pub fn in_map(def: &LevelDef, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < def.size.x as i32 && y < def.size.y as i32
}

pub fn cell(def: &LevelDef, x: i32, y: i32) -> char {
    if !in_map(def, x, y) {
        return '#';
    }
    def.rows[(def.size.y as i32 - 1 - y) as usize].as_bytes()[x as usize] as char
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editing() -> Editing {
        let mut editing = Editing::new(PathBuf::from("/nonexistent"), 1);
        editing.def = Some(LevelDef::blank("essai", Region::Forest, UVec2::new(20, 14)));
        editing
    }

    #[test]
    fn painting_fills_undoes_and_redoes() {
        let mut editing = editing();
        editing.checkpoint();
        assert!(editing.paint_brush(Paint::Terrain('~', None), 5, 5, 2));
        assert_eq!(editing.def.as_ref().unwrap().rows[14 - 1 - 5].as_bytes()[5], b'~');
        // Une tuile déjà de cette nature ne change pas.
        assert!(!editing.paint_tile(Paint::Terrain('~', None), 5, 5));
        editing.checkpoint();
        assert!(editing.paint_fill(Paint::Terrain('~', None), 10, 7));
        assert!(editing.def.as_ref().unwrap().rows.iter().skip(1).take(12).all(|r| r[1..19].chars().all(|c| c == '~')));
        editing.undo();
        assert_eq!(editing.def.as_ref().unwrap().rows[14 - 1 - 7].as_bytes()[10], b'.');
        editing.redo();
        assert_eq!(editing.def.as_ref().unwrap().rows[14 - 1 - 7].as_bytes()[10], b'~');
    }

    #[test]
    fn a_style_filter_leaves_the_other_tiles_alone() {
        let mut editing = editing();
        // Le sol seulement: un mur ne change pas de matière.
        assert!(!editing.paint_tile(Paint::Style(Region::Crypts, StyleTarget::Floor), 0, 0));
        assert!(editing.paint_tile(Paint::Style(Region::Crypts, StyleTarget::Floor), 5, 5));
        assert_eq!(editing.def.as_ref().unwrap().region_at(5, 5), Region::Crypts);
        assert!(editing.paint_tile(Paint::Style(Region::Crypts, StyleTarget::Walls), 0, 0));
    }
}
