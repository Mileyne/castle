//! Génération du château: une grille de salles reliées par des couloirs, déterministe à partir d'une graine.
//!
//! Le serveur et les clients appellent `Dungeon::generate(seed)` et obtiennent exactement la même carte;
//! seule la graine circule sur le réseau.

use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use crate::{DoorKind, Room, TILE_SIZE, Tile};

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
}

/// Rayon de l'enchantement qui interdit d'approcher la porte royale, en pixels.
pub const WARD_RADIUS: f32 = 72.0;

/// Emplacements de l'énigme de la porte royale, en coordonnées du monde.
#[derive(Clone, Debug)]
pub struct PuzzleSites {
    /// Centre de la porte royale (deux tuiles).
    pub door: Vec2,
    /// Centre de la herse, une tuile devant la porte, côté couloir.
    pub portcullis: Vec2,
    /// La porte et la herse s'étendent en hauteur (couloir horizontal) quand c'est vrai.
    pub vertical: bool,
    /// Trois tuiles de mur (faces tournées vers une salle) où poser les panneaux; le sol est libre juste devant.
    pub panels: [Vec2; 3],
}

impl Layout {
    /// Index de la salle qui contient ce point du monde (les couloirs n'en sont pas).
    pub fn room_at(&self, room: &Room, point: Vec2) -> Option<usize> {
        let tile = room.world_to_tile(point);
        self.rooms.iter().position(|r| {
            (r.min.x as i32..=r.max.x as i32).contains(&tile.x) && (r.min.y as i32..=r.max.y as i32).contains(&tile.y)
        })
    }

    /// Centre et taille d'une salle en coordonnées du monde, murs compris.
    pub fn room_frame(&self, room: &Room, index: usize) -> (Vec2, Vec2) {
        let info = &self.rooms[index];
        let low = room.tile_center(info.min.x, info.min.y) - Vec2::splat(TILE_SIZE);
        let high = room.tile_center(info.max.x, info.max.y) + Vec2::splat(TILE_SIZE);
        ((low + high) / 2.0, high - low)
    }

    /// Le couloir de la porte royale: la salle `a` est la salle de sortie, `b` l'antichambre.
    pub fn royal_connection(&self) -> &Connection {
        self.connections
            .iter()
            .find(|c| c.door == Some(DoorKind::Royal))
            .expect("le château a toujours une porte royale")
    }

    /// Où poser la herse, l'enchantement et les trois panneaux de la porte royale. Les panneaux sont des tuiles de mur
    /// tournées vers l'antichambre (ou, à défaut de place, vers ses voisines), hors de portée de l'enchantement.
    pub fn puzzle_sites(&self, room: &Room) -> PuzzleSites {
        let conn = self.royal_connection();
        let (a, b) = (conn.a, conn.b);
        let second = if conn.door_vertical { conn.door_min + UVec2::Y } else { conn.door_min + UVec2::X };
        let door = (room.tile_center(conn.door_min.x, conn.door_min.y) + room.tile_center(second.x, second.y)) / 2.0;
        // Le couloir part de la salle de sortie: « vers l'extérieur » s'éloigne de son centre.
        let exit_center = self.rooms[a].center();
        let outward = if conn.door_vertical {
            Vec2::new(if conn.door_min.x > exit_center.x { 1.0 } else { -1.0 }, 0.0)
        } else {
            Vec2::new(0.0, if conn.door_min.y > exit_center.y { 1.0 } else { -1.0 })
        };
        let portcullis = door + outward * TILE_SIZE;

        // Salles où chercher des murs: l'antichambre d'abord, puis ses voisines de proche en proche.
        let mut order = vec![b];
        let mut next = 0;
        while next < order.len() && order.len() < 5 {
            let current = order[next];
            next += 1;
            for c in &self.connections {
                let other = if c.a == current { c.b } else if c.b == current { c.a } else { continue };
                if other != a && !order.contains(&other) {
                    order.push(other);
                }
            }
        }
        let scramble = |x: u32, y: u32| (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ conn.door_min.x.wrapping_mul(83492791)) >> 3;
        let mut panels: Vec<(UVec2, Vec2)> = Vec::new();
        for min_gap in [4u32, 3, 2, 1] {
            panels.clear();
            for &index in &order {
                let info = &self.rooms[index];
                let y = info.max.y + 1;
                let mut candidates: Vec<UVec2> = (info.min.x..=info.max.x)
                    .map(|x| UVec2::new(x, y))
                    .filter(|t| {
                        let (x, y) = (t.x as i32, t.y as i32);
                        room.tile(x, y) == Tile::Wall
                            && [-1, 0, 1].iter().all(|dx| room.tile(x + dx, y - 1) == Tile::Floor)
                            && room.tile_center(t.x, t.y).distance(door) >= WARD_RADIUS + 32.0
                    })
                    .collect();
                candidates.sort_by_key(|t| scramble(t.x, t.y));
                for t in candidates {
                    if panels.iter().all(|(p, _)| p.y != t.y || p.x.abs_diff(t.x) >= min_gap) {
                        panels.push((t, room.tile_center(t.x, t.y)));
                        if panels.len() == 3 {
                            break;
                        }
                    }
                }
                if panels.len() == 3 {
                    break;
                }
            }
            if panels.len() == 3 {
                break;
            }
        }
        let positions: Vec<Vec2> = panels.iter().map(|(_, p)| *p).collect();
        let panels: [Vec2; 3] = positions.try_into().expect("trois panneaux trouvent toujours une place");
        PuzzleSites { door, portcullis, vertical: conn.door_vertical, panels }
    }
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

        Self {
            room,
            layout: Layout { rooms, connections, start, exit, treasure },
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

    /// Sur beaucoup de châteaux: trois panneaux distincts sur des faces de mur, hors de l'enchantement, et une herse qui
    /// tient dans le couloir devant la porte royale.
    #[test]
    fn puzzle_sites_fit_every_castle() {
        for seed in 0..500u64 {
            let dungeon = Dungeon::generate(seed);
            let (room, layout) = (&dungeon.room, &dungeon.layout);
            let sites = layout.puzzle_sites(room);
            let half = TILE_SIZE / 2.0;
            for (i, panel) in sites.panels.iter().enumerate() {
                let tile = |p: Vec2| (((p.x - room.tile_center(0, 0).x) / TILE_SIZE).round() as i32, ((p.y - room.tile_center(0, 0).y) / TILE_SIZE).round() as i32);
                let (x, y) = tile(*panel);
                assert_eq!(room.tile(x, y), Tile::Wall, "seed {seed}: le panneau {i} n'est pas dans un mur");
                assert_eq!(room.tile(x, y - 1), Tile::Floor, "seed {seed}: rien de libre devant le panneau {i}");
                assert!(panel.distance(sites.door) >= WARD_RADIUS + 16.0, "seed {seed}: panneau {i} sous l'enchantement");
                for other in &sites.panels[i + 1..] {
                    assert!(panel.distance(*other) >= TILE_SIZE, "seed {seed}: deux panneaux au même endroit");
                }
            }
            // La herse couvre deux tuiles de couloir libres, une tuile devant la porte.
            let along = if sites.vertical { Vec2::Y } else { Vec2::X };
            for side in [-1.0f32, 1.0] {
                let p = sites.portcullis + along * side * half;
                let (x, y) = (((p.x - room.tile_center(0, 0).x) / TILE_SIZE + 0.0).round() as i32, ((p.y - room.tile_center(0, 0).y) / TILE_SIZE).round() as i32);
                assert_eq!(room.tile(x, y), Tile::Floor, "seed {seed}: la herse sort du couloir en {x},{y}");
            }
            assert!(sites.portcullis.distance(sites.door) > 0.0);
        }
    }

    #[test]
    fn puzzle_panels_are_spread_out_in_the_antechamber() {
        // En général les trois panneaux sont à plusieurs tuiles les uns des autres (on relâche l'écart si la place manque).
        let spread = (0..200u64)
            .filter(|&seed| {
                let dungeon = Dungeon::generate(seed);
                let sites = dungeon.layout.puzzle_sites(&dungeon.room);
                sites.panels.iter().enumerate().all(|(i, a)| sites.panels[i + 1..].iter().all(|b| a.distance(*b) >= 3.0 * TILE_SIZE))
            })
            .count();
        assert!(spread >= 180, "seulement {spread} châteaux sur 200 ont des panneaux bien écartés");
    }

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
