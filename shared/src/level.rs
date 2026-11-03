//! Niveaux de la campagne, écrits à la main (ou exportés d'un château généré, puis retouchés dans l'éditeur).
//!
//! Un niveau est un fichier RON: la grille de tuiles en texte, les zones (salles, couloirs, portes) et la liste des
//! éléments posés (monstres, générateurs, objets, énigme...). Il est compilé dans le programme: le serveur et les clients
//! lisent exactement la même carte. Les positions sont en tuiles (`x` vers la droite, `y` vers le haut; le centre de la
//! tuile (i, j) est (i + 0,5, j + 0,5)).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Dungeon, ExitKind, INGREDIENTS_NEEDED, InteractKind, Layout, MonsterKind, PickupKind, QuestKind, Region, Room, RoomKind, TILE_SIZE, Tile,
};

/// Les scènes de dialogue qu'un déclencheur ou un panneau peut jouer.
pub const DIALOGUE_NODES: [&str; 30] = [
    "prologue",
    "intro",
    "royal_door",
    "alcove",
    "puzzle_target",
    "puzzle_rune",
    "puzzle_gears",
    "bridge_intro",
    "bridge_panel",
    "drawbridge_intro",
    "drawbridge_panel",
    "gate_intro",
    "gate_panel",
    "castle_intro",
    "knight_intro",
    "knight_door",
    "boss_warg",
    "boss_troll",
    "boss_muldant",
    "samelia_freed",
    "samelia_taken",
    "elves_dawn",
    "boss_gardener",
    "samelia_rescued",
    "gate_farewell",
    "heart_reveal",
    "rope_intro",
    "rope_panel",
    "courtyard_intro",
    "courtyard_panel",
];

/// Une extrémité d'un lien entre éléments d'un niveau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEnd {
    /// Un élément de `things`, par son rang.
    Thing(usize),
    /// La sortie du niveau (l'élément `Exit` s'il y en a un, sinon la zone de sortie).
    Exit,
    /// Une porte de `layout.connections`, par son rang.
    Door(usize),
}

/// Ce qui relie deux éléments, tel que les règles du jeu le font: voir `LevelDef::links`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub from: LinkEnd,
    pub to: LinkEnd,
    pub why: &'static str,
}

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
    /// Samélia, la princesse des elfes: debout (elle parle) ou, avec `caged`, prisonnière à délivrer comme un captif.
    Guide { at: Vec2, caged: bool },
    /// Enchantement qui tient à distance de la porte royale.
    Ward { at: Vec2 },
    /// Herse devant la porte royale; `vertical` quand elle s'étend en hauteur.
    Portcullis { at: Vec2, vertical: bool },
    /// Panneau de l'énigme de la porte royale (dans le mur), avec la scène qu'il déclenche.
    Panel { kind: InteractKind, at: Vec2, node: String },
    /// Sortie du niveau. Sans elle, la sortie est au centre de la salle de sortie.
    Exit { kind: ExitKind, at: Vec2 },
    /// Obstacle qui barre un accès (un pont cassé) jusqu'à ce que l'objectif du niveau soit rempli. Ce n'est pas la sortie: elle est
    /// plus loin, dans la zone de sortie.
    Barrier { kind: ExitKind, at: Vec2 },
    /// Grand motif (arbre, rocher, colonne, fontaine...) dont le pied bloque le passage.
    Landmark { kind: crate::decor::LandmarkKind, at: Vec2 },
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
            | Self::Guide { at, .. }
            | Self::Ward { at }
            | Self::Portcullis { at, .. }
            | Self::Panel { at, .. }
            | Self::Exit { at, .. }
            | Self::Barrier { at, .. }
            | Self::Landmark { at, .. }
            | Self::Trigger { at, .. } => *at,
        }
    }

    /// Déplace l'élément (l'éditeur de niveaux).
    pub fn set_at(&mut self, to: Vec2) {
        match self {
            Self::Monster { at, .. }
            | Self::Generator { at }
            | Self::Pickup { at, .. }
            | Self::Trap { at }
            | Self::Relic { at }
            | Self::Captive { at }
            | Self::Guide { at, .. }
            | Self::Ward { at }
            | Self::Portcullis { at, .. }
            | Self::Panel { at, .. }
            | Self::Landmark { at, .. }
            | Self::Trigger { at, .. } => *at = to,
            // Un pont cassé couvre un pont de deux tuiles de large: il se pose sur un coin de tuiles.
            Self::Exit { kind, at } | Self::Barrier { kind, at } => *at = if *kind == ExitKind::Bridge { to.round() } else { to },
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
    /// Décor de chaque tuile, quand il change dans le niveau: une chaîne par ligne comme `rows`, où `.` garde la région du
    /// niveau et un chiffre de `0` à `8` emprunte le sol ou les murs de la région de ce rang dans `Region::ALL`. Vide: toute
    /// la carte garde la région du niveau. Permet à un niveau de changer d'aspect au fil de la progression.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style: Vec<String>,
    /// Décor posé sur le sol (herbes, cailloux, flaques...): voir `scatter_decor`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decor: Vec<crate::decor::Decor>,
    /// Salles, couloirs et portes (voir `Layout`).
    pub layout: Layout,
    /// Objectif du niveau: genre et nombre à atteindre.
    pub quest: Option<(QuestKind, u32)>,
    /// La sortie est la zone de sortie elle-même (invisible, par exemple au bout d'un chemin) plutôt qu'un escalier posé
    /// à son centre. Sans effet quand un élément `Exit` est posé à la main.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exit_area: bool,
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
            style: Vec::new(),
            decor: Vec::new(),
            layout: Layout {
                rooms: vec![zone(2, RoomKind::Start), zone(w - 8, RoomKind::Exit)],
                connections: Vec::<Connection>::new(),
                start: 0,
                exit: 1,
                treasure: None,
            },
            quest: None,
            exit_area: false,
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
            style: Vec::new(),
            decor: Vec::new(),
            layout: dungeon.layout.clone(),
            quest,
            exit_area: false,
            things,
        }
    }

    /// La région dont la tuile (`x`, `y` croissant vers le haut) emprunte ses briques: celle que le décor du niveau lui donne,
    /// sinon la région du niveau.
    pub fn region_at(&self, x: i32, y: i32) -> Region {
        let row = self.size.y as i32 - 1 - y;
        let style = usize::try_from(row).ok().and_then(|row| self.style.get(row));
        style
            .and_then(|line| line.chars().nth(usize::try_from(x).ok()?))
            .and_then(|c| c.to_digit(10))
            .and_then(|rank| Region::ALL.get(rank as usize).copied())
            .unwrap_or(self.region)
    }

    /// Donne à une tuile le décor de `region` (ou la région du niveau si elle est identique, pour garder le fichier léger).
    pub fn set_region_at(&mut self, x: i32, y: i32, region: Region) {
        if x < 0 || y < 0 || x >= self.size.x as i32 || y >= self.size.y as i32 {
            return;
        }
        let row = (self.size.y as i32 - 1 - y) as usize;
        if self.style.is_empty() {
            if region == self.region {
                return;
            }
            self.style = vec![".".repeat(self.size.x as usize); self.size.y as usize];
        }
        let rank = Region::ALL.iter().position(|r| *r == region).unwrap_or(0);
        let mark = if region == self.region { '.' } else { char::from_digit(rank as u32, 10).unwrap_or('.') };
        let mut chars: Vec<char> = self.style[row].chars().collect();
        chars[x as usize] = mark;
        self.style[row] = chars.into_iter().collect();
        // Plus aucun écart avec la région du niveau: on n'écrit rien dans le fichier.
        if self.style.iter().all(|line| line.chars().all(|c| c == '.')) {
            self.style.clear();
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
                tiles[y * self.size.x as usize + x] = match c {
                    '#' => Tile::Wall,
                    '~' => Tile::Water,
                    '=' => Tile::Bridge,
                    _ => Tile::Floor,
                };
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
            if let Some(c) = line.chars().find(|c| !matches!(c, '#' | '.' | '~' | '=')) {
                problems.push(format!("la ligne {} contient le caractère inconnu '{c}'", i + 1));
            }
        }
        problems.extend(self.decor_problems());
        if !self.style.is_empty() {
            if self.style.len() != self.size.y as usize {
                problems.push(format!("{} lignes de décor pour une hauteur de {}", self.style.len(), self.size.y));
            }
            for (i, line) in self.style.iter().enumerate() {
                if line.chars().count() != self.size.x as usize {
                    problems.push(format!("la ligne de décor {} fait {} tuiles pour une largeur de {}", i + 1, line.chars().count(), self.size.x));
                }
                if let Some(c) = line.chars().find(|c| *c != '.' && !c.to_digit(10).is_some_and(|d| (d as usize) < Region::ALL.len())) {
                    problems.push(format!("la ligne de décor {} contient le caractère inconnu '{c}'", i + 1));
                }
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
            // Un pont cassé couvre un bloc de deux tuiles sur deux de pont (ou de sol): c'est tout ce bloc qui doit être praticable.
            if let Thing::Exit { kind: ExitKind::Bridge, .. } | Thing::Barrier { kind: ExitKind::Bridge, .. } = thing {
                let corner = dungeon.room.bridge_block_corner(at);
                if !dungeon.room.bridge_block(corner).iter().all(|(x, y)| dungeon.room.tile(*x, *y).walkable()) {
                    problems.push(format!("{thing:?}: le pont cassé doit couvrir un pont de deux tuiles de large"));
                }
                continue;
            }
            if !dungeon.room.tile(at.x.floor() as i32, at.y.floor() as i32).walkable() {
                problems.push(format!("{thing:?} n'est pas sur le sol"));
            }
        }
        // Le départ et la sortie communiquent (les portes comptent comme ouvertes).
        let (start, exit) = (rooms[self.layout.start].center(), rooms[self.layout.exit].center());
        let mut seen = vec![false; (self.size.x * self.size.y) as usize];
        let mut stack = vec![start.as_ivec2()];
        while let Some(t) = stack.pop() {
            if !dungeon.room.tile(t.x, t.y).walkable() {
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
        problems.extend(self.check_links());
        if rooms[self.layout.start].kind != RoomKind::Start {
            problems.push("la salle de départ n'est pas de genre Start".to_string());
        }
        problems
    }

    /// Les liens entre les éléments, déduits des règles du jeu (il n'y en a pas d'autres, rien n'est écrit dans le fichier):
    /// ce qu'il faut trouver pour chaque panneau, ce que chaque panneau défait (un accès: herse, enchantement, porte royale), et
    /// l'objectif du niveau. La sortie, elle, n'est pas un déblocage: on la prend en atteignant sa zone.
    pub fn links(&self) -> Vec<Link> {
        use LinkEnd::{Door, Exit, Thing as At};
        let mut links = Vec::new();
        let royal = self.layout.connections.iter().position(|c| c.door == Some(crate::DoorKind::Royal));
        // Ce que l'objectif ouvre: l'obstacle posé sur la carte s'il y en a un, sinon la sortie.
        let opens = self.things.iter().position(|t| matches!(t, Thing::Barrier { .. })).map_or(Exit, At);
        for (i, thing) in self.things.iter().enumerate() {
            match thing {
                Thing::Pickup { kind, .. } => {
                    let (wanted, why) = match kind {
                        PickupKind::GearPart => (InteractKind::Mechanism, "pièce de rechange du mécanisme"),
                        PickupKind::Ingredient => (InteractKind::Rune, "ingrédient de la rune"),
                        PickupKind::TargetDisc => (InteractKind::Target, "disque de la cible"),
                        _ => continue,
                    };
                    for (j, other) in self.things.iter().enumerate() {
                        if matches!(other, Thing::Panel { kind, .. } if *kind == wanted) {
                            links.push(Link { from: At(i), to: At(j), why });
                        }
                    }
                }
                Thing::Panel { kind, .. } => {
                    for (j, other) in self.things.iter().enumerate() {
                        match (kind, other) {
                            (InteractKind::Rune, Thing::Ward { .. }) => links.push(Link { from: At(i), to: At(j), why: "dissipe l'enchantement" }),
                            (InteractKind::Target, Thing::Portcullis { .. }) => links.push(Link { from: At(i), to: At(j), why: "la flèche lève la herse" }),
                            _ => {}
                        }
                    }
                    if let Some(door) = royal {
                        links.push(Link { from: At(i), to: Door(door), why: "à terminer avant de pousser la porte royale" });
                    }
                    for (j, other) in self.things.iter().enumerate() {
                        if matches!(other, Thing::Barrier { .. }) {
                            links.push(Link { from: At(i), to: At(j), why: "à terminer pour débloquer le passage" });
                        }
                    }
                }
                Thing::Ward { .. } | Thing::Portcullis { .. } => {
                    if let Some(door) = royal {
                        links.push(Link { from: At(i), to: Door(door), why: "barre l'accès à la porte royale" });
                    }
                }
                Thing::Relic { .. } => links.push(Link { from: At(i), to: opens, why: "relique de l'objectif" }),
                Thing::Captive { .. } => links.push(Link { from: At(i), to: opens, why: "prisonnier de l'objectif" }),
                Thing::Monster { kind, .. } if kind.is_boss() => links.push(Link { from: At(i), to: opens, why: "le boss garde la sortie" }),
                _ => {}
            }
        }
        links
    }

    /// Les anomalies des liens: un panneau auquel il manque de quoi agir, une herse que rien ne lève, un enchantement que rien ne dissipe.
    fn check_links(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let count = |f: &dyn Fn(&Thing) -> bool| self.things.iter().filter(|t| f(t)).count();
        let panels = |wanted: InteractKind| count(&|t| matches!(t, Thing::Panel { kind, .. } if *kind == wanted));
        let pickups = |wanted: PickupKind| count(&|t| matches!(t, Thing::Pickup { kind, .. } if *kind == wanted));
        for (wanted, pickup, need, what) in [
            (InteractKind::Mechanism, PickupKind::GearPart, 1, "pièce de mécanisme (orange)"),
            (InteractKind::Rune, PickupKind::Ingredient, INGREDIENTS_NEEDED as usize, "ingrédient(s) (violet)"),
            (InteractKind::Target, PickupKind::TargetDisc, 1, "disque de cible (vert)"),
        ] {
            let (have, wanted_panels) = (pickups(pickup), panels(wanted));
            if wanted_panels > 0 && have < need {
                problems.push(format!("un panneau demande {need} {what}, le niveau n'en contient que {have}"));
            }
            if wanted_panels == 0 && have > 0 {
                problems.push(format!("{have} {what} posé(s), mais aucun panneau ne s'en sert"));
            }
        }
        if count(&|t| matches!(t, Thing::Portcullis { .. })) > 0 && panels(InteractKind::Target) == 0 {
            problems.push("la herse ne peut être levée que par un panneau de cible (Rôdeur)".to_string());
        }
        if count(&|t| matches!(t, Thing::Ward { .. })) > 0 && panels(InteractKind::Rune) == 0 {
            problems.push("l'enchantement ne peut être dissipé que par un panneau de rune (Ensorceleur)".to_string());
        }
        if count(&|t| matches!(t, Thing::Exit { .. })) > 1 {
            problems.push("plus d'une sortie posée: seule l'une d'elles sera gardée".to_string());
        }
        for thing in &self.things {
            if let Thing::Trigger { node, .. } | Thing::Panel { node, .. } = thing {
                if !node.is_empty() && !DIALOGUE_NODES.contains(&node.as_str()) {
                    problems.push(format!("scène inconnue: « {node} »"));
                }
            }
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
    fn tiles_can_borrow_the_look_of_another_region() {
        let mut def = LevelDef::blank("essai", Region::Forest, UVec2::new(20, 12));
        assert_eq!(def.region_at(3, 3), Region::Forest);
        // Repeindre dans la région du niveau ne crée rien.
        def.set_region_at(3, 3, Region::Forest);
        assert!(def.style.is_empty());
        def.set_region_at(3, 3, Region::Crypts);
        def.set_region_at(19, 11, Region::Royal);
        assert_eq!(def.region_at(3, 3), Region::Crypts);
        assert_eq!(def.region_at(19, 11), Region::Royal);
        assert_eq!(def.region_at(4, 3), Region::Forest);
        assert!(def.validate().iter().all(|p| !p.contains("décor")));
        // Le décor survit à l'écriture du fichier, et disparaît quand on le remet à la région du niveau.
        let back = LevelDef::parse(&def.to_ron()).unwrap();
        assert_eq!(back.region_at(3, 3), Region::Crypts);
        def.set_region_at(3, 3, Region::Forest);
        def.set_region_at(19, 11, Region::Forest);
        assert!(def.style.is_empty());
    }

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

    /// De l'eau sans pont coupe le niveau: la validation le signale; transformée en pont, la salle redevient praticable.
    #[test]
    fn water_cuts_a_level_unless_it_is_a_bridge() {
        let dungeon = Dungeon::generate(3);
        let mut def = LevelDef::from_dungeon("essai", Region::Crypts, &dungeon, None, Vec::new());
        assert!(def.validate().is_empty());
        let exit = dungeon.layout.rooms[dungeon.layout.exit].clone();
        let fill = |def: &mut LevelDef, c: char| {
            for y in exit.min.y..=exit.max.y {
                let row = (def.size.y - 1 - y) as usize;
                let mut line: Vec<char> = def.rows[row].chars().collect();
                for x in exit.min.x..=exit.max.x {
                    line[x as usize] = c;
                }
                def.rows[row] = line.into_iter().collect();
            }
        };
        fill(&mut def, '~');
        assert!(def.to_dungeon().is_ok(), "l'eau est un caractère connu");
        let center = exit.center();
        assert_eq!(def.to_dungeon().unwrap().room.tile(center.x as i32, center.y as i32), Tile::Water);
        assert!(def.validate().iter().any(|p| p.contains("atteignable")), "{:?}", def.validate());
        // Des ponts à la place de l'eau: la salle se rejoint de nouveau.
        fill(&mut def, '=');
        assert_eq!(def.to_dungeon().unwrap().room.tile(center.x as i32, center.y as i32), Tile::Bridge);
        assert!(def.validate().is_empty(), "{:?}", def.validate());
    }

    /// Un pont cassé visé à une tuile près n'est pas signalé s'il peut se caler sur un pont de deux tuiles de large; sur l'eau, il l'est.
    #[test]
    fn a_broken_bridge_is_valid_on_a_two_wide_bridge_only() {
        let dungeon = Dungeon::generate(3);
        let mut def = LevelDef::from_dungeon("essai", Region::Crypts, &dungeon, None, Vec::new());
        // Une rivière de trois tuiles de large dans la salle de départ, avec un pont de deux tuiles de large.
        let start = dungeon.layout.rooms[dungeon.layout.start].clone();
        let (cx, cy) = (start.center().x as i32, start.center().y as i32);
        let paint = |def: &mut LevelDef, x: i32, y: i32, c: char| {
            let row = (def.size.y as i32 - 1 - y) as usize;
            let mut chars: Vec<char> = def.rows[row].chars().collect();
            chars[x as usize] = c;
            def.rows[row] = chars.into_iter().collect();
        };
        for y in cy - 3..=cy + 3 {
            for x in cx - 1..=cx + 1 {
                paint(&mut def, x, y, '~');
            }
        }
        for y in cy - 1..=cy {
            for x in cx - 1..=cx + 1 {
                paint(&mut def, x, y, '=');
            }
        }
        // Le pont cassé est visé sur la rangée du dessus (pas pile sur le bon coin): il se cale quand même.
        def.things.push(Thing::Exit { kind: ExitKind::Bridge, at: Vec2::new(cx as f32 + 0.5, cy as f32 + 1.5) });
        assert!(def.validate().iter().all(|p| !p.contains("pont cassé")), "{:?}", def.validate());
        // Sur de l'eau, il est signalé.
        let last = def.things.len() - 1;
        def.things[last] = Thing::Exit { kind: ExitKind::Bridge, at: Vec2::new(cx as f32, cy as f32 + 3.0) };
        assert!(def.validate().iter().any(|p| p.contains("pont cassé")), "{:?}", def.validate());
    }

    #[test]
    fn world_and_tile_positions_are_inverse() {
        let size = UVec2::new(88, 48);
        let at = Vec2::new(12.5, 30.25);
        assert!((LevelDef::tile_in(size, LevelDef::world_in(size, at)) - at).length() < 1e-4);
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;

    #[test]
    fn every_campaign_level_has_consistent_links() {
        for level in 1..=crate::LEVEL_COUNT {
            let problems = crate::level_def(level).validate();
            assert!(problems.is_empty(), "niveau {level}: {problems:?}");
        }
    }

    #[test]
    fn panels_link_to_what_they_need_and_what_they_undo() {
        let mut def = LevelDef::blank("essai", Region::Forest, UVec2::new(32, 20));
        let at = Vec2::new(5.5, 5.5);
        def.things = vec![
            Thing::Panel { kind: InteractKind::Rune, at, node: String::new() },
            Thing::Ward { at },
            Thing::Pickup { kind: PickupKind::Ingredient, at },
        ];
        let links = def.links();
        assert!(links.iter().any(|l| l.from == LinkEnd::Thing(2) && l.to == LinkEnd::Thing(0)));
        assert!(links.iter().any(|l| l.from == LinkEnd::Thing(0) && l.to == LinkEnd::Thing(1)));
        assert!(!links.iter().any(|l| l.to == LinkEnd::Exit), "un panneau ne débloque pas la sortie");
        let problems = def.validate();
        assert!(problems.iter().any(|p| p.contains("ingrédient")), "{problems:?}");
    }
}
