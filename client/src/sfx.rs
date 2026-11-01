//! Bruitages: armes, impacts, cris des monstres, objets ramassés, prisonniers délivrés, mécanismes.
//!
//! Le serveur n'envoie aucun message pour ces sons: le client les déduit de ce qu'il voit changer (compteur de coups d'un
//! héros, points de vie qui baissent, objet ou monstre qui disparaît, mécanisme achevé...). Les fichiers sont dans
//! `assets/audio/sfx/`, produits par `tools/sfx/make_sfx.py`; un enregistrement du même nom les remplace.

use std::collections::HashMap;

use bevy::{audio::Volume, prelude::*};
use castle_shared::*;

/// Volume général des bruitages (la musique est à 0,5).
const VOLUME: f32 = 0.7;
/// Les sons plus loin que cela du centre de l'écran sont inaudibles; en deçà de `NEAR` ils sont à plein volume.
const NEAR: f32 = 280.0;
const FAR: f32 = 900.0;
/// Nombre maximal de bruitages joués en même temps.
const MAX_VOICES: usize = 24;
/// Délai minimal entre deux fois le même bruitage, pour qu'une mêlée ne devienne pas une bouillie.
const COOLDOWN: f32 = 0.07;
/// Au-delà de ce nombre de disparitions dans la même image, c'est un changement de niveau: on ne fait aucun bruit.
const MASS_REMOVAL: u32 = 12;

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SfxQueue>()
            .insert_resource(SfxRng(Rng::new(0x5F7)))
            .add_systems(Update, (swings, hurts, mechanisms, play).chain())
            .add_observer(count_removals)
            .add_observer(monster_died)
            .add_observer(generator_destroyed)
            .add_observer(item_picked)
            .add_observer(captive_freed)
            .add_observer(door_opened)
            .add_observer(portcullis_raised)
            .add_observer(hero_downed);
    }
}

#[derive(Resource)]
struct SfxRng(Rng);

/// Un bruitage à jouer: son nom, d'où il vient, son volume relatif et sa vitesse (hauteur).
struct Cue {
    name: &'static str,
    at: Option<Vec2>,
    gain: f32,
    speed: f32,
    /// Venu de la disparition d'une entité: supprimé quand tout le niveau disparaît.
    removal: bool,
}

#[derive(Resource, Default)]
struct SfxQueue {
    cues: Vec<Cue>,
    removals: u32,
}

impl SfxQueue {
    fn push(&mut self, name: &'static str, at: Option<Vec2>, gain: f32, speed: f32) {
        self.cues.push(Cue { name, at, gain, speed, removal: false });
    }

    fn push_removal(&mut self, name: &'static str, at: Option<Vec2>, gain: f32, speed: f32) {
        self.cues.push(Cue { name, at, gain, speed, removal: true });
    }
}

#[derive(Component)]
struct SfxVoice;

/// Le bruitage de la voix d'un monstre (famille de sons) et sa hauteur: les grosses bêtes sont plus graves.
fn voice_of(kind: MonsterKind) -> (&'static str, f32) {
    match kind {
        MonsterKind::Skeleton => ("skeleton", 1.0),
        MonsterKind::Bat => ("bat", 1.0),
        MonsterKind::Goblin => ("goblin", 1.0),
        MonsterKind::Orc => ("orc", 1.0),
        MonsterKind::Ghoul => ("ghoul", 1.0),
        MonsterKind::Wolf => ("wolf", 1.0),
        MonsterKind::Warg => ("wolf", 0.7),
        MonsterKind::Troll => ("orc", 0.72),
        MonsterKind::Muldant => ("orc", 0.6),
    }
}

fn count_removals(_: On<Remove<Position>>, mut queue: ResMut<SfxQueue>) {
    queue.removals += 1;
}

fn place(transforms: &Query<&Transform>, entity: Entity) -> Option<Vec2> {
    transforms.get(entity).ok().map(|t| t.translation.truncate())
}

fn monster_died(removed: On<Remove<Monster>>, monsters: Query<&Monster>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    if let Ok(monster) = monsters.get(removed.entity) {
        let (family, speed) = voice_of(monster.0);
        let name = match family {
            "skeleton" => "skeleton_die",
            "bat" => "bat_die",
            "goblin" => "goblin_die",
            "orc" => "orc_die",
            "ghoul" => "ghoul_die",
            _ => "wolf_die",
        };
        queue.push_removal(name, place(&transforms, removed.entity), if monster.0.is_boss() { 1.4 } else { 1.0 }, speed);
    }
}

fn generator_destroyed(removed: On<Remove<Generator>>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    queue.push_removal("crash", place(&transforms, removed.entity), 1.0, 1.0);
}

fn item_picked(removed: On<Remove<Pickup>>, pickups: Query<&Pickup>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    if let Ok(pickup) = pickups.get(removed.entity) {
        let name = match pickup.0 {
            PickupKind::Coin => "pickup_coin",
            PickupKind::Food => "pickup_food",
            PickupKind::Potion => "pickup_potion",
            PickupKind::Key => "pickup_key",
            PickupKind::Ingredient | PickupKind::GearPart | PickupKind::TargetDisc => "pickup_item",
        };
        queue.push_removal(name, place(&transforms, removed.entity), 0.9, 1.0);
    }
}

fn captive_freed(removed: On<Remove<Captive>>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    queue.push_removal("freed", place(&transforms, removed.entity), 1.0, 1.0);
}

fn door_opened(removed: On<Remove<Door>>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    queue.push_removal("door", place(&transforms, removed.entity), 1.0, 1.0);
}

fn portcullis_raised(removed: On<Remove<Portcullis>>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    queue.push_removal("portcullis", place(&transforms, removed.entity), 1.0, 1.0);
}

fn hero_downed(added: On<Add<Downed>>, transforms: Query<&Transform>, mut queue: ResMut<SfxQueue>) {
    queue.push("downed", place(&transforms, added.entity), 1.0, 1.0);
}

/// Le coup d'un héros (attaque ou pouvoir): le compteur de coups change, le son dépend de la classe.
fn swings(heroes: Query<(Ref<Strikes>, &HeroClass, &Transform)>, mut queue: ResMut<SfxQueue>, mut rng: ResMut<SfxRng>) {
    for (strikes, class, transform) in &heroes {
        if !strikes.is_changed() || strikes.is_added() {
            continue;
        }
        let name = match class {
            HeroClass::Knight => "swing_knight",
            HeroClass::Sorcerer => "swing_spell",
            HeroClass::Ranger => "swing_arrow",
            HeroClass::Forgebraise => "swing_hammer",
        };
        queue.push(name, Some(transform.translation.truncate()), 0.9, 0.94 + rng.0.next_f32() * 0.12);
    }
}

/// Points de vie vus à l'image précédente.
#[derive(Component)]
struct SeenHealth(f32);

/// Une baisse de points de vie: impact, et cri de la victime.
fn hurts(
    mut commands: Commands,
    mut living: Query<(Entity, &Health, &Transform, &mut SeenHealth, Option<&Monster>, Option<&Generator>, Has<Player>)>,
    fresh: Query<(Entity, &Health), Without<SeenHealth>>,
    mut queue: ResMut<SfxQueue>,
    mut rng: ResMut<SfxRng>,
) {
    for (entity, health) in &fresh {
        commands.entity(entity).insert(SeenHealth(health.current));
    }
    for (_, health, transform, mut seen, monster, generator, player) in &mut living {
        let lost = seen.0 - health.current;
        seen.0 = health.current;
        if lost < 0.5 {
            continue;
        }
        let at = Some(transform.translation.truncate());
        let pitch = 0.92 + rng.0.next_f32() * 0.16;
        if generator.is_some() {
            queue.push("hit_stone", at, 0.9, pitch);
        } else if let Some(monster) = monster {
            let (family, speed) = voice_of(monster.0);
            queue.push(if monster.0 == MonsterKind::Skeleton { "hit_bone" } else { "hit_flesh" }, at, 0.9, pitch);
            if health.current > 0.0 {
                let hurt = match family {
                    "skeleton" => "skeleton_hurt",
                    "bat" => "bat_hurt",
                    "goblin" => "goblin_hurt",
                    "orc" => "orc_hurt",
                    "ghoul" => "ghoul_hurt",
                    _ => "wolf_hurt",
                };
                queue.push(hurt, at, 0.8, speed * pitch);
            }
        } else if player {
            queue.push("hit_flesh", at, 0.8, pitch);
            queue.push("hurt_hero", at, 0.9, pitch);
        }
    }
}

/// Un mécanisme qui s'achève, la sortie qui s'ouvre.
fn mechanisms(
    interactables: Query<(Entity, &Interactable, &Transform), Changed<Interactable>>,
    exits: Query<(Entity, &LevelExit, &Transform), Changed<LevelExit>>,
    mut seen: Local<HashMap<Entity, bool>>,
    mut queue: ResMut<SfxQueue>,
) {
    if seen.len() > 512 {
        seen.clear();
    }
    for (entity, interactable, transform) in &interactables {
        if seen.insert(entity, interactable.done) == Some(false) && interactable.done {
            queue.push("puzzle", Some(transform.translation.truncate()), 1.0, 1.0);
        }
    }
    for (entity, exit, transform) in &exits {
        if seen.insert(entity, exit.open) == Some(false) && exit.open {
            queue.push("exit_open", Some(transform.translation.truncate()), 0.9, 1.0);
        }
    }
}

/// Joue les bruitages demandés pendant l'image: volume selon la distance au centre de l'écran, hauteur légèrement variable.
fn play(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    camera: Query<&Transform, With<Camera2d>>,
    voices: Query<(), With<SfxVoice>>,
    mut queue: ResMut<SfxQueue>,
    mut last: Local<HashMap<&'static str, f32>>,
) {
    let mass = queue.removals > MASS_REMOVAL;
    queue.removals = 0;
    let cues = std::mem::take(&mut queue.cues);
    let now = time.elapsed_secs();
    let listener = camera.iter().next().map(|t| t.translation.truncate());
    let mut active = voices.iter().count();
    for cue in cues {
        if (mass && cue.removal) || active >= MAX_VOICES {
            continue;
        }
        if last.get(cue.name).is_some_and(|at| now - at < COOLDOWN) {
            continue;
        }
        let near = match (listener, cue.at) {
            (Some(listener), Some(at)) => 1.0 - ((listener.distance(at) - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0),
            _ => 1.0,
        };
        if near <= 0.0 {
            continue;
        }
        last.insert(cue.name, now);
        active += 1;
        commands.spawn((
            SfxVoice,
            AudioPlayer::new(assets.load::<AudioSource>(format!("audio/sfx/{}.ogg", cue.name))),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOLUME * crate::settings::volume(crate::settings::Channel::Sfx) * cue.gain * near)).with_speed(cue.speed),
        ));
    }
}
