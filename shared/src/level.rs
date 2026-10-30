//! Niveaux de la campagne, écrits à la main (ou exportés d'un château généré, puis retouchés dans l'éditeur).
//!
//! Un niveau est un fichier RON: la grille de tuiles en texte, les zones (salles, couloirs, portes) et la liste des
//! éléments posés (monstres, générateurs, objets, énigme...). Il est compilé dans le programme: le serveur et les clients
//! lisent exactement la même carte. Les positions sont en tuiles (`x` vers la droite, `y` vers le haut; le centre de la
//! tuile (i, j) est (i + 0,5, j + 0,5)).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Dungeon, ExitKind, InteractKind, Layout, MonsterKind, PickupKind, QuestKind, Region, Room, RoomKind, TILE_SIZE, Tile,
};

/// Version du format des fichiers de niveau.
pub const LEVEL_FORMAT: u32 = 1;

/// Un élément posé dans un niveau.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Thing {
    Monster { kind: MonsterKind, at: Vec2 },
    Generator { at: Vec2 },
    Pickup { kind: PickupKind, at: Vec2 },
    Trap { at: Vec2 },
    /// Relique de la quête de récupération.
    Relic { at: Vec2 },
    /// Prisonnier de la quête de sauvetage.
    Captive { at: Vec2 },
    /// Enchantement qui tient à distance de la porte royale.
    Ward { at: Vec2 },
    /// Herse devant la porte royale; `vertical` quand elle s'étend en hauteur.
    Portcullis { at: Vec2, vertical: bool },
    /// Panneau de l'énigme de la porte royale (dans le mur), avec la scène qu'il déclenche.
    Panel { kind: InteractKind, at: Vec2, node: String },
    /// Sortie du niveau. Sans elle, la sortie est au centre de la salle de sortie.
    Exit { kind: ExitKind, at: Vec2 },
    /// Scène de dialogue déclenchée quand un héros s'approche.
    Trigger { node: String, at: Vec2, radius: f32, delay: f32 },
}

impl Thing {
    /// Position de l'élément, en tuiles.
    pub fn at(&self) -> Vec2 {
        match self {
            Self::Monster { at, .. }
            | Self::Generator { at }
            | Self::Pickup { at, .. }
            | Self::Trap { at }
            | Self::Relic { at }
            | Self::Captive { at }
            | Self::Ward { at }
            | Self::Portcullis { at, .. }
            | Self::Panel { at, .. }
            | Self::Exit { at, .. }
            | Self::Trigger { at, .. } => *at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelDef {
    pub format: u32,
    /// Nom pour l'éditeur (le joueur ne voit que des clés de traduction).
    pub name: String,
    pub region: Region,
    pub size: UVec2,
    /// Les tuiles, une chaîne par ligne, de haut en bas: `#` mur, `.` sol.
    pub rows: Vec<String>,
    /// Salles, couloirs et portes (voir `Layout`).
    pub layout: Layout,
    /// Objectif du niveau: genre et nombre à atteindre.
    pub quest: Option<(QuestKind, u32)>,
    pub things: Vec<Thing>,
}

impl LevelDef {
    /// Position dans le monde d'un point en tuiles.
    pub fn world(&self, at: Vec2) -> Vec2 {
        Self::world_in(self.size, at)
    }

    pub fn world_in(size: UVec2, at: Vec2) -> Vec2 {
        (at - size.as_vec2() / 2.0) * TILE_SIZE
    }

    /// Position en tuiles d'un point du monde (l'inverse de `world_in`).
    pub fn tile_in(size: UVec2, world: Vec2) -> Vec2 {
        world / TILE_SIZE + size.as_vec2() / 2.0
    }

    /// Un niveau vide pour commencer: un terrain ouvert entouré de murs, avec une zone de départ à gauche et une zone de
    /// sortie à droite.
    pub fn blank(name: &str, region: Region, size: UVec2) -> Self {
        use crate::{Connection, RoomInfo};
        let (w, h) = (size.x.max(16), size.y.max(12));
        let rows = (0..h)
            .map(|row| (0..w).map(|x| if row == 0 || row == h - 1 || x == 0 || x == w - 1 { '#' } else { '.' }).collect())
            .collect();
        let zone = |x: u32, kind| RoomInfo { min: UVec2::new(x, h / 2 - 3), max: UVec2::new(x + 5, h / 2 + 2), kind };
        Self {
            format: LEVEL_FORMAT,
            name: name.to_string(),
            region,
            size: UVec2::new(w, h),
            rows,
            layout: Layout {
                rooms: vec![zone(2, RoomKind::Start), zone(w - 8, RoomKind::Exit)],
                connections: Vec::<Connection>::new(),
                start: 0,
                exit: 1,
                treasure: None,
            },
            quest: None,
            things: Vec::new(),
        }
    }

    /// Décrit un château généré: ses tuiles et ses zones, avec les éléments donnés.
    pub fn from_dungeon(
        name: &str,
        region: Region,
        dungeon: &Dungeon,
        quest: Option<(QuestKind, u32)>,
        things: Vec<Thing>,
    ) -> Self {
        let size = dungeon.room.size();
        let rows = (0..size.y as i32)
            .rev()
            .map(|y| {
                (0..size.x as i32)
                    .map(|x| if dungeon.room.tile(x, y) == Tile::Wall { '#' } else { '.' })
                    .collect()
            })
            .collect();
        Self {
            format: LEVEL_FORMAT,
            name: name.to_string(),
            region,
            size,
            rows,
            layout: dungeon.layout.clone(),
            quest,
            things,
        }
    }

    /// La carte et les zones du niveau, telles que le jeu les utilise.
    pub fn to_dungeon(&self) -> Result<Dungeon, String> {
        let problems = self.check_grid();
        if let Some(problem) = problems.first() {
            return Err(problem.clone());
        }
        let mut tiles = vec![Tile::Wall; (self.size.x * self.size.y) as usize];
        for (row, line) in self.rows.iter().enumerate() {
            let y = self.size.y as usize - 1 - row;
            for (x, c) in line.chars().enumerate() {
                tiles[y * self.size.x as usize + x] = if c == '#' { Tile::Wall } else { Tile::Floor };
            }
        }
        Ok(Dungeon { room: Room { size: self.size, tiles }, layout: self.layout.clone() })
    }

    /// Problèmes de forme de la grille (dimensions, caractères).
    fn check_grid(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.size.x == 0 || self.size.y == 0 {
            problems.push("la taille du niveau est nulle".to_string());
        }
        if self.rows.len() != self.size.y as usize {
            problems.push(format!("{} lignes de tuiles pour une hauteur de {}", self.rows.len(), self.size.y));
        }
        for (i, line) in self.rows.iter().enumerate() {
            if line.chars().count() != self.size.x as usize {
                problems.push(format!("la ligne {} fait {} tuiles pour une largeur de {}", i + 1, line.chars().count(), self.size.x));
            }
            if let Some(c) = line.chars().find(|c| !matches!(c, '#' | '.')) {
                problems.push(format!("la ligne {} contient le caractère inconnu '{c}'", i + 1));
            }
        }
        problems
    }

    /// Vérifie qu'un niveau est jouable: grille correcte, zones valides, éléments sur le sol, sortie atteignable depuis
    /// le départ. Renvoie les problèmes trouvés (vide si tout va bien).
    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.check_grid();
        if !problems.is_empty() {
            return problems;
        }
        let rooms = &self.layout.rooms;
        if self.layout.start >= rooms.len() || self.layout.exit >= rooms.len() {
            problems.push("la salle de départ ou de sortie n'existe pas".to_string());
            return problems;
        }
        for (i, room) in rooms.iter().enumerate() {
            if room.max.x >= self.size.x || room.max.y >= self.size.y || room.min.x > room.max.x || room.min.y > room.max.y {
                problems.push(format!("la salle {i} sort de la carte"));
            }
        }
        let Ok(dungeon) = self.to_dungeon() else {
            return problems;
        };
        for thing in &self.things {
            // Les panneaux de l'énigme sont dans le mur; tout le reste est posé sur le sol.
            if matches!(thing, Thing::Panel { .. }) {
                continue;
            }
            let at = thing.at();
            if dungeon.room.tile(at.x.floor() as i32, at.y.floor() as i32) != Tile::Floor {
                problems.push(format!("{thing:?} n'est pas sur le sol"));
            }
        }
        // Le départ et la sortie communiquent (les portes comptent comme ouvertes).
        let (start, exit) = (rooms[self.layout.start].center(), rooms[self.layout.exit].center());
        let mut seen = vec![false; (self.size.x * self.size.y) as usize];
        let mut stack = vec![start.as_ivec2()];
        while let Some(t) = stack.pop() {
            if dungeon.room.tile(t.x, t.y) != Tile::Floor {
                continue;
            }
            let i = (t.y as u32 * self.size.x + t.x as u32) as usize;
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            stack.extend([t + IVec2::X, t - IVec2::X, t + IVec2::Y, t - IVec2::Y]);
        }
        if !seen[(exit.y * self.size.x + exit.x) as usize] {
            problems.push("la sortie n'est pas atteignable depuis le départ".to_string());
        }
        if rooms[self.layout.start].kind != RoomKind::Start {
            problems.push("la salle de départ n'est pas de genre Start".to_string());
        }
        problems
    }

    /// Lit un niveau écrit en RON.
    pub fn parse(text: &str) -> Result<Self, String> {
        let level: Self = ron::from_str(text).map_err(|e| format!("niveau illisible: {e}"))?;
        if level.format != LEVEL_FORMAT {
            return Err(format!("format de niveau {} non géré (attendu {LEVEL_FORMAT})", level.format));
        }
        Ok(level)
    }

    /// Écrit le niveau en RON lisible.
    pub fn to_ron(&self) -> String {
        let config = ron::ser::PrettyConfig::new().depth_limit(6).indentor("  ".to_string());
        ron::ser::to_string_pretty(self, config).expect("un niveau se sérialise toujours")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_castle_survives_the_level_file_round_trip() {
        for seed in 0..20u64 {
            let dungeon = Dungeon::generate(seed);
            let things = vec![
                Thing::Generator { at: Vec2::new(30.5, 20.5) },
                Thing::Panel { kind: InteractKind::Rune, at: Vec2::new(5.5, 5.5), node: "puzzle_rune".into() },
            ];
            let def = LevelDef::from_dungeon("essai", Region::Crypts, &dungeon, Some((QuestKind::Hunt, 6)), things);
            let text = def.to_ron();
            let back = LevelDef::parse(&text).expect("relu");
            let rebuilt = back.to_dungeon().expect("carte");
            assert_eq!(rebuilt.room.size(), dungeon.room.size());
            for y in 0..dungeon.room.size().y as i32 {
                for x in 0..dungeon.room.size().x as i32 {
                    assert_eq!(rebuilt.room.tile(x, y), dungeon.room.tile(x, y), "seed {seed} tuile {x},{y}");
                }
            }
            assert_eq!(back.things, def.things);
            assert_eq!(back.layout.rooms.len(), dungeon.layout.rooms.len());
            // Les deux éléments ci-dessus ne sont pas forcément sur le sol: seule la forme compte ici.
            assert!(back.check_grid().is_empty(), "seed {seed}");
        }
    }

    #[test]
    fn validation_reports_a_broken_grid_and_an_unreachable_exit() {
        let dungeon = Dungeon::generate(3);
        let mut def = LevelDef::from_dungeon("essai", Region::Crypts, &dungeon, None, Vec::new());
        assert!(def.validate().is_empty(), "{:?}", def.validate());
        def.rows[0].push('.');
        assert!(def.validate().iter().any(|p| p.contains("largeur")));
        def.rows[0].pop();
        // On mure toute la carte autour de la sortie.
        let exit = dungeon.layout.rooms[dungeon.layout.exit].clone();
        for row in 0..def.rows.len() {
            let y = def.size.y - 1 - row as u32;
            if y >= exit.min.y && y <= exit.max.y {
                let mut chars: Vec<char> = def.rows[row].chars().collect();
                for x in exit.min.x.saturating_sub(1)..=exit.max.x + 1 {
                    if x < def.size.x && !(x >= exit.min.x && x <= exit.max.x && y >= exit.min.y && y <= exit.max.y) {
                        chars[x as usize] = '#';
                    }
                }
                def.rows[row] = chars.into_iter().collect();
            }
        }
        assert!(def.validate().iter().any(|p| p.contains("atteignable")), "{:?}", def.validate());
    }

    #[test]
    fn world_and_tile_positions_are_inverse() {
        let size = UVec2::new(88, 48);
        let at = Vec2::new(12.5, 30.25);
        assert!((LevelDef::tile_in(size, LevelDef::world_in(size, at)) - at).length() < 1e-4);
    }
}
