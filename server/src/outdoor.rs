//! Génération des niveaux extérieurs (forêt, marais, abords du château): des clairières irrégulières reliées par des
//! sentiers sinueux, entourées d'arbres (ou d'eau dans le marais). Elle ne sert qu'à produire les fichiers de départ de la
//! campagne (`castle_server --export-levels`): les niveaux sont ensuite retouchés à la main dans l'éditeur.

use bevy::prelude::*;
use castle_shared::*;

use crate::Rng;

/// Le mécanisme propre à une classe qui ouvre la sortie: son panneau (dans un mur, près de la sortie), ce qu'il faut
/// trouver pour l'actionner, et les scènes qui l'expliquent (à l'arrivée, puis devant le panneau).
pub struct Mechanism {
    pub kind: InteractKind,
    pub intro: &'static str,
    pub panel: &'static str,
}

/// Ce qu'un niveau extérieur doit contenir.
pub struct OutdoorSpec {
    pub name: &'static str,
    pub region: Region,
    pub level: u32,
    /// Clairières de combat entre le départ et la sortie.
    pub clearings: usize,
    /// Monstres par clairière, et générateurs (à partir de la deuxième clairière).
    pub pack: u32,
    /// Loups en plus de la meute ordinaire, répartis entre les clairières de combat.
    pub extra_wolves: u32,
    pub generators: bool,
    pub quest: Option<(QuestKind, u32)>,
    pub exit: ExitKind,
    /// Une seule grande arène au lieu d'un parcours (niveau de boss).
    pub arena: bool,
    pub mechanism: Option<Mechanism>,
    /// Prisonniers en cage à délivrer (ils laissent du butin).
    pub captives: u32,
}

struct Grid {
    w: i32,
    h: i32,
    floor: Vec<bool>,
}

impl Grid {
    fn new(w: i32, h: i32) -> Self {
        Self { w, h, floor: vec![false; (w * h) as usize] }
    }

    fn at(&self, x: i32, y: i32) -> bool {
        x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1 && self.floor[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, floor: bool) {
        if x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1 {
            self.floor[(y * self.w + x) as usize] = floor;
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: i32, floor: bool) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    self.set(cx + dx, cy + dy, floor);
                }
            }
        }
    }

    /// Une clairière aux bords irréguliers; renvoie son rectangle englobant.
    fn clearing(&mut self, cx: i32, cy: i32, rx: f32, ry: f32, rng: &mut Rng) -> (UVec2, UVec2) {
        let (p1, p2) = (rng.next_f32() * 6.28, rng.next_f32() * 6.28);
        let (mut min, mut max) = (IVec2::new(i32::MAX, i32::MAX), IVec2::new(i32::MIN, i32::MIN));
        for dy in -(ry as i32 + 3)..=(ry as i32 + 3) {
            for dx in -(rx as i32 + 3)..=(rx as i32 + 3) {
                let angle = (dy as f32).atan2(dx as f32);
                let edge = 1.0 + 0.16 * (3.0 * angle + p1).sin() + 0.09 * (5.0 * angle + p2).sin();
                let d = ((dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2)).sqrt();
                if d <= edge * 0.92 {
                    self.set(cx + dx, cy + dy, true);
                    min = min.min(IVec2::new(cx + dx, cy + dy));
                    max = max.max(IVec2::new(cx + dx, cy + dy));
                }
            }
        }
        (UVec2::new(min.x.max(1) as u32, min.y.max(1) as u32), UVec2::new(max.x as u32, max.y as u32))
    }

    /// Un sentier sinueux d'un point à un autre (horizontal, puis vertical, puis horizontal), avec son tracé.
    fn path(&mut self, a: IVec2, b: IVec2, radius: i32, rng: &mut Rng) -> Vec<IVec2> {
        let mid = (a.x + b.x) / 2 + (rng.next_f32() * 6.0 - 3.0) as i32;
        let phase = rng.next_f32() * 6.28;
        let mut points = Vec::new();
        let walk = |from: IVec2, to: IVec2, points: &mut Vec<IVec2>| {
            let steps = (to - from).abs().max_element().max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let wobble = if (to - from).x.abs() > (to - from).y.abs() { IVec2::new(0, ((i as f32 * 0.35 + phase).sin() * 1.6) as i32) } else { IVec2::ZERO };
                let p = IVec2::new(from.x + ((to.x - from.x) as f32 * t) as i32, from.y + ((to.y - from.y) as f32 * t) as i32) + wobble;
                points.push(p);
            }
        };
        walk(a, IVec2::new(mid, a.y), &mut points);
        walk(IVec2::new(mid, a.y), IVec2::new(mid, b.y), &mut points);
        walk(IVec2::new(mid, b.y), b, &mut points);
        for p in &points {
            self.disc(p.x, p.y, radius, true);
        }
        points
    }
}

/// Construit un niveau extérieur.
pub fn outdoor_level(spec: &OutdoorSpec, seed: u64) -> LevelDef {
    let mut rng = Rng(seed | 1);
    let (w, h) = if spec.arena { (80, 52) } else { (132, 60) };
    let mut grid = Grid::new(w, h);
    let mut rooms: Vec<RoomInfo> = Vec::new();
    let mut centers: Vec<IVec2> = Vec::new();
    let mut path_points: Vec<IVec2> = Vec::new();

    // Les clairières, de gauche à droite: départ, combats, sortie.
    let count = if spec.arena { 3 } else { spec.clearings + 2 };
    let mut y = h / 2;
    for i in 0..count {
        let last = i == count - 1;
        let first = i == 0;
        let x = if spec.arena {
            [11, 40, 70][i]
        } else {
            12 + (i as i32) * (w - 26) / (count as i32 - 1)
        };
        // Les clairières alternent haut et bas pour que le sentier serpente.
        let (rx, ry) = if spec.arena && i == 1 {
            (24.0, 17.0)
        } else if first {
            (7.0, 6.0)
        } else if last {
            (5.5, 4.5)
        } else {
            (8.5 + rng.next_f32() * 2.5, 6.5 + rng.next_f32() * 2.5)
        };
        y = if spec.arena { h / 2 } else if i % 2 == 0 { h / 2 - 8 + (rng.next_f32() * 6.0) as i32 } else { h / 2 + 4 + (rng.next_f32() * 8.0) as i32 };
        let (min, max) = grid.clearing(x, y, rx, ry, &mut rng);
        let kind = if first { RoomKind::Start } else if last { RoomKind::Exit } else { RoomKind::Combat };
        rooms.push(RoomInfo { min, max, kind });
        centers.push(IVec2::new(x, y));
    }
    let _ = y;
    // Sentiers entre clairières voisines (plus étroit devant la sortie).
    let mut connections = Vec::new();
    for i in 0..count - 1 {
        let radius = if i + 2 == count { 1 } else { 1 + (rng.next_f32() * 1.5) as i32 };
        path_points.extend(grid.path(centers[i], centers[i + 1], radius, &mut rng));
        connections.push(Connection { a: i, b: i + 1, door_min: UVec2::ZERO, door_vertical: false, door: None });
    }

    // Obstacles dans les clairières de combat: arbres isolés, rochers, mares (marais).
    let near_path = |x: i32, y: i32, points: &[IVec2]| points.iter().any(|p| (p.x - x).abs() <= 2 && (p.y - y).abs() <= 2);
    for (i, room) in rooms.iter().enumerate() {
        if room.kind != RoomKind::Combat {
            continue;
        }
        let center = centers[i];
        let area = room.size().x * room.size().y;
        for _ in 0..(area as f32 * 0.04) as usize {
            let x = room.min.x as i32 + (rng.next_f32() * room.size().x as f32) as i32;
            let y = room.min.y as i32 + (rng.next_f32() * room.size().y as f32) as i32;
            if !grid.at(x, y) || near_path(x, y, &path_points) || (x - center.x).abs() + (y - center.y).abs() < 4 {
                continue;
            }
            match spec.region {
                Region::Marsh => grid.disc(x, y, 1 + (rng.next_f32() * 1.6) as i32, false),
                // Arbres et rochers occupent 2x2 tuiles: la canopée (ou le sommet du rocher) au-dessus, les troncs en dessous.
                _ => {
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        grid.set(x + dx, y + dy, false);
                    }
                }
            }
        }
    }

    let size = UVec2::new(w as u32, h as u32);
    let rows: Vec<String> = (0..h).rev().map(|y| (0..w).map(|x| if grid.at(x, y) { '.' } else { '#' }).collect()).collect();
    let mut things = Vec::new();
    let tile_at = |x: i32, y: i32| Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
    // Un point libre au hasard dans une zone (hors des obstacles).
    let free_point = |room: &RoomInfo, grid: &Grid, rng: &mut Rng| -> Vec2 {
        for _ in 0..60 {
            let x = room.min.x as i32 + (rng.next_f32() * room.size().x as f32) as i32;
            let y = room.min.y as i32 + (rng.next_f32() * room.size().y as f32) as i32;
            if grid.at(x, y) && grid.at(x + 1, y) && grid.at(x - 1, y) && grid.at(x, y + 1) && grid.at(x, y - 1) {
                return tile_at(x, y);
            }
        }
        tile_at(room.center().x as i32, room.center().y as i32)
    };

    let combat: Vec<usize> = rooms.iter().enumerate().filter(|(_, r)| r.kind == RoomKind::Combat).map(|(i, _)| i).collect();
    for (n, &i) in combat.iter().enumerate() {
        let room = rooms[i].clone();
        let pack = if spec.arena { spec.pack } else { spec.pack + (n as u32 / 2) };
        for _ in 0..pack {
            let kind = MonsterKind::pick_in(spec.region, spec.level, rng.next_f32());
            things.push(Thing::Monster { kind: if spec.arena { MonsterKind::Wolf } else { kind }, at: free_point(&room, &grid, &mut rng) });
        }
        if spec.generators && n >= 1 && !spec.arena {
            things.push(Thing::Generator { at: free_point(&room, &grid, &mut rng) });
        }
        for _ in 0..3 {
            things.push(Thing::Pickup { kind: PickupKind::Coin, at: free_point(&room, &grid, &mut rng) });
        }
        if n % 2 == 0 {
            things.push(Thing::Pickup { kind: PickupKind::Food, at: free_point(&room, &grid, &mut rng) });
        }
    }
    // Une meute de loups en plus, répartie à parts égales entre les clairières.
    for k in 0..spec.extra_wolves as usize {
        let room = rooms[combat[k % combat.len()]].clone();
        things.push(Thing::Monster { kind: MonsterKind::Wolf, at: free_point(&room, &grid, &mut rng) });
    }
    // Quelques potions, dans des clairières différentes.
    for k in 0..2.min(combat.len()) {
        let room = rooms[combat[(k * 2 + 1) % combat.len()]].clone();
        things.push(Thing::Pickup { kind: PickupKind::Potion, at: free_point(&room, &grid, &mut rng) });
    }
    // Ce qu'il faut trouver pour accomplir l'objectif, réparti dans les clairières.
    if let Some((kind, goal)) = spec.quest {
        for k in 0..goal as usize {
            let room = rooms[combat[(combat.len() - 1 - k % combat.len().max(1)).min(combat.len() - 1)]].clone();
            match kind {
                QuestKind::Retrieve => things.push(Thing::Relic { at: free_point(&room, &grid, &mut rng) }),
                QuestKind::Rescue => things.push(Thing::Captive { at: free_point(&room, &grid, &mut rng) }),
                QuestKind::Hunt | QuestKind::Repair | QuestKind::Release | QuestKind::Unseal | QuestKind::Breach => {}
            }
        }
    }
    for k in 0..spec.captives as usize {
        let room = rooms[combat[(combat.len() - 1 - k % combat.len().max(1)).min(combat.len() - 1)]].clone();
        things.push(Thing::Captive { at: free_point(&room, &grid, &mut rng) });
    }
    let exit_room = rooms.last().cloned().expect("une zone de sortie");
    things.push(Thing::Exit { kind: spec.exit, at: tile_at(exit_room.center().x as i32, exit_room.center().y as i32) });

    if let Some(mechanism) = &spec.mechanism {
        // Le panneau est posé sur un tronc ou un rocher (une tuile de mur) qui borde la clairière de sortie, face au sud.
        let center = exit_room.center().as_ivec2();
        let mut panel = None;
        'search: for dx in [0, -1, 1, -2, 2, -3, 3, -4, 4] {
            for y in (exit_room.min.y as i32..=exit_room.max.y as i32).rev() {
                let x = center.x + dx;
                if grid.at(x, y) && !grid.at(x, y + 1) && y + 1 < h - 1 {
                    panel = Some(tile_at(x, y + 1));
                    break 'search;
                }
            }
        }
        let panel = panel.expect("un mur au bord de la clairière de sortie");
        things.push(Thing::Panel { kind: mechanism.kind, at: panel, node: mechanism.panel.to_string() });
        // Ce qu'il faut trouver pour l'actionner, dans des clairières différentes (le plus loin du départ en premier).
        let (pickup, count) = match mechanism.kind {
            InteractKind::Target => (PickupKind::TargetDisc, 1),
            InteractKind::Mechanism => (PickupKind::GearPart, 1),
            InteractKind::Rune => (PickupKind::Ingredient, 3),
            InteractKind::Push => (PickupKind::Coin, 0),
        };
        for k in 0..count {
            let room = rooms[combat[(combat.len() - 1 - k % combat.len().max(1)).min(combat.len() - 1)]].clone();
            things.push(Thing::Pickup { kind: pickup, at: free_point(&room, &grid, &mut rng) });
        }
        // La scène qui présente le mécanisme, dès que l'équipe arrive à l'abri des ennemis.
        let start = rooms[0].center();
        things.push(Thing::Trigger { node: mechanism.intro.to_string(), at: tile_at(start.x as i32, start.y as i32), radius: 200.0, delay: 2.0 });
    }

    LevelDef {
        format: LEVEL_FORMAT,
        name: spec.name.to_string(),
        region: spec.region,
        size,
        rows,
        layout: Layout { rooms, connections, start: 0, exit: count - 1, treasure: None },
        quest: spec.quest,
        things,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(region: Region, arena: bool) -> OutdoorSpec {
        OutdoorSpec {
            name: "essai",
            region,
            level: 2,
            clearings: 3,
            pack: 3,
            extra_wolves: 0,
            generators: true,
            quest: Some((QuestKind::Hunt, 6)),
            exit: ExitKind::Gate,
            arena,
            mechanism: Some(Mechanism { kind: InteractKind::Rune, intro: "gate_intro", panel: "gate_panel" }),
            captives: 1,
        }
    }

    #[test]
    fn outdoor_levels_are_valid_for_many_seeds() {
        for seed in 0..60u64 {
            for (region, arena) in [(Region::Forest, false), (Region::Marsh, false), (Region::Approach, false), (Region::Approach, true)] {
                let def = outdoor_level(&spec(region, arena), seed);
                assert!(def.validate().is_empty(), "{region:?} arène {arena} graine {seed}: {:?}", def.validate());
                assert!(def.things.iter().any(|t| matches!(t, Thing::Monster { .. })));
            }
        }
    }
}
