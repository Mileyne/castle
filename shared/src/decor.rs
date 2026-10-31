//! Décor posé sur le sol d'un niveau: herbes, cailloux, flaques, ossements, tapis... Il ne gêne pas les déplacements (c'est de la
//! peinture au sol); le jeu l'assemble avec les tuiles. Les niveaux le listent (`LevelDef::decor`), et `scatter` en sème selon la
//! région pour les remplir d'un coup.

use bevy::{
    ecs::component::Component,
    math::{UVec2, Vec2},
};
use serde::{Deserialize, Serialize};

use crate::{LevelDef, Region, RoomKind, Thing};

/// Les sortes de décor, dans l'ordre des colonnes de la planche `decor.png` de `tools/sprite_forge`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorKind {
    Grass,
    Flowers,
    Fern,
    Pebbles,
    Rock,
    Mushrooms,
    Roots,
    Leaves,
    Puddle,
    Moss,
    Branch,
    Crack,
    Rubble,
    Bones,
    Rug,
    Ash,
}

/// Variantes dessinées de chaque sorte (lignes de la planche).
pub const DECOR_VARIANTS: u8 = 4;

impl DecorKind {
    pub const ALL: [DecorKind; 16] = [
        Self::Grass,
        Self::Flowers,
        Self::Fern,
        Self::Pebbles,
        Self::Rock,
        Self::Mushrooms,
        Self::Roots,
        Self::Leaves,
        Self::Puddle,
        Self::Moss,
        Self::Branch,
        Self::Crack,
        Self::Rubble,
        Self::Bones,
        Self::Rug,
        Self::Ash,
    ];

    /// Numéro de la colonne de la planche.
    pub fn column(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Nom pour l'éditeur.
    pub fn label(self) -> &'static str {
        match self {
            Self::Grass => "Herbe",
            Self::Flowers => "Fleurs",
            Self::Fern => "Fougère",
            Self::Pebbles => "Cailloux",
            Self::Rock => "Rocher",
            Self::Mushrooms => "Champignons",
            Self::Roots => "Racines",
            Self::Leaves => "Feuilles mortes",
            Self::Puddle => "Flaque",
            Self::Moss => "Mousse",
            Self::Branch => "Branche",
            Self::Crack => "Lézarde",
            Self::Rubble => "Gravats",
            Self::Bones => "Ossements",
            Self::Rug => "Tapis",
            Self::Ash => "Cendres",
        }
    }

    /// Ce qu'on sème dans une région, avec le poids relatif de chaque sorte.
    pub fn for_region(region: Region) -> &'static [(DecorKind, f32)] {
        use DecorKind::*;
        match region {
            Region::Forest => &[(Grass, 4.0), (Fern, 2.0), (Leaves, 2.0), (Flowers, 1.5), (Mushrooms, 1.0), (Roots, 1.0), (Branch, 1.0), (Rock, 0.8), (Pebbles, 0.8), (Puddle, 0.8)],
            Region::Marsh => &[(Puddle, 4.0), (Moss, 2.0), (Grass, 2.0), (Mushrooms, 1.5), (Roots, 1.5), (Branch, 1.0), (Pebbles, 1.0), (Fern, 1.0)],
            Region::Approach => &[(Pebbles, 3.0), (Grass, 2.0), (Rock, 1.5), (Crack, 1.0), (Leaves, 1.0), (Puddle, 0.8), (Rubble, 0.8), (Bones, 0.3)],
            Region::Crypts => &[(Crack, 3.0), (Bones, 2.0), (Rubble, 2.0), (Moss, 1.0), (Pebbles, 1.0), (Puddle, 0.5)],
            Region::Gardens => &[(Flowers, 3.0), (Grass, 3.0), (Fern, 1.5), (Leaves, 1.5), (Pebbles, 1.0), (Moss, 1.0), (Mushrooms, 0.5)],
            Region::Library => &[(Crack, 1.5), (Rubble, 1.0), (Rug, 0.6), (Pebbles, 0.3)],
            Region::Forge => &[(Ash, 3.0), (Rubble, 2.0), (Crack, 2.0), (Pebbles, 1.0), (Bones, 0.3)],
            Region::Tower => &[(Crack, 2.0), (Rubble, 1.5), (Ash, 1.0), (Pebbles, 1.0)],
            Region::Royal => &[(Rug, 1.5), (Crack, 1.0), (Pebbles, 0.3)],
        }
    }
}

/// Grands motifs posés sur le sol, dessinés en grand et qui bloquent le passage (au pied seulement): arbres, rochers, colonnes...
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandmarkKind {
    Oak,
    Pine,
    DeadTree,
    Boulder,
    Pillar,
    Statue,
    Fountain,
}

impl LandmarkKind {
    pub const ALL: [LandmarkKind; 7] = [Self::Oak, Self::Pine, Self::DeadTree, Self::Boulder, Self::Pillar, Self::Statue, Self::Fountain];

    /// Nom pour l'éditeur.
    pub fn label(self) -> &'static str {
        match self {
            Self::Oak => "Chêne",
            Self::Pine => "Sapin",
            Self::DeadTree => "Arbre mort",
            Self::Boulder => "Gros rocher",
            Self::Pillar => "Colonne",
            Self::Statue => "Statue",
            Self::Fountain => "Fontaine",
        }
    }

    /// Nom du fichier de sa planche (voir `tools/sprite_forge`).
    pub fn file(self) -> &'static str {
        match self {
            Self::Oak => "oak",
            Self::Pine => "pine",
            Self::DeadTree => "deadtree",
            Self::Boulder => "boulder",
            Self::Pillar => "pillar",
            Self::Statue => "statue",
            Self::Fountain => "fountain",
        }
    }

    /// Facteur d'affichage par rapport à la case de 64 unités des grands motifs (la fontaine est dessinée plus petite dans sa case).
    pub fn display_scale(self) -> f32 {
        match self {
            Self::Oak => 1.15,
            Self::Fountain => 1.5,
            _ => 1.0,
        }
    }

    /// Demi-dimensions (en unités du monde) de ce qui bloque le passage, autour du pied du motif: le tronc d'un arbre, pas sa
    /// couronne.
    pub fn half_size(self) -> Vec2 {
        match self {
            Self::Oak => Vec2::new(5.0, 4.0),
            Self::Pine => Vec2::new(4.0, 4.0),
            Self::DeadTree => Vec2::new(3.5, 3.5),
            Self::Boulder => Vec2::new(12.0, 8.0),
            Self::Pillar => Vec2::new(6.0, 6.0),
            Self::Statue => Vec2::new(7.0, 7.0),
            Self::Fountain => Vec2::new(14.0, 10.0),
        }
    }

    /// Ce qu'on sème dans une région, avec le poids relatif de chaque sorte (rien pour les régions sans grand motif).
    pub fn for_region(region: Region) -> &'static [(LandmarkKind, f32)] {
        use LandmarkKind::*;
        match region {
            Region::Forest => &[(Oak, 3.0), (Pine, 3.0), (Boulder, 0.6), (DeadTree, 0.4)],
            Region::Marsh => &[(DeadTree, 3.0), (Oak, 1.5), (Boulder, 1.0)],
            Region::Approach => &[(Boulder, 2.0), (DeadTree, 1.5), (Pillar, 0.8), (Pine, 1.0)],
            Region::Crypts => &[(Pillar, 3.0), (Statue, 1.5)],
            Region::Gardens => &[(Statue, 1.5), (Oak, 1.5), (Fountain, 0.5)],
            Region::Library => &[(Pillar, 3.0), (Statue, 1.0)],
            Region::Forge => &[(Pillar, 2.0), (Boulder, 1.0)],
            Region::Tower => &[(Pillar, 3.0), (Statue, 1.0)],
            Region::Royal => &[(Pillar, 3.0), (Statue, 2.0), (Fountain, 0.4)],
        }
    }
}

/// Un grand motif posé: replique pour que les clients le dessinent (le serveur lui donne en plus sa zone de blocage).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landmark(pub LandmarkKind);

/// Un élément de décor: sa sorte, sa variante, et la position de son centre en tuiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decor {
    pub kind: DecorKind,
    pub variant: u8,
    pub at: Vec2,
}

/// Part des tuiles de sol qui reçoivent un élément de décor.
pub const SCATTER_DENSITY: f32 = 0.11;

impl LevelDef {
    /// Sème du décor sur le sol du niveau selon le décor de chaque tuile (voir `region_at`). Le résultat ne dépend que de `seed`.
    pub fn scatter_decor(&self, seed: u64) -> Vec<Decor> {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32
        };
        let (w, h) = (self.size.x as i32, self.size.y as i32);
        let floor = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && self.rows[(h - 1 - y) as usize].as_bytes()[x as usize] == b'.';
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if !floor(x, y) || next() > SCATTER_DENSITY {
                    continue;
                }
                let table = DecorKind::for_region(self.region_at(x, y));
                let total: f32 = table.iter().map(|(_, weight)| weight).sum();
                let mut roll = next() * total;
                let mut kind = table[table.len() - 1].0;
                for (candidate, weight) in table {
                    if roll < *weight {
                        kind = *candidate;
                        break;
                    }
                    roll -= weight;
                }
                // Un tapis tient sur deux tuiles de large et une de haut: il lui faut la place.
                if kind == DecorKind::Rug && !(floor(x - 1, y) && floor(x + 1, y) && floor(x, y - 1) && floor(x, y + 1)) {
                    continue;
                }
                let at = Vec2::new(x as f32 + 0.5 + (next() - 0.5) * 0.7, y as f32 + 0.5 + (next() - 0.5) * 0.7);
                out.push(Decor { kind, variant: (next() * DECOR_VARIANTS as f32) as u8 % DECOR_VARIANTS, at });
            }
        }
        out
    }

    /// Sème des grands motifs (arbres, colonnes, rochers...) dans les endroits dégagés du niveau: jamais près d'un élément déjà posé,
    /// dans les zones de départ et de sortie, ni dans un passage étroit (il faut trois tuiles de sol de côté autour d'un arbre ou d'une colonne, cinq pour un gros motif).
    /// Le résultat ne dépend que de `seed`.
    pub fn scatter_landmarks(&self, seed: u64) -> Vec<Thing> {
        let mut state = seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32
        };
        let (w, h) = (self.size.x as i32, self.size.y as i32);
        let floor = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && self.rows[(h - 1 - y) as usize].as_bytes()[x as usize] == b'.';
        let in_zone = |x: i32, y: i32| {
            self.layout.rooms.iter().any(|r| {
                matches!(r.kind, RoomKind::Start | RoomKind::Exit)
                    && x >= r.min.x as i32 - 2
                    && x <= r.max.x as i32 + 2
                    && y >= r.min.y as i32 - 2
                    && y <= r.max.y as i32 + 2
            })
        };
        let mut candidates: Vec<(i32, i32)> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| !in_zone(x, y) && (-1..=1).all(|dy| (-1..=1).all(|dx| floor(x + dx, y + dy))))
            .collect();
        // Mélange (Fisher-Yates), pour que le choix ne suive pas l'ordre de la carte.
        for i in (1..candidates.len()).rev() {
            candidates.swap(i, (next() * (i + 1) as f32) as usize % (i + 1));
        }
        let floor_tiles = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| floor(x, y)).count();
        // Les extérieurs sont plus boisés que l'intérieur du château.
        let target = floor_tiles / if self.region.is_outdoor() { 55 } else { 110 };
        let mut placed: Vec<(Vec2, LandmarkKind)> = Vec::new();
        for (x, y) in candidates {
            if placed.len() >= target {
                break;
            }
            let at = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if self.things.iter().any(|t| t.at().distance(at) < 3.0) || placed.iter().any(|(p, _)| p.distance(at) < 3.0) {
                continue;
            }
            let table = LandmarkKind::for_region(self.region_at(x, y));
            let total: f32 = table.iter().map(|(_, weight)| weight).sum();
            let mut roll = next() * total;
            let mut kind = table[table.len() - 1].0;
            for (candidate, weight) in table {
                if roll < *weight {
                    kind = *candidate;
                    break;
                }
                roll -= weight;
            }
            // Les gros motifs (rochers, statues, fontaines) ont besoin de cinq tuiles de sol de côté.
            let wide = matches!(kind, LandmarkKind::Boulder | LandmarkKind::Statue | LandmarkKind::Fountain);
            if wide && !(-2..=2).all(|dy| (-2..=2).all(|dx| floor(x + dx, y + dy))) {
                continue;
            }
            // Une seule fontaine par niveau, et pas plus de trois statues.
            let count = |k: LandmarkKind| placed.iter().filter(|(_, p)| *p == k).count();
            if (kind == LandmarkKind::Fountain && count(kind) >= 1) || (kind == LandmarkKind::Statue && count(kind) >= 3) {
                continue;
            }
            placed.push((at, kind));
        }
        placed.into_iter().map(|(at, kind)| Thing::Landmark { kind, at }).collect()
    }

    /// Les problèmes du décor: éléments hors de la carte ou variantes inconnues.
    pub fn decor_problems(&self) -> Vec<String> {
        let size = UVec2::new(self.size.x, self.size.y).as_vec2();
        self.decor
            .iter()
            .filter(|d| d.at.x < 0.0 || d.at.y < 0.0 || d.at.x >= size.x || d.at.y >= size.y || d.variant >= DECOR_VARIANTS)
            .map(|d| format!("{:?} est hors de la carte ou de variante inconnue", d.kind))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landmarks_keep_every_part_of_the_level_reachable() {
        for region in [Region::Forest, Region::Crypts] {
            let mut def = LevelDef::blank("essai", region, UVec2::new(64, 40));
            let landmarks = def.scatter_landmarks(3);
            assert!(landmarks.len() >= 5, "{region:?}: {} grands motifs", landmarks.len());
            def.things.extend(landmarks);
            assert!(def.validate().is_empty(), "{:?}", def.validate());
            // Les pieds des motifs bloquent des tuiles: le départ doit toujours rejoindre la sortie.
            let (w, h) = (def.size.x as i32, def.size.y as i32);
            let blocked = |x: i32, y: i32| {
                def.things.iter().any(|t| match t {
                    Thing::Landmark { kind, at } => {
                        let half = kind.half_size() / crate::TILE_SIZE;
                        (x as f32 + 0.5 - at.x).abs() < half.x + 0.5 && (y as f32 + 0.5 - at.y).abs() < half.y + 0.5
                    }
                    _ => false,
                })
            };
            let start = def.layout.rooms[def.layout.start].center().as_ivec2();
            let exit = def.layout.rooms[def.layout.exit].center().as_ivec2();
            let mut seen = std::collections::HashSet::from([(start.x, start.y)]);
            let mut stack = vec![(start.x, start.y)];
            while let Some((x, y)) = stack.pop() {
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    let open = nx >= 0 && ny >= 0 && nx < w && ny < h && def.rows[(h - 1 - ny) as usize].as_bytes()[nx as usize] == b'.' && !blocked(nx, ny);
                    if open && seen.insert((nx, ny)) {
                        stack.push((nx, ny));
                    }
                }
            }
            assert!(seen.contains(&(exit.x, exit.y)), "{region:?}: la sortie n'est plus atteignable");
        }
    }

    #[test]
    fn scattered_decor_stays_on_the_floor_and_follows_the_region() {
        let def = LevelDef::blank("essai", Region::Forest, UVec2::new(40, 24));
        let decor = def.scatter_decor(7);
        assert!(decor.len() > 20, "du décor est semé ({})", decor.len());
        assert_eq!(decor, def.scatter_decor(7), "même graine, même décor");
        let forest: Vec<DecorKind> = DecorKind::for_region(Region::Forest).iter().map(|(k, _)| *k).collect();
        for d in &decor {
            assert!(forest.contains(&d.kind), "{:?} n'a rien à faire en forêt", d.kind);
            let (x, y) = (d.at.x.floor() as i32, d.at.y.floor() as i32);
            assert_eq!(def.rows[(def.size.y as i32 - 1 - y) as usize].as_bytes()[x as usize], b'.', "{:?} sur un mur", d.kind);
        }
        assert!(def.decor_problems().is_empty());
    }
}
