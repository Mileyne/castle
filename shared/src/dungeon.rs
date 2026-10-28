//! Génération du château: une grille de salles reliées par des couloirs, déterministe à partir d'une graine.
//!
//! Le serveur et les clients appellent `Dungeon::generate(seed)` et obtiennent exactement la même carte;
//! seule la graine circule sur le réseau.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{DoorKind, Room, Tile};

/// Grille de cellules, une salle par cellule, et taille d'une cellule en tuiles.
pub const GRID: UVec2 = UVec2::new(4, 3);
pub const CELL: UVec2 = UVec2::new(22, 16);
/// Largeur des couloirs, en tuiles.
pub const CORRIDOR_WIDTH: u32 = 2;

/// Les six régions du château.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    Crypts,
    Gardens,
    Library,
    Forge,
    Tower,
    Royal,
}

impl Region {
    pub const ALL: [Region; 6] = [
        Region::Crypts,
        Region::Gardens,
        Region::Library,
        Region::Forge,
        Region::Tower,
        Region::Royal,
    ];

    /// Clé de localisation du nom de la région.
    pub fn name_key(self) -> &'static str {
        match self {
            Self::Crypts => "region-crypts",
            Self::Gardens => "region-gardens",
            Self::Library => "region-library",
            Self::Forge => "region-forge",
            Self::Tower => "region-tower",
            Self::Royal => "region-royal",
        }
    }

    /// Teinte RVB (sol, murs) appliquée aux tuiles, en attendant des tuiles propres à chaque région.
    pub fn tint(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::Crypts => ([0.85, 0.95, 1.1], [0.8, 0.9, 1.1]),
            Self::Gardens => ([0.75, 1.15, 0.8], [0.7, 1.05, 0.75]),
            Self::Library => ([1.15, 0.95, 0.75], [1.15, 0.9, 0.7]),
            Self::Forge => ([1.25, 0.8, 0.7], [1.2, 0.75, 0.65]),
            Self::Tower => ([0.9, 0.85, 1.25], [0.85, 0.8, 1.25]),
            Self::Royal => ([1.15, 1.0, 0.8], [1.2, 1.05, 0.7]),
        }
    }
}

/// Une expédition en cours. Entité répliquée: les clients en déduisent la carte.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Expedition {
    pub seed: u64,
    pub region: Region,
    /// Numéro du niveau: 1 au départ, +1 à chaque passage par le portail.
    pub level: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Start,
    Combat,
    /// Salle au trésor, fermée par une porte à clé.
    Treasure,
    /// Salle du portail de sortie, derrière la porte royale.
    Exit,
}

#[derive(Clone, Debug)]
pub struct RoomInfo {
    /// Tuiles de la salle, bornes incluses.
    pub min: UVec2,
    pub max: UVec2,
    pub kind: RoomKind,
}

impl RoomInfo {
    pub fn size(&self) -> UVec2 {
        self.max - self.min + UVec2::ONE
    }

    pub fn center(&self) -> UVec2 {
        self.min + self.size() / 2
    }
}

/// Couloir entre deux salles, avec l'emplacement d'une éventuelle porte.
#[derive(Clone, Debug)]
pub struct Connection {
    pub a: usize,
    pub b: usize,
    /// Coin minimal du bloc de la porte (deux tuiles de large, juste à la sortie de la salle `a`).
    pub door_min: UVec2,
    /// La porte s'étend en hauteur (couloir horizontal) ou en largeur (couloir vertical).
    pub door_vertical: bool,
    pub door: Option<DoorKind>,
}

#[derive(Resource, Clone, Debug)]
pub struct Layout {
    pub rooms: Vec<RoomInfo>,
    pub connections: Vec<Connection>,
    pub start: usize,
    pub exit: usize,
    pub treasure: Option<usize>,
    /// Salles où se trouvent le levier, la rune et le mécanisme de la porte royale.
    pub puzzle_rooms: [usize; 3],
}

pub struct Dungeon {
    pub room: Room,
    pub layout: Layout,
}

/// Générateur pseudo-aléatoire déterministe (splitmix64).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entier dans `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as u32
    }

    /// Flottant dans `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32 <= p
    }


    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range(0, i as u32) as usize;
            items.swap(i, j);
        }
    }
}

impl Dungeon {
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let cell_count = (GRID.x * GRID.y) as usize;
        let cell_of = |i: usize| UVec2::new(i as u32 % GRID.x, i as u32 / GRID.x);
        let index_of = |c: UVec2| (c.y * GRID.x + c.x) as usize;

        // Une salle de taille et de position variables dans chaque cellule.
        let mut rooms: Vec<RoomInfo> = (0..cell_count)
            .map(|i| {
                let (w, h) = (rng.range(10, 18), rng.range(8, 12));
                let ox = rng.range(2, CELL.x - w - 2);
                let oy = rng.range(2, CELL.y - h - 2);
                let min = cell_of(i) * CELL + UVec2::new(ox, oy);
                RoomInfo {
                    min,
                    max: min + UVec2::new(w - 1, h - 1),
                    kind: RoomKind::Combat,
                }
            })
            .collect();

        let neighbors = |i: usize| -> Vec<usize> {
            let c = cell_of(i);
            let mut n = Vec::new();
            if c.x > 0 {
                n.push(index_of(c - UVec2::X));
            }
            if c.x + 1 < GRID.x {
                n.push(index_of(c + UVec2::X));
            }
            if c.y > 0 {
                n.push(index_of(c - UVec2::Y));
            }
            if c.y + 1 < GRID.y {
                n.push(index_of(c + UVec2::Y));
            }
            n
        };

        // Arbre couvrant par parcours en profondeur aléatoire: toutes les salles sont reliées.
        let start = rng.range(0, cell_count as u32 - 1) as usize;
        let mut visited = vec![false; cell_count];
        let mut stack = vec![start];
        visited[start] = true;
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut depth = vec![0u32; cell_count];
        while let Some(&current) = stack.last() {
            let mut options: Vec<usize> =
                neighbors(current).into_iter().filter(|&n| !visited[n]).collect();
            if options.is_empty() {
                stack.pop();
                continue;
            }
            rng.shuffle(&mut options);
            let next = options[0];
            visited[next] = true;
            depth[next] = depth[current] + 1;
            edges.push((current, next));
            stack.push(next);
        }

        let degree = |edges: &[(usize, usize)], i: usize| {
            edges.iter().filter(|(a, b)| *a == i || *b == i).count()
        };
        // Sortie: l'impasse la plus éloignée du départ. Trésor: une autre impasse.
        let mut leaves: Vec<usize> = (0..cell_count)
            .filter(|&i| i != start && degree(&edges, i) == 1)
            .collect();
        rng.shuffle(&mut leaves);
        leaves.sort_by_key(|&i| std::cmp::Reverse(depth[i]));
        let exit = leaves[0];
        let treasure = leaves.get(1).copied();

        // Quelques boucles pour que le château ne soit pas un simple arbre, sans ouvrir la sortie ni le trésor.
        for a in 0..cell_count {
            for b in neighbors(a) {
                let protected = |i: usize| i == exit || Some(i) == treasure;
                let known = edges.iter().any(|&(x, y)| (x, y) == (a, b) || (x, y) == (b, a));
                if a < b && !known && !protected(a) && !protected(b) && rng.chance(0.3) {
                    edges.push((a, b));
                }
            }
        }

        rooms[start].kind = RoomKind::Start;
        rooms[exit].kind = RoomKind::Exit;
        if let Some(t) = treasure {
            rooms[t].kind = RoomKind::Treasure;
        }

        // Carte: tout est mur, puis on creuse les salles et les couloirs.
        let size = GRID * CELL;
        let mut room = Room {
            size,
            tiles: vec![Tile::Wall; (size.x * size.y) as usize],
        };
        for info in &rooms {
            carve(&mut room, info.min, info.max);
        }

        let mut connections = Vec::new();
        for &(x, y) in &edges {
            // La porte se place toujours à la sortie de la salle d'impasse.
            let (a, b) = if y == exit || Some(y) == treasure { (y, x) } else { (x, y) };
            let door = if a == exit {
                Some(DoorKind::Royal)
            } else if Some(a) == treasure {
                Some(DoorKind::Locked)
            } else {
                None
            };
            connections.push(carve_corridor(&mut room, &rooms, a, b, door));
        }

        // Piliers dans les salles de combat, à l'écart des axes où débouchent les couloirs.
        for info in rooms.iter().filter(|r| r.kind == RoomKind::Combat) {
            let (min, max, c) = (info.min, info.max, info.center());
            for _ in 0..rng.range(0, 3) {
                let (px, py) = (rng.range(min.x + 2, max.x - 3), rng.range(min.y + 2, max.y - 3));
                let near_axis = px.abs_diff(c.x) <= 3 || py.abs_diff(c.y) <= 3;
                if !near_axis {
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        room.set(px + dx, py + dy, Tile::Wall);
                    }
                }
            }
        }

        // Levier, rune et mécanisme dans trois salles de combat différentes.
        let mut combat: Vec<usize> = (0..cell_count)
            .filter(|&i| rooms[i].kind == RoomKind::Combat)
            .collect();
        rng.shuffle(&mut combat);
        let pick = |n: usize| combat[n % combat.len()];
        let puzzle_rooms = [pick(0), pick(1), pick(2)];

        Self {
            room,
            layout: Layout { rooms, connections, start, exit, treasure, puzzle_rooms },
        }
    }
}

fn carve(room: &mut Room, min: UVec2, max: UVec2) {
    for y in min.y..=max.y {
        for x in min.x..=max.x {
            room.set(x, y, Tile::Floor);
        }
    }
}

/// Creuse un couloir en L de la salle `a` vers la salle `b` et repère l'emplacement de la porte.
fn carve_corridor(
    room: &mut Room,
    rooms: &[RoomInfo],
    a: usize,
    b: usize,
    door: Option<DoorKind>,
) -> Connection {
    let (ra, rb) = (&rooms[a], &rooms[b]);
    let (ca, cb) = (ra.center(), rb.center());
    // Deux cellules voisines sur la même ligne: on part à l'horizontale; sinon à la verticale.
    let horizontal_first = ra.min.y / CELL.y == rb.min.y / CELL.y;
    let (door_min, door_vertical);
    if horizontal_first {
        carve(room, UVec2::new(ca.x.min(cb.x), ca.y - 1), UVec2::new(ca.x.max(cb.x), ca.y));
        carve(
            room,
            UVec2::new(cb.x - 1, ca.y.min(cb.y) - 1),
            UVec2::new(cb.x, ca.y.max(cb.y)),
        );
        let x = if cb.x > ca.x { ra.max.x + 1 } else { ra.min.x - 1 };
        door_min = UVec2::new(x, ca.y - 1);
        door_vertical = true;
    } else {
        carve(room, UVec2::new(ca.x - 1, ca.y.min(cb.y)), UVec2::new(ca.x, ca.y.max(cb.y)));
        carve(
            room,
            UVec2::new(ca.x.min(cb.x) - 1, cb.y - 1),
            UVec2::new(ca.x.max(cb.x), cb.y),
        );
        let y = if cb.y > ca.y { ra.max.y + 1 } else { ra.min.y - 1 };
        door_min = UVec2::new(ca.x - 1, y);
        door_vertical = false;
    }
    Connection { a, b, door_min, door_vertical, door }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reachable(dungeon: &Dungeon) -> Vec<bool> {
        let room = &dungeon.room;
        let size = room.size();
        let start = dungeon.layout.rooms[dungeon.layout.start].center();
        let mut seen = vec![false; (size.x * size.y) as usize];
        let mut queue = vec![start];
        seen[(start.y * size.x + start.x) as usize] = true;
        while let Some(p) = queue.pop() {
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (x, y) = (p.x as i32 + dx, p.y as i32 + dy);
                if room.tile(x, y) == Tile::Floor && !seen[(y as u32 * size.x + x as u32) as usize] {
                    seen[(y as u32 * size.x + x as u32) as usize] = true;
                    queue.push(UVec2::new(x as u32, y as u32));
                }
            }
        }
        seen
    }

    #[test]
    fn same_seed_same_castle() {
        let (a, b) = (Dungeon::generate(42), Dungeon::generate(42));
        assert_eq!(a.room.tiles, b.room.tiles);
        assert_eq!(a.layout.exit, b.layout.exit);
        assert_ne!(a.room.tiles, Dungeon::generate(43).room.tiles);
    }

    #[test]
    fn every_room_is_reachable_and_doors_sit_in_corridors() {
        for seed in 0..200 {
            let d = Dungeon::generate(seed);
            let seen = reachable(&d);
            for info in &d.layout.rooms {
                let c = info.center();
                assert!(seen[(c.y * d.room.size().x + c.x) as usize], "seed {seed}: salle isolée");
            }
            for conn in &d.layout.connections {
                for k in 0..CORRIDOR_WIDTH {
                    let t = if conn.door_vertical {
                        conn.door_min + UVec2::new(0, k)
                    } else {
                        conn.door_min + UVec2::new(k, 0)
                    };
                    assert_eq!(d.room.tile(t.x as i32, t.y as i32), Tile::Floor, "seed {seed}: porte dans un mur");
                }
            }
        }
    }

    #[test]
    fn paths_cross_the_castle() {
        for seed in 0..40 {
            let d = Dungeon::generate(seed);
            let tile_center = |i: usize| {
                let c = d.layout.rooms[i].center();
                d.room.tile_center(c.x, c.y)
            };
            let from = tile_center(d.layout.start);
            for i in 0..d.layout.rooms.len() {
                let path = d.room.find_path(from, tile_center(i), &[]);
                assert!(path.is_some(), "seed {seed}: pas de chemin vers la salle {i}");
            }
        }
    }

    #[test]
    fn exit_is_a_dead_end_behind_the_royal_door() {
        for seed in 0..200 {
            let d = Dungeon::generate(seed);
            let into_exit: Vec<_> = d
                .layout
                .connections
                .iter()
                .filter(|c| c.a == d.layout.exit || c.b == d.layout.exit)
                .collect();
            assert_eq!(into_exit.len(), 1, "seed {seed}");
            assert_eq!(into_exit[0].door, Some(DoorKind::Royal));
            assert_ne!(d.layout.exit, d.layout.start);
        }
    }
}
