//! Types partagés entre client et serveur de Castle.

use bevy::{ecs::entity::MapEntities, prelude::*};

mod dungeon;
pub use dungeon::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_ID: u64 = 1;
pub const DEFAULT_PORT: u16 = 5000;
pub const TILE_SIZE: f32 = 16.0;
pub const MAX_PLAYERS: usize = 4;
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

/// Distance à laquelle un héros qui détecte les secrets révèle leviers et pièges.
pub const DETECT_RADIUS: f32 = 90.0;
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

    /// Passif de l'Ensorceleur et du Rôdeur: voir les secrets et les pièges magiques.
    pub fn detects_secrets(self) -> bool {
        matches!(self, Self::Sorcerer | Self::Ranger)
    }
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
    pub fn stats(self) -> ClassStats {
        match self {
            Self::Knight => ClassStats { name_key: "class-knight", max_hp: 150, speed: 70.0, special_key: "special-knight", max_mana: 60.0, mana_regen: 3.0, special_cost: 30.0, special_cooldown: 12.0, attack: Attack::Melee { range: 22.0, damage: 12.0, cooldown: 0.45 } },
            Self::Sorcerer => ClassStats { name_key: "class-sorcerer", max_hp: 80, speed: 80.0, special_key: "special-sorcerer", max_mana: 100.0, mana_regen: 6.0, special_cost: 40.0, special_cooldown: 8.0, attack: Attack::Shot { speed: 180.0, damage: 10.0, cooldown: 0.5, lifetime: 1.2 } },
            Self::Ranger => ClassStats { name_key: "class-ranger", max_hp: 100, speed: 95.0, special_key: "special-ranger", max_mana: 70.0, mana_regen: 4.0, special_cost: 35.0, special_cooldown: 10.0, attack: Attack::Shot { speed: 260.0, damage: 8.0, cooldown: 0.35, lifetime: 1.0 } },
            Self::Forgebraise => ClassStats { name_key: "class-forgebraise", max_hp: 110, speed: 75.0, special_key: "special-forgebraise", max_mana: 80.0, mana_regen: 4.0, special_cost: 35.0, special_cooldown: 14.0, attack: Attack::Melee { range: 20.0, damage: 16.0, cooldown: 0.7 } },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
}

/// Carte d'une salle en tuiles. Le serveur et les clients construisent la même, sans la répliquer.
/// L'origine du monde est au centre de la salle, l'axe y vers le haut.
#[derive(Resource, Clone, Debug)]
pub struct Room {
    size: UVec2,
    tiles: Vec<Tile>,
}

impl Room {
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

    /// Une boîte centrée en `center`, de demi-côté `half`, touche-t-elle un mur ?
    pub fn overlaps_wall(&self, center: Vec2, half: f32) -> bool {
        let to_grid = |v: Vec2| ((v / TILE_SIZE) + self.size.as_vec2() / 2.0).floor().as_ivec2();
        // On retire un epsilon: coller exactement un mur n'est pas le traverser.
        let min = to_grid(center - Vec2::splat(half));
        let max = to_grid(center + Vec2::splat(half - 0.001));
        (min.y..=max.y).any(|y| (min.x..=max.x).any(|x| self.tile(x, y) == Tile::Wall))
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
        self.tile(t.x, t.y) == Tile::Floor
    }

    /// Une boîte de demi-côté `half` peut-elle aller en ligne droite de `from` à `to` sans toucher un mur ?
    pub fn segment_clear(&self, from: Vec2, to: Vec2, half: f32) -> bool {
        let d = to - from;
        let steps = (d.length() / 3.0).ceil().max(1.0) as u32;
        (0..=steps).all(|i| !self.overlaps_wall(from + d * (i as f32 / steps as f32), half))
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
            while j > i && !self.segment_clear(origin, points[j], HERO_HALF_SIZE) {
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
}

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
        match self {
            Self::Skeleton => MonsterStats { max_hp: 30.0, speed: 40.0, damage: 8.0, cooldown: 0.8, half_size: 5.0, sight: 110.0 },
            Self::Bat => MonsterStats { max_hp: 12.0, speed: 75.0, damage: 4.0, cooldown: 0.6, half_size: 3.5, sight: 150.0 },
        }
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
}

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
}

impl QuestKind {
    /// Clé de traduction de l'intitulé.
    pub fn name_key(self) -> &'static str {
        match self {
            Self::Hunt => "quest-hunt",
            Self::Retrieve => "quest-retrieve",
            Self::Rescue => "quest-rescue",
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

/// Zone d'effet au sol (Tempête Arcane, Pluie de Flèches), lancée par un héros de cette classe.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Zone {
    pub radius: f32,
    pub class: HeroClass,
}

/// Tourelle automatique du Forgebraise.
#[derive(Component, Serialize, Deserialize)]
pub struct Turret;

/// Étapes de l'énigme de la porte royale: chacune demande une classe précise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractKind {
    /// Levier caché, actionné par le Rôdeur.
    Lever,
    /// Rune à dissiper, par l'Ensorceleur.
    Rune,
    /// Mécanisme à réparer, par le Forgebraise.
    Mechanism,
    /// La porte elle-même, que le Chevalier pousse une fois le reste fait.
    Push,
}

impl InteractKind {
    pub fn class(self) -> HeroClass {
        match self {
            Self::Lever => HeroClass::Ranger,
            Self::Rune => HeroClass::Sorcerer,
            Self::Mechanism => HeroClass::Forgebraise,
            Self::Push => HeroClass::Knight,
        }
    }

    /// Temps pendant lequel il faut maintenir la touche d'action, en secondes.
    pub fn hold_time(self) -> f32 {
        match self {
            Self::Lever => 0.8,
            Self::Rune => 1.5,
            Self::Mechanism => 2.5,
            Self::Push => 2.0,
        }
    }

    /// Les secrets ne se voient qu'à proximité d'un héros qui les détecte.
    pub fn is_secret(self) -> bool {
        matches!(self, Self::Lever)
    }
}

/// Objet sur lequel on agit en maintenant la touche d'action.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Interactable {
    pub kind: InteractKind,
    /// Avancement de l'action en cours, de 0 à 1.
    pub progress: f32,
    pub done: bool,
}

impl Interactable {
    pub fn new(kind: InteractKind) -> Self {
        Self { kind, progress: 0.0, done: false }
    }
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

/// Portail de sortie de l'expédition.
#[derive(Component, Serialize, Deserialize)]
pub struct Portal;

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

/// Le joueur choisit sur l'écran titre de repartir de zéro: le serveur génère un nouveau château.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NewGame;

/// Le joueur prend le contrôle du héros de cette classe dans son équipe.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SwitchHero(pub HeroClass);

/// Équipe à laquelle appartient un héros: chaque joueur connecté dirige une équipe de quatre héros.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyId(pub u32);

/// Envoyé par le serveur à un client pour lui dire quel héros il dirige en ce moment, et son équipe.
#[derive(Event, Clone, Copy, Debug, Serialize, Deserialize, MapEntities)]
pub struct YouAre {
    #[entities]
    pub hero: Entity,
    pub party: u32,
}

/// Enregistre ce qui est répliqué et les événements réseau.
/// Doit être ajouté à l'identique côté client et côté serveur.
pub struct CastleSharedPlugin;

impl Plugin for CastleSharedPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<Expedition>()
            .replicate::<Player>()
            .replicate::<PartyId>()
            .replicate::<HeroClass>()
            .replicate::<Position>()
            .replicate::<Health>()
            .replicate::<Interactable>()
            .replicate::<Door>()
            .replicate::<Portal>()
            .replicate::<Trap>()
            .replicate::<Mana>()
            .replicate::<Shielded>()
            .replicate::<Zone>()
            .replicate::<Turret>()
            .replicate::<Monster>()
            .replicate::<Generator>()
            .replicate::<Pickup>()
            .replicate::<Quest>()
            .replicate::<Relic>()
            .replicate::<Captive>()
            .replicate::<Gold>()
            .replicate::<Keys>()
            .replicate::<Projectile>()
            .replicate::<Facing>()
            .replicate::<Strikes>()
            .replicate::<Downed>()
            .add_server_event::<AttackFx>(Channel::Unordered)
            .add_server_event::<ScreenShake>(Channel::Unordered)
            .add_mapped_server_event::<YouAre>(Channel::Ordered)
            .add_server_event::<DialogueBegin>(Channel::Ordered)
            .add_server_event::<DialogueAdvance>(Channel::Ordered)
            .add_server_event::<DialogueSelect>(Channel::Ordered)
            .add_client_event::<DialogueRequestAdvance>(Channel::Ordered)
            .add_client_event::<DialogueVote>(Channel::Ordered)
            .add_client_event::<DialogueFinished>(Channel::Ordered)
            .add_client_event::<PlayerInput>(Channel::Unordered)
            .add_client_event::<SwitchHero>(Channel::Ordered)
            .add_client_event::<NewGame>(Channel::Ordered);
    }
}

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
