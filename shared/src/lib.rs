//! Types partagés entre client et serveur de Castle.

use bevy::prelude::*;

mod dungeon;
pub mod balance;
pub mod campaign;
pub mod decor;
pub mod level;
pub mod save;
pub mod tileart;
pub use balance::{Balance, balance};
pub use dungeon::*;
pub use campaign::{ACT_COUNT, BOSS_EVERY, LEVEL_COUNT, act_of, is_boss_level, level_def, level_file, override_level};
pub use decor::{DECOR_VARIANTS, Decor, DecorKind, Landmark, LandmarkKind};
pub use level::{DIALOGUE_NODES, LEVEL_FORMAT, Link, LinkEnd, LevelDef, Thing};
use serde::{Deserialize, Serialize};

pub const TILE_SIZE: f32 = 16.0;
/// Taille de la salle de test, en tuiles.
/// Demi-taille de la boîte de collision d'un héros, en pixels.
pub const HERO_HALF_SIZE: f32 = 6.0;

/// Les quatre héros fondateurs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeroClass {
    #[default]
    Knight,
    Sorcerer,
    Ranger,
    Forgebraise,
}

/// Demi-côté de la boîte d'un générateur de monstres, en pixels.
pub const GENERATOR_HALF_SIZE: f32 = 8.0;
/// Rayon d'un projectile, en pixels.
pub const PROJECTILE_RADIUS: f32 = 3.0;

/// Distance maximale pour agir sur un objet (touche d'action).
pub const INTERACT_RANGE: f32 = 18.0;

/// Attaque principale d'une classe.
#[derive(Clone, Copy, Debug)]
pub enum Attack {
    /// Coup au corps à corps dans un cône devant le héros.
    Melee { range: f32, damage: f32, cooldown: f32 },
    /// Projectile tiré droit devant.
    Shot { speed: f32, damage: f32, cooldown: f32, lifetime: f32 },
}

impl Attack {
    pub fn cooldown(self) -> f32 {
        match self {
            Self::Melee { cooldown, .. } | Self::Shot { cooldown, .. } => cooldown,
        }
    }
}

impl HeroClass {
    pub const ALL: [HeroClass; 4] = [
        HeroClass::Knight,
        HeroClass::Sorcerer,
        HeroClass::Ranger,
        HeroClass::Forgebraise,
    ];
}

/// Statistiques de base d'une classe.
/// `name_key` et `special_key` sont des clés de localisation (Fluent), jamais du texte affiché.
pub struct ClassStats {
    pub name_key: &'static str,
    pub max_hp: u32,
    pub speed: f32,
    pub special_key: &'static str,
    pub max_mana: f32,
    /// Mana regagné par seconde.
    pub mana_regen: f32,
    pub special_cost: f32,
    /// Temps de recharge du pouvoir spécial, en secondes.
    pub special_cooldown: f32,
    pub attack: Attack,
}

impl HeroClass {
    /// Toutes les valeurs d'équilibrage de la classe (dont celles de son comportement de compagnon).
    pub fn tuning(self) -> &'static balance::Hero {
        let heroes = &balance().heroes;
        match self {
            Self::Knight => &heroes.knight,
            Self::Sorcerer => &heroes.sorcerer,
            Self::Ranger => &heroes.ranger,
            Self::Forgebraise => &heroes.forgebraise,
        }
    }

    /// Les statistiques de la classe: les valeurs viennent de l'équilibrage (`balance.ron`); le genre d'attaque (coup ou tir), les
    /// clés de traduction et le pouvoir de chaque classe sont dans le code.
    pub fn stats(self) -> ClassStats {
        let heroes = &balance().heroes;
        let (name_key, special_key, melee, h) = match self {
            Self::Knight => ("class-knight", "special-knight", true, &heroes.knight),
            Self::Sorcerer => ("class-sorcerer", "special-sorcerer", false, &heroes.sorcerer),
            Self::Ranger => ("class-ranger", "special-ranger", false, &heroes.ranger),
            Self::Forgebraise => ("class-forgebraise", "special-forgebraise", true, &heroes.forgebraise),
        };
        let attack = if melee {
            Attack::Melee { range: h.attack_range, damage: h.attack_damage, cooldown: h.attack_cooldown }
        } else {
            Attack::Shot { speed: h.shot_speed, damage: h.attack_damage, cooldown: h.attack_cooldown, lifetime: h.shot_lifetime }
        };
        ClassStats {
            name_key,
            max_hp: h.max_hp,
            speed: h.speed,
            special_key,
            max_mana: h.max_mana,
            mana_regen: h.mana_regen,
            special_cost: h.special_cost,
            special_cooldown: h.special_cooldown,
            attack,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    /// Eau: infranchissable à pied. On ne la traverse que sur un pont.
    Water,
    /// Pont: une tuile praticable posée sur l'eau.
    Bridge,
}

impl Tile {
    /// Les héros et les monstres peuvent-ils marcher dessus ?
    pub fn walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::Bridge)
    }
}

/// Carte d'une salle en tuiles. Le serveur et les clients construisent la même, sans la répliquer.
/// L'origine du monde est au centre de la salle, l'axe y vers le haut.
#[derive(Resource, Clone, Debug)]
pub struct Room {
    size: UVec2,
    tiles: Vec<Tile>,
}

impl Room {
    /// Une salle dessinée en texte, pour les essais et les aperçus: `#` est un mur, `~` de l'eau, `=` un pont, tout autre caractère du
    /// sol. La première ligne est le nord; toutes les lignes ont la même longueur.
    pub fn from_rows(rows: &[&str]) -> Room {
        let size = UVec2::new(rows[0].chars().count() as u32, rows.len() as u32);
        let mut room = Room { size, tiles: vec![Tile::Floor; (size.x * size.y) as usize] };
        for (row, text) in rows.iter().enumerate() {
            assert_eq!(text.chars().count() as u32, size.x, "ligne {row} de longueur différente");
            for (x, c) in text.chars().enumerate() {
                let tile = match c {
                    '#' => Tile::Wall,
                    '~' => Tile::Water,
                    '=' => Tile::Bridge,
                    _ => continue,
                };
                room.set(x as u32, size.y - 1 - row as u32, tile);
            }
        }
        room
    }

    fn set(&mut self, x: u32, y: u32, tile: Tile) {
        let i = (y * self.size.x + x) as usize;
        self.tiles[i] = tile;
    }

    pub fn size(&self) -> UVec2 {
        self.size
    }

    /// Tuile aux coordonnées de grille. Hors de la salle, c'est un mur.
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.size.x as i32 || y >= self.size.y as i32 {
            return Tile::Wall;
        }
        self.tiles[(y as u32 * self.size.x + x as u32) as usize]
    }

    /// Centre d'une tuile, en coordonnées du monde.
    pub fn tile_center(&self, x: u32, y: u32) -> Vec2 {
        (Vec2::new(x as f32, y as f32) + Vec2::splat(0.5) - self.size.as_vec2() / 2.0) * TILE_SIZE
    }

    /// Le coin de tuiles où poser un pont cassé (deux tuiles sur deux, en coordonnées de tuiles) quand on vise `at`: parmi les coins à deux
    /// tuiles ou moins du plus proche, celui dont les quatre tuiles contiennent le plus de pont, à égalité le plus proche de `at`. Le bloc couvre ainsi
    /// toute la largeur du pont même si on l'a visé à une tuile près.
    pub fn bridge_block_corner(&self, at: Vec2) -> Vec2 {
        let base = at.round();
        let mut best = (base, f32::NEG_INFINITY);
        for dy in -2..=2 {
            for dx in -2..=2 {
                let corner = base + Vec2::new(dx as f32, dy as f32);
                let bridges = self.bridge_block(corner).iter().filter(|(x, y)| self.tile(*x, *y) == Tile::Bridge).count() as f32;
                let score = bridges * 10.0 - corner.distance(at);
                if score > best.1 {
                    best = (corner, score);
                }
            }
        }
        best.0
    }

    /// Les quatre tuiles d'un bloc de deux sur deux dont le coin central est `corner`.
    pub fn bridge_block(&self, corner: Vec2) -> [(i32, i32); 4] {
        let (x, y) = (corner.x as i32, corner.y as i32);
        [(x - 1, y - 1), (x, y - 1), (x - 1, y), (x, y)]
    }

    /// Une boîte centrée en `center`, de demi-côté `half`, touche-t-elle un mur ou de l'eau (ce qu'on ne traverse pas à pied) ?
    pub fn overlaps_wall(&self, center: Vec2, half: f32) -> bool {
        self.overlaps_tile(center, half, |tile| !tile.walkable())
    }

    /// Comme `overlaps_wall`, mais seuls les murs comptent: les tirs et le regard passent au-dessus de l'eau.
    pub fn overlaps_opaque(&self, center: Vec2, half: f32) -> bool {
        self.overlaps_tile(center, half, |tile| tile == Tile::Wall)
    }

    fn overlaps_tile(&self, center: Vec2, half: f32, blocks: impl Fn(Tile) -> bool) -> bool {
        let to_grid = |v: Vec2| ((v / TILE_SIZE) + self.size.as_vec2() / 2.0).floor().as_ivec2();
        // On retire un epsilon: coller exactement un mur n'est pas le traverser.
        let min = to_grid(center - Vec2::splat(half));
        let max = to_grid(center + Vec2::splat(half - 0.001));
        (min.y..=max.y).any(|y| (min.x..=max.x).any(|x| blocks(self.tile(x, y))))
    }

    /// Déplace une boîte en glissant le long des murs (axes testés séparément).
    pub fn move_and_collide(&self, pos: Vec2, delta: Vec2, half: f32) -> Vec2 {
        self.move_and_collide_solids(pos, delta, half, &[])
    }

    /// Tuile (colonne, ligne) qui contient ce point du monde.
    pub fn world_to_tile(&self, p: Vec2) -> IVec2 {
        ((p / TILE_SIZE) + self.size.as_vec2() / 2.0).floor().as_ivec2()
    }

    fn is_floor(&self, t: IVec2) -> bool {
        self.tile(t.x, t.y).walkable()
    }

    /// Une boîte de demi-côté `half` peut-elle aller en ligne droite de `from` à `to` sans toucher un mur ?
    pub fn segment_clear(&self, from: Vec2, to: Vec2, half: f32) -> bool {
        let d = to - from;
        let steps = (d.length() / 3.0).ceil().max(1.0) as u32;
        (0..=steps).all(|i| !self.overlaps_wall(from + d * (i as f32 / steps as f32), half))
    }

    /// Le regard passe-t-il en ligne droite de `from` à `to` ? Les murs l'arrêtent, pas l'eau (on voit et on tire par-dessus).
    pub fn segment_clear_sight(&self, from: Vec2, to: Vec2, half: f32) -> bool {
        let d = to - from;
        let steps = (d.length() / 3.0).ceil().max(1.0) as u32;
        (0..=steps).all(|i| !self.overlaps_opaque(from + d * (i as f32 / steps as f32), half))
    }

    /// Comme `segment_clear`, en tenant compte aussi des obstacles rectangulaires (centre, demi-dimensions): un obstacle que la boîte
    /// chevauche déjà au départ est ignoré, pour qu'on puisse s'en dégager.
    pub fn segment_clear_solids(&self, from: Vec2, to: Vec2, half: f32, solids: &[(Vec2, Vec2)]) -> bool {
        if !self.segment_clear(from, to, half) {
            return false;
        }
        let hits = |p: Vec2, (c, h): &(Vec2, Vec2)| (p.x - c.x).abs() < h.x + half && (p.y - c.y).abs() < h.y + half;
        let d = to - from;
        let steps = (d.length() / 3.0).ceil().max(1.0) as u32;
        solids.iter().filter(|s| !hits(from, s)).all(|s| (0..=steps).all(|i| !hits(from + d * (i as f32 / steps as f32), s)))
    }

    /// Chemin de `from` à `to` à travers les salles et les couloirs, sous forme de points de passage.
    ///
    /// A* sur la grille de tuiles (déplacements en diagonale sans couper les coins), puis le chemin est
    /// raccourci en ligne droite partout où une boîte de héros passe. Les tuiles recouvertes par un
    /// obstacle de `solids` (portes fermées) sont infranchissables. `None` si le but est inaccessible.
    pub fn find_path(&self, from: Vec2, to: Vec2, solids: &[(Vec2, Vec2)]) -> Option<Vec<Vec2>> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let blocked = |t: IVec2| {
            !self.is_floor(t)
                || solids.iter().any(|(c, h)| {
                    let tc = self.tile_center(t.x as u32, t.y as u32);
                    (tc.x - c.x).abs() < h.x + HERO_HALF_SIZE && (tc.y - c.y).abs() < h.y + HERO_HALF_SIZE
                })
        };
        // Le départ ou le but peuvent tomber sur un mur (formation, bord de couloir): on cherche la tuile libre la plus proche.
        let nearest_free = |p: Vec2| {
            let t = self.world_to_tile(p);
            (0..=3).find_map(|r| {
                (-r..=r)
                    .flat_map(|dy| (-r..=r).map(move |dx| IVec2::new(dx, dy)))
                    .map(|d| t + d)
                    .filter(|c| !blocked(*c))
                    .min_by_key(|c| (*c - t).length_squared())
            })
        };
        let (start, goal) = (nearest_free(from)?, nearest_free(to)?);
        let width = self.size.x as i32;
        let index = |t: IVec2| (t.y * width + t.x) as usize;
        let mut cost = vec![u32::MAX; (self.size.x * self.size.y) as usize];
        let mut came_from = vec![usize::MAX; cost.len()];
        let mut open = BinaryHeap::new();
        let heuristic = |t: IVec2| {
            let d = (goal - t).abs();
            10 * (d.x + d.y) as u32 - 6 * d.x.min(d.y) as u32
        };
        cost[index(start)] = 0;
        open.push(Reverse((heuristic(start), index(start))));
        while let Some(Reverse((_, current))) = open.pop() {
            let t = IVec2::new(current as i32 % width, current as i32 / width);
            if t == goal {
                break;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let n = t + IVec2::new(dx, dy);
                let diagonal = dx != 0 && dy != 0;
                // Pas de coupe de coin: les deux tuiles voisines doivent aussi être libres.
                if blocked(n) || (diagonal && (blocked(t + IVec2::new(dx, 0)) || blocked(t + IVec2::new(0, dy)))) {
                    continue;
                }
                let next = cost[current] + if diagonal { 14 } else { 10 };
                if next < cost[index(n)] {
                    cost[index(n)] = next;
                    came_from[index(n)] = current;
                    open.push(Reverse((next + heuristic(n), index(n))));
                }
            }
        }
        if came_from[index(goal)] == usize::MAX && start != goal {
            return None;
        }

        // Reconstruction, du but vers le départ.
        let mut tiles = vec![goal];
        let mut at = index(goal);
        while at != index(start) {
            at = came_from[at];
            tiles.push(IVec2::new(at as i32 % width, at as i32 / width));
        }
        tiles.reverse();
        let mut points: Vec<Vec2> = tiles
            .iter()
            .map(|t| self.tile_center(t.x as u32, t.y as u32))
            .collect();
        // Le dernier point est le but exact quand il est accessible tel quel.
        if !blocked(self.world_to_tile(to)) {
            *points.last_mut()? = to;
        }

        // Raccourcissement: depuis chaque point, aller le plus loin possible en ligne droite.
        let mut route = Vec::new();
        let mut i = 0;
        let mut origin = from;
        while i < points.len() {
            let mut j = points.len() - 1;
            while j > i && !self.segment_clear_solids(origin, points[j], HERO_HALF_SIZE, solids) {
                j -= 1;
            }
            route.push(points[j]);
            origin = points[j];
            i = j + 1;
        }
        Some(route)
    }

    /// Comme `move_and_collide`, avec des obstacles rectangulaires supplémentaires (centre, demi-dimensions):
    /// portes, monstres, générateurs. Un obstacle que la boîte chevauche déjà est ignoré, pour qu'on puisse
    /// toujours s'en dégager au lieu de rester coincé dedans.
    pub fn move_and_collide_solids(
        &self,
        pos: Vec2,
        delta: Vec2,
        half: f32,
        solids: &[(Vec2, Vec2)],
    ) -> Vec2 {
        let overlaps = |c: Vec2, (sc, sh): &(Vec2, Vec2)| {
            (c.x - sc.x).abs() < sh.x + half && (c.y - sc.y).abs() < sh.y + half
        };
        let blocked = |c: Vec2| {
            self.overlaps_wall(c, half)
                || solids.iter().any(|s| overlaps(c, s) && !overlaps(pos, s))
        };
        let mut p = pos;
        let try_x = Vec2::new(p.x + delta.x, p.y);
        if !blocked(try_x) {
            p = try_x;
        }
        let try_y = Vec2::new(p.x, p.y + delta.y);
        if !blocked(try_y) {
            p = try_y;
        }
        p
    }
}

/// Marque l'entité d'un joueur (c'est l'entité de connexion du client côté serveur).
#[derive(Component, Serialize, Deserialize)]
pub struct Player;

/// Points de vie.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterKind {
    Skeleton,
    Bat,
    /// Petit, vif et fragile.
    Goblin,
    /// Massif, lent, frappe fort.
    Orc,
    /// Décharné, rapide et mordant.
    Ghoul,
    /// Rapide et vorace, il chasse en meute (forêt, marais, abords du château).
    Wolf,
    /// Boss du niveau 4: le chef de la meute, un loup énorme qui hurle pour rameuter des loups.
    Warg,
    /// Boss du niveau 8: un troll couronné, lent, qui frappe le sol de ses poings.
    Troll,
    /// Boss du niveau 12: Vaelor, un colosse à la peau orange et à la crinière blonde, qui lance de la foudre noire et appelle des squelettes.
    Vaelor,
    /// Boss de l'acte 2 (niveau 8): le Jardinier Noir, un elfe fondu à la forêt, qui frappe de ses racines et appelle des gobelins.
    Gardener,
    /// Boss de l'acte 4 (niveau 16): le Dragon de la Forge, gardien des profondeurs, qui lance des ondes de feu.
    Dragon,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonsterStats {
    pub max_hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub cooldown: f32,
    pub half_size: f32,
    /// Distance à laquelle il repère un héros.
    pub sight: f32,
}

impl MonsterKind {
    pub fn stats(self) -> MonsterStats {
        let m = &balance().monsters;
        match self {
            Self::Skeleton => m.skeleton,
            Self::Bat => m.bat,
            Self::Goblin => m.goblin,
            Self::Orc => m.orc,
            Self::Ghoul => m.ghoul,
            Self::Wolf => m.wolf,
            Self::Warg => m.warg,
            Self::Troll => m.troll,
            Self::Vaelor => m.vaelor,
            Self::Gardener => m.gardener,
            Self::Dragon => m.dragon,
        }
    }

    /// Un boss: personnage unique du niveau, qu'il faut vaincre en groupe pour libérer la sortie.
    pub fn is_boss(self) -> bool {
        matches!(self, Self::Warg | Self::Troll | Self::Vaelor | Self::Gardener | Self::Dragon)
    }

    /// Clé de traduction du nom d'un boss (barre de vie).
    pub fn boss_name_key(self) -> Option<&'static str> {
        match self {
            Self::Warg => Some("boss-warg"),
            Self::Troll => Some("boss-troll"),
            Self::Vaelor => Some("boss-vaelor"),
            Self::Gardener => Some("boss-gardener"),
            Self::Dragon => Some("boss-dragon"),
            _ => None,
        }
    }

    /// Les monstres qu'un générateur peut produire à ce niveau, avec leur poids relatif: le gobelin arrive d'emblée,
    /// la goule au niveau 2 et l'orque au niveau 3, pour que la difficulté monte avec le château (tables de `balance.ron`).
    pub fn spawn_table(level: u32) -> &'static [balance::SpawnWeight] {
        let spawn = &balance().spawn;
        match level {
            0 | 1 => &spawn.castle_level_1,
            2 => &spawn.castle_level_2,
            _ => &spawn.castle_level_3_and_more,
        }
    }

    /// Les monstres qu'un générateur produit dans cette région: les extérieurs ont leurs propres bêtes, l'intérieur du château
    /// suit la table du niveau.
    pub fn spawn_table_in(region: Region, level: u32) -> &'static [balance::SpawnWeight] {
        let spawn = &balance().spawn;
        match region {
            Region::Forest => &spawn.forest,
            Region::Marsh => &spawn.marsh,
            Region::Approach => &spawn.approach,
            _ => Self::spawn_table(level),
        }
    }

    /// Le monstre tiré par `roll` (entre 0 et 1) dans la table de cette région (voir `spawn_table_in`).
    pub fn pick_in(region: Region, level: u32, roll: f32) -> MonsterKind {
        Self::pick_from(Self::spawn_table_in(region, level), roll)
    }

    /// Le monstre tiré par `roll` (entre 0 et 1) dans la table du niveau.
    pub fn pick(level: u32, roll: f32) -> MonsterKind {
        Self::pick_from(Self::spawn_table(level), roll)
    }

    /// Le monstre tiré par `roll` (entre 0 et 1) dans une table donnée.
    pub fn pick_from_table(table: &'static [balance::SpawnWeight], roll: f32) -> MonsterKind {
        Self::pick_from(table, roll)
    }

    fn pick_from(table: &'static [balance::SpawnWeight], roll: f32) -> MonsterKind {
        let total: f32 = table.iter().map(|w| w.weight).sum();
        let mut left = roll.clamp(0.0, 1.0) * total;
        for w in table {
            if left < w.weight {
                return w.monster;
            }
            left -= w.weight;
        }
        table[table.len() - 1].monster
    }
}

/// Monstre d'un certain type.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Monster(pub MonsterKind);

/// Générateur de monstres, destructible.
#[derive(Component, Serialize, Deserialize)]
pub struct Generator;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupKind {
    Coin,
    Food,
    Key,
    /// Ingrédient du sort qui dissipe l'enchantement de la porte royale (violet, pour l'Ensorceleur).
    Ingredient,
    /// Pièce de rechange du mécanisme grippé (orange, pour le Forgebraise).
    GearPart,
    /// Disque de la cible de la herse (vert, pour le Rôdeur).
    TargetDisc,
    /// Potion de régénération: soigne peu à peu toute l'équipe de celui qui la boit.
    Potion,
}

/// Sur un héros qui boit une potion de régénération: le client le teinte de vert tant que l'effet dure.
#[derive(Component, Serialize, Deserialize)]
pub struct Regenerating;

/// Objet ramassable au sol.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Pickup(pub PickupKind);

/// Genres d'objectifs de niveau: le portail de sortie n'apparaît qu'une fois l'objectif rempli.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestKind {
    /// Vaincre un nombre de monstres.
    Hunt,
    /// Retrouver des reliques dispersées dans les salles de combat.
    Retrieve,
    /// Délivrer des prisonniers enfermés dans les salles de combat.
    Rescue,
    /// Réparer le pont: le Forgebraise remet en marche un mécanisme grippé avec une pièce de rechange.
    Repair,
    /// Libérer le pont-levis: le Rôdeur fixe le disque d'une cible puis la touche d'une flèche.
    Release,
    /// Briser le sceau de la porte: l'Ensorceleur dissipe une rune avec trois ingrédients.
    Unseal,
    /// Forcer la porte royale: le Chevalier la pousse de toute sa force.
    Breach,
    /// Vaincre le boss du niveau.
    Boss,
}

impl QuestKind {
    /// Clé de traduction de l'intitulé.
    pub fn name_key(self) -> &'static str {
        match self {
            Self::Hunt => "quest-hunt",
            Self::Retrieve => "quest-retrieve",
            Self::Rescue => "quest-rescue",
            Self::Repair => "quest-repair",
            Self::Release => "quest-release",
            Self::Unseal => "quest-unseal",
            Self::Breach => "quest-breach",
            Self::Boss => "quest-boss",
        }
    }

    /// Le mécanisme dont l'accomplissement remplit cet objectif, pour les objectifs propres à une classe.
    pub fn mechanism(self) -> Option<InteractKind> {
        match self {
            Self::Repair => Some(InteractKind::Mechanism),
            Self::Release => Some(InteractKind::Target),
            Self::Unseal => Some(InteractKind::Rune),
            Self::Breach => Some(InteractKind::Push),
            Self::Hunt | Self::Retrieve | Self::Rescue | Self::Boss => None,
        }
    }

    /// Objectif du niveau `level` (le premier est toujours une chasse), et nombre à atteindre.
    pub fn for_level(level: u32) -> (Self, u32) {
        match (level.max(1) - 1) % 3 {
            0 => (Self::Hunt, (6 + 2 * ((level.max(1) - 1) / 3)).min(18)),
            1 => (Self::Retrieve, 3),
            _ => (Self::Rescue, 2),
        }
    }
}

/// L'objectif du niveau en cours. Entité répliquée: les clients l'affichent.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Quest {
    pub kind: QuestKind,
    pub goal: u32,
    pub progress: u32,
}

impl Quest {
    pub fn is_done(&self) -> bool {
        self.progress >= self.goal
    }
}

/// Relique à retrouver (quête de récupération): se ramasse au contact.
#[derive(Component, Serialize, Deserialize)]
pub struct Relic;

/// Prisonnier à délivrer (quête de sauvetage): un héros doit rester près de lui un instant.
#[derive(Component, Serialize, Deserialize)]
pub struct Captive;

/// Samélia, la princesse des elfes (avec `Captive`, elle est prisonnière dans une cage).
#[derive(Component, Serialize, Deserialize)]
pub struct Guide;

/// Pièces d'un héros.
#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Gold(pub u32);

/// Clés d'un héros.
#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Keys(pub u32);

/// Projectile tiré par un héros de cette classe, dans la direction `dir`.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Projectile {
    pub class: HeroClass,
    pub dir: Vec2,
}

/// Compteur de coups d'un héros (attaque ou pouvoir): le client lance l'animation d'arme à chaque changement.
#[derive(Component, Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct Strikes(pub u8);

/// Direction vers laquelle regarde un héros (vecteur unitaire): sert à choisir la vue du sprite.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Facing(pub Vec2);

/// Héros à terre: il ne peut plus agir, un allié peut le relever.
#[derive(Component, Serialize, Deserialize)]
pub struct Downed;

/// Mana d'un héros, dépensé par les pouvoirs spéciaux.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Mana {
    pub current: f32,
    pub max: f32,
}

impl Mana {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// Héros protégé par le Rempart du Chevalier (dégâts reçus réduits).
#[derive(Component, Serialize, Deserialize)]
pub struct Shielded;

/// Zone d'effet au sol (la Frappe titanesque du Forgebraise), lancée par un héros de cette classe.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Zone {
    pub radius: f32,
    pub class: HeroClass,
}

/// Un monstre figé par le pouvoir de l'Ensorceleur: il ne bouge plus et n'attaque plus pendant ce nombre de secondes.
#[derive(Component, Clone, Copy, Debug)]
pub struct Frozen(pub f32);

/// Obstacles de la porte royale: chacun demande une classe précise, et la couleur de son panneau est celle de cette
/// classe (violet pour l'Ensorceleur, orange pour le Forgebraise, vert pour le Rôdeur, acier pour le Chevalier).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractKind {
    /// Cible de la herse, dans le mur: le Rôdeur y fixe le disque trouvé, puis la touche d'une flèche.
    Target,
    /// Rune de l'enchantement, dans le mur: l'Ensorceleur y verse les ingrédients trouvés pour le dissiper.
    Rune,
    /// Mécanisme grippé, dans le mur: le Forgebraise y monte la pièce de rechange trouvée.
    Mechanism,
    /// La porte elle-même, que le Chevalier pousse une fois le reste fait.
    Push,
}

impl InteractKind {
    pub fn class(self) -> HeroClass {
        match self {
            Self::Target => HeroClass::Ranger,
            Self::Rune => HeroClass::Sorcerer,
            Self::Mechanism => HeroClass::Forgebraise,
            Self::Push => HeroClass::Knight,
        }
    }

    /// Temps pendant lequel il faut maintenir la touche d'action, en secondes. La cible ne demande que de fixer le disque.
    pub fn hold_time(self) -> f32 {
        match self {
            Self::Target => 0.8,
            Self::Rune => 1.5,
            Self::Mechanism => 2.5,
            Self::Push => 3.0,
        }
    }
}

/// Objet sur lequel on agit en maintenant la touche d'action.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Interactable {
    pub kind: InteractKind,
    /// Avancement de l'action en cours, de 0 à 1.
    pub progress: f32,
    pub done: bool,
    /// Cible seulement: le disque est fixé, il ne reste qu'à le toucher d'une flèche.
    pub mounted: bool,
}

impl Interactable {
    pub fn new(kind: InteractKind) -> Self {
        Self { kind, progress: 0.0, done: false, mounted: false }
    }
}

/// Ce que l'équipe a trouvé pour l'énigme de la porte royale (une seule réserve, partagée par tous les joueurs).
#[derive(Component, Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PuzzleKit {
    pub ingredients: u8,
    pub gear: u8,
    pub target: u8,
}

/// Nombre d'ingrédients nécessaires pour dissiper l'enchantement.
pub const INGREDIENTS_NEEDED: u8 = 3;

/// Enchantement qui repousse tout héros à moins de `radius` du centre de la porte royale.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Ward {
    pub radius: f32,
}

/// Herse qui barre le couloir devant la porte royale, jusqu'à ce que la cible soit touchée.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Portcullis {
    pub vertical: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorKind {
    /// S'ouvre au contact d'un héros qui a une clé.
    Locked,
    /// La porte royale: s'ouvre quand l'énigme est résolue.
    Royal,
}

/// Porte de deux tuiles; `vertical` quand elle s'étend en hauteur (couloir horizontal).
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Door {
    pub kind: DoorKind,
    pub vertical: bool,
}

/// Genres de sortie d'un niveau (chacun a son décor).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitKind {
    /// Escalier, à l'intérieur du château.
    Stairs,
    /// Barrière de rondins qui ferme le sentier de la forêt.
    Gate,
    /// Pont du marais, brisé puis réparé.
    Bridge,
    /// Pont-levis du château, levé puis baissé.
    Drawbridge,
    /// Sans dessin dans le jeu (visible seulement dans l'éditeur): la fin d'un chemin, par exemple.
    Hidden,
}

/// Sortie du niveau: quand tous les héros debout y sont, le niveau est terminé. Elle est fermée tant que l'objectif du
/// niveau n'est pas rempli.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LevelExit {
    pub kind: ExitKind,
    pub open: bool,
}

/// Piège au sol. Invisible sauf à proximité d'un héros qui détecte; le Rôdeur peut le désamorcer.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Trap {
    pub armed: bool,
}

/// Position d'une entité dans le monde, en pixels. Autoritaire côté serveur.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Position(pub Vec2);

/// Entrée d'un joueur, envoyée du client vers le serveur (serveur autoritaire).
#[derive(Event, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct PlayerInput {
    pub direction: Vec2,
    pub attack: bool,
    pub special: bool,
    /// Touche d'action maintenue (levier, rune, mécanisme, désamorçage).
    pub interact: bool,
}

/// Coup au corps à corps, envoyé aux clients pour l'effet visuel.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AttackFx {
    pub pos: Vec2,
    pub dir: Vec2,
    pub class: HeroClass,
}

/// Un héros lance son pouvoir spécial, pour l'effet visuel: `from` est le lanceur, `at` le centre de l'effet (là où la frappe tombe, ou
/// le lanceur), `dir` sa direction, `radius` la portée en unités du monde (l'effet la montre).
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SpecialFx {
    pub class: HeroClass,
    pub from: Vec2,
    pub at: Vec2,
    pub dir: Vec2,
    pub radius: f32,
}

/// Le serveur déclenche une scène de dialogue: chaque client la joue dans sa langue.
/// Le serveur ne transmet que le nom du nœud; les textes sont côté client.
#[derive(Event, Clone, Debug, Serialize, Deserialize)]
pub struct DialogueBegin(pub String);

/// Passe à la réplique suivante, pour tous les joueurs en même temps.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueAdvance;

/// Résultat du vote du groupe: indice du choix retenu.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueSelect(pub u32);

/// Un joueur demande la réplique suivante.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueRequestAdvance;

/// Vote d'un joueur parmi les choix proposés.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueVote(pub u32);

/// Un client a terminé la scène.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueFinished;

/// Le serveur demande un léger tremblement d'écran (générateur détruit, énigme résolue, porte ouverte):
/// l'intensité va de 0 (rien) à 1 (fort).
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ScreenShake(pub f32);

/// Le geste d'un boss que les héros doivent esquiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossMove {
    /// Coup de poing sur le sol: dégâts dans le cercle.
    Slam,
    /// Onde de foudre noire autour de Vaelor: dégâts dans le cercle.
    Nova,
    /// Cri qui appelle des renforts (pas de zone à éviter).
    Howl,
}

/// Un boss prépare un geste: les clients dessinent la zone à éviter pendant `delay` secondes, jusqu'au coup.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BossAlert {
    pub kind: BossMove,
    pub at: Vec2,
    pub radius: f32,
    pub delay: f32,
}

/// Bilan du niveau qui vient d'être terminé, envoyé quand l'équipe entre dans le portail: les clients l'affichent
/// pendant que le serveur attend avant de générer le niveau suivant.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LevelSummary {
    /// Numéro du niveau terminé.
    pub level: u32,
    /// Temps passé dans le niveau, en secondes.
    pub seconds: f32,
    /// Monstres vaincus et générateurs détruits.
    pub kills: u32,
    pub generators: u32,
    /// Points de vie perdus par l'équipe, et nombre de fois qu'un héros est tombé.
    pub damage_taken: f32,
    pub downed: u32,
    /// Butin ramassé: or, clés, nourriture, potions et éléments d'énigme.
    pub gold: u32,
    pub keys: u32,
    pub food: u32,
    pub potions: u32,
    pub pieces: u32,
}

/// Le joueur a validé le bilan de fin de niveau: le niveau suivant démarre quand tous les joueurs l'ont fait.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ContinueLevel;

/// Un joueur a maintenu Entrée: le serveur purge le dialogue en cours et ceux sur le point de démarrer.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueSkip;

/// Le serveur a purgé les dialogues: chacun abandonne la scène qu'il affichait.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DialogueAbort;

/// Le joueur ouvre ou ferme le menu de pause. Le serveur ne s'arrête que pour une partie solo.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SetPause(pub bool);

/// Le joueur prend le contrôle du héros de cette classe dans son équipe.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SwitchHero(pub HeroClass);

/// Équipe à laquelle appartient un héros: chaque joueur connecté dirige une équipe de quatre héros.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyId(pub u32);

/// Envoyé par le serveur à un client pour lui dire quel héros il dirige en ce moment, et son équipe.
#[derive(Event, Clone, Copy, Debug)]
pub struct YouAre {
    pub hero: Entity,
    pub party: u32,
}

/// Marque tout ce que la simulation pose dans le monde d'une partie (héros, monstres, objets, niveau): il disparaît avec elle.
#[derive(Component)]
pub struct WorldEntity;

#[cfg(test)]
mod tests {
    use super::*;

    /// Arène de 20x12 tuiles: sol entouré de murs.
    fn arena() -> Room {
        let size = UVec2::new(20, 12);
        let mut room = Room { size, tiles: vec![Tile::Floor; (size.x * size.y) as usize] };
        for x in 0..size.x {
            room.set(x, 0, Tile::Wall);
            room.set(x, size.y - 1, Tile::Wall);
        }
        for y in 0..size.y {
            room.set(0, y, Tile::Wall);
            room.set(size.x - 1, y, Tile::Wall);
        }
        room
    }

    #[test]
    fn border_blocks_and_center_is_free() {
        let room = arena();
        assert!(!room.overlaps_wall(Vec2::ZERO, HERO_HALF_SIZE));
        assert!(room.overlaps_wall(room.tile_center(0, 5), HERO_HALF_SIZE));
        assert!(room.overlaps_wall(Vec2::splat(10_000.0), HERO_HALF_SIZE));
    }

    #[test]
    fn slides_along_a_wall() {
        let room = arena();
        // Contre le mur du bas, on avance en x mais pas vers le bas.
        let start = Vec2::new(-40.0, room.tile_center(1, 1).y);
        let end = room.move_and_collide(start, Vec2::new(10.0, -10.0), HERO_HALF_SIZE);
        assert_eq!(end.y, start.y);
        assert_eq!(end.x, start.x + 10.0);
    }

    #[test]
    fn path_goes_around_a_wall() {
        let mut room = arena();
        // Un mur vertical au milieu, avec un passage tout en haut.
        for y in 1..9 {
            room.set(10, y, Tile::Wall);
        }
        let (from, to) = (room.tile_center(3, 3), room.tile_center(16, 3));
        assert!(!room.segment_clear(from, to, HERO_HALF_SIZE));
        let path = room.find_path(from, to, &[]).expect("un chemin existe");
        let mut at = from;
        for waypoint in &path {
            assert!(room.segment_clear(at, *waypoint, HERO_HALF_SIZE), "segment bloqué vers {waypoint}");
            at = *waypoint;
        }
        assert!(at.distance(to) < 1.0);
        assert!(path.len() >= 2, "il faut au moins un détour");
    }

    #[test]
    fn closed_door_blocks_the_path() {
        let room = arena();
        // Une porte qui ferme toute la hauteur de l'arène.
        let door = [(Vec2::ZERO, Vec2::new(8.0, 200.0))];
        let (from, to) = (room.tile_center(3, 5), room.tile_center(16, 5));
        assert!(room.find_path(from, to, &door).is_none());
        assert!(room.find_path(from, to, &[]).is_some());
    }

    #[test]
    fn can_walk_out_of_an_overlapping_body() {
        let room = arena();
        let body = [(Vec2::ZERO, Vec2::splat(8.0))];
        let inside = Vec2::new(2.0, 0.0);
        let out = room.move_and_collide_solids(inside, Vec2::new(5.0, 0.0), HERO_HALF_SIZE, &body);
        assert!(out.x > inside.x, "on doit pouvoir s'éloigner d'un corps qu'on chevauche");
    }

    #[test]
    fn solids_block_movement() {
        let room = arena();
        let start = Vec2::new(-48.0, 0.0);
        let wall = [(Vec2::new(-30.0, 0.0), Vec2::new(8.0, 40.0))];
        let end = room.move_and_collide_solids(start, Vec2::new(30.0, 0.0), HERO_HALF_SIZE, &wall);
        assert_eq!(end, start);
        assert!(room.move_and_collide(start, Vec2::new(30.0, 0.0), HERO_HALF_SIZE).x > start.x);
    }
}

#[cfg(test)]
mod quest_tests {
    use super::*;

    #[test]
    fn quests_rotate_with_the_level() {
        assert_eq!(QuestKind::for_level(1), (QuestKind::Hunt, 6));
        assert_eq!(QuestKind::for_level(2), (QuestKind::Retrieve, 3));
        assert_eq!(QuestKind::for_level(3), (QuestKind::Rescue, 2));
        assert_eq!(QuestKind::for_level(4), (QuestKind::Hunt, 8));
        assert!(QuestKind::for_level(100).1 > 0);
    }
}

#[cfg(test)]
mod monster_tests {
    use super::*;

    #[test]
    fn spawn_tables_are_complete_and_grow_with_the_level() {
        let kinds = |level| MonsterKind::spawn_table(level).iter().map(|w| w.monster).collect::<Vec<_>>();
        for level in 1..8 {
            let total: f32 = MonsterKind::spawn_table(level).iter().map(|w| w.weight).sum();
            assert!((total - 1.0).abs() < 1e-5, "niveau {level}: poids {total}");
        }
        assert!(kinds(1).contains(&MonsterKind::Goblin));
        assert!(!kinds(1).contains(&MonsterKind::Ghoul) && !kinds(1).contains(&MonsterKind::Orc));
        assert!(kinds(2).contains(&MonsterKind::Ghoul) && !kinds(2).contains(&MonsterKind::Orc));
        assert!(kinds(3).contains(&MonsterKind::Orc));
    }

    #[test]
    fn pick_covers_the_whole_table_and_never_panics() {
        for level in [0, 1, 2, 3, 10] {
            let mut seen = std::collections::HashSet::new();
            for step in 0..=1000 {
                seen.insert(format!("{:?}", MonsterKind::pick(level, step as f32 / 1000.0)));
            }
            assert_eq!(seen.len(), MonsterKind::spawn_table(level).len(), "niveau {level}");
        }
        assert_eq!(MonsterKind::pick(3, 0.0), MonsterKind::Skeleton);
        assert_eq!(MonsterKind::pick(3, 1.0), MonsterKind::Orc);
        // Une valeur hors bornes reste dans la table.
        assert_eq!(MonsterKind::pick(1, 5.0), MonsterKind::Goblin);
    }

    /// L'eau ne se traverse pas à pied, sauf sur un pont; les tirs et le regard passent au-dessus de l'eau mais pas des murs.
    #[test]
    fn water_blocks_walking_except_on_a_bridge_and_does_not_block_sight() {
        let no_bridge = Room::from_rows(&["........", "...~~...", "...~~...", "........"]);
        let bridge = Room::from_rows(&["........", "...~~...", "...==...", "........"]);
        let (west, east) = (no_bridge.tile_center(1, 1), no_bridge.tile_center(6, 1));
        assert_eq!(no_bridge.tile(3, 2), Tile::Water);
        assert_eq!(bridge.tile(3, 1), Tile::Bridge);
        // L'eau arrête la marche, pas le regard.
        assert!(!no_bridge.segment_clear(west, east, HERO_HALF_SIZE) || no_bridge.tile(3, 1) != Tile::Water);
        let (a, b) = (no_bridge.tile_center(1, 2), no_bridge.tile_center(6, 2));
        assert!(!no_bridge.segment_clear(a, b, HERO_HALF_SIZE), "on ne marche pas sur l'eau");
        assert!(no_bridge.segment_clear_sight(a, b, 1.0), "on voit et on tire par-dessus l'eau");
        assert!(no_bridge.overlaps_wall(no_bridge.tile_center(3, 2), 2.0) && !no_bridge.overlaps_opaque(no_bridge.tile_center(3, 2), 2.0));
        // Sur le pont, on passe.
        let (c, d) = (bridge.tile_center(1, 1), bridge.tile_center(6, 1));
        assert!(bridge.segment_clear(c, d, HERO_HALF_SIZE), "le pont est praticable");
        // Un mur arrête aussi le regard.
        let wall = Room::from_rows(&["........", "...#....", "........"]);
        assert!(!wall.segment_clear_sight(wall.tile_center(1, 1), wall.tile_center(6, 1), 1.0));
    }

    /// Un pont cassé se cale sur le bloc de deux sur deux qui contient le plus de pont, même visé à une tuile près.
    #[test]
    fn a_broken_bridge_snaps_onto_the_two_wide_bridge() {
        // Un pont de deux tuiles de large (colonnes 3 et 4, lignes 2 et 3 depuis le bas) sur une rivière ouest-est.
        let river = Room::from_rows(&["~~~~~~~~~~", "~~~~~~~~~~", "..========", "..========", "~~~~~~~~~~", "~~~~~~~~~~"]);
        // Les lignes du bas du texte sont le sud: le pont est aux lignes y = 2 et 3 (la première ligne du texte est y = 5).
        assert_eq!(river.tile(4, 2), Tile::Bridge);
        assert_eq!(river.tile(4, 3), Tile::Bridge);
        assert_eq!(river.tile(4, 1), Tile::Water);
        // Visé au centre d'une tuile (le cas de l'éditeur), du côté nord, sud, ou sur le pont: dans tous les cas le coin est (x, 4).
        for cursor_y in [2.5f32, 3.5, 4.2, 1.9] {
            let corner = river.bridge_block_corner(Vec2::new(5.5, cursor_y));
            let covered = river.bridge_block(corner).iter().filter(|(x, y)| river.tile(*x, *y) == Tile::Bridge).count();
            assert_eq!(covered, 4, "le bloc couvre toute la largeur du pont: coin {corner:?} visé en y = {cursor_y}");
        }
        // Sans pont du tout, le coin de tuiles le plus proche est gardé.
        let dry = Room::from_rows(&["....", "....", "...."]);
        assert_eq!(dry.bridge_block_corner(Vec2::new(2.2, 1.9)), Vec2::new(2.0, 2.0));
    }

    /// Le chemin d'un héros contourne l'eau en passant par le pont; sans pont, aucun chemin ne traverse.
    #[test]
    fn a_path_crosses_the_water_only_by_the_bridge() {
        let river = ["..~~..", "..~~..", "..==..", "..~~..", "..~~.."];
        let room = Room::from_rows(&river);
        let (from, to) = (room.tile_center(0, 0), room.tile_center(5, 0));
        let path = room.find_path(from, to, &[]).expect("le pont permet de traverser");
        assert!(path.iter().all(|p| !room.overlaps_wall(*p, HERO_HALF_SIZE)));
        assert!(path.iter().any(|p| (p.y - room.tile_center(2, 2).y).abs() < TILE_SIZE), "le chemin passe par le pont");
        let dry = Room::from_rows(&["..~~..", "..~~..", "..~~..", "..~~..", "..~~.."]);
        assert!(dry.find_path(dry.tile_center(0, 0), dry.tile_center(5, 0), &[]).is_none() || !dry.segment_clear(dry.tile_center(0, 0), dry.tile_center(5, 0), HERO_HALF_SIZE));
    }

    #[test]
    fn heavier_monsters_hit_harder_and_move_slower() {
        let (goblin, orc) = (MonsterKind::Goblin.stats(), MonsterKind::Orc.stats());
        assert!(orc.max_hp > goblin.max_hp && orc.damage > goblin.damage && orc.speed < goblin.speed);
        assert!(orc.half_size > goblin.half_size);
    }
}

#[cfg(test)]
mod hero_tests {
    use super::*;

    #[test]
    fn heroes_can_take_several_hits_from_every_monster() {
        // Même le plus faible des héros encaisse plusieurs coups du plus dur des monstres.
        let strongest = [MonsterKind::Skeleton, MonsterKind::Bat, MonsterKind::Goblin, MonsterKind::Orc, MonsterKind::Ghoul]
            .into_iter()
            .map(|m| m.stats().damage)
            .fold(0.0, f32::max);
        for class in HeroClass::ALL {
            let hits = class.stats().max_hp as f32 / strongest;
            assert!(hits >= 5.0, "{class:?} ne survit qu'à {hits:.1} coups");
        }
    }
}
