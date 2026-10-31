//! Musique de fond: « Processional March » sur l'écran titre (menus et connexion); en jeu, « Shadowed Corridors » et
//! « Dungeon Exploration » en alternance d'un niveau à l'autre, et la piste des boss dès que le combat avec un boss s'engage.

use bevy::{audio::Volume, prelude::*};
use bevy_replicon::prelude::ClientState;
use castle_shared::{Expedition, Health, Monster, Player, Position};

const TITLE_TRACK: &str = "audio/title.ogg";
const GAME_TRACKS: [&str; 2] = ["audio/game.ogg", "audio/dungeon.ogg"];
const BOSS_TRACK: &str = "audio/boss.ogg";
/// Un boss est « en combat » quand un héros s'approche à cette distance (unités du monde) ou qu'il est déjà blessé.
const BOSS_ENGAGE: f32 = 300.0;
const VOLUME: f32 = 0.5;

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (switch_track, mix_music).chain());
    }
}

/// Une piste en cours de lecture, et sa part de volume (0 à 1): elle monte tant qu'elle est voulue, et descend sinon, jusqu'à s'éteindre.
#[derive(Component)]
struct Music {
    path: &'static str,
    level: f32,
    wanted: bool,
}

/// Durée du fondu enchaîné entre deux pistes, en secondes.
const CROSSFADE: f32 = 2.5;

/// Choisit la piste qui correspond à l'état du client; les autres s'éteignent peu à peu (fondu enchaîné).
fn switch_track(
    state: Res<State<ClientState>>,
    expeditions: Query<&Expedition>,
    bosses: Query<(&Monster, &Health, &Position)>,
    heroes: Query<&Position, With<Player>>,
    mut engaged: Local<bool>,
    mut playing: Query<&mut Music>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    // Une fois le combat engagé, la piste du boss dure jusqu'à sa mort (même si les héros s'éloignent).
    let boss = bosses.iter().filter(|(monster, ..)| monster.0.is_boss());
    *engaged = boss.clone().next().is_some()
        && (*engaged
            || boss.clone().any(|(_, health, at)| health.current < health.max || heroes.iter().any(|hero| hero.0.distance(at.0) <= BOSS_ENGAGE)));
    let level = expeditions.iter().last().map_or(1, |e| e.level);
    let wanted = if *state.get() != ClientState::Connected {
        TITLE_TRACK
    } else if *engaged {
        BOSS_TRACK
    } else {
        // Les niveaux alternent les deux pistes d'exploration.
        GAME_TRACKS[(level.max(1) as usize - 1) % GAME_TRACKS.len()]
    };
    let mut already = false;
    for mut music in &mut playing {
        music.wanted = music.path == wanted;
        already |= music.wanted;
    }
    if !already {
        commands.spawn((
            Music { path: wanted, level: 0.0, wanted: true },
            AudioPlayer::new(assets.load::<AudioSource>(wanted)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

/// De combien la musique baisse pendant qu'une voix parle (environ 6 dB), et la vitesse du fondu (part du volume par seconde).
const DUCKED: f32 = 0.5;
const DUCK_RATE: f32 = 2.5;

/// Règle le volume de chaque piste: fondu enchaîné, et baisse en douceur tant qu'une voix parle.
fn mix_music(
    mut commands: Commands,
    time: Res<Time>,
    voices: Query<(), With<crate::dialogue::Voice>>,
    mut music: Query<(Entity, &mut Music, Option<&mut AudioSink>)>,
    mut duck: Local<Option<f32>>,
) {
    let dt = time.delta_secs();
    let target = if voices.is_empty() { 1.0 } else { DUCKED };
    let current = duck.unwrap_or(1.0);
    let ducked = if current < target { (current + DUCK_RATE * dt).min(target) } else { (current - DUCK_RATE * dt).max(target) };
    *duck = Some(ducked);
    for (entity, mut track, sink) in &mut music {
        let step = dt / CROSSFADE;
        track.level = if track.wanted { (track.level + step).min(1.0) } else { (track.level - step).max(0.0) };
        if !track.wanted && track.level <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut sink) = sink {
            // Courbe en puissance: à mi-fondu, les deux pistes ont chacune près de 0,7 de volume, sans creux au milieu.
            let fade = track.level.sqrt();
            sink.set_volume(Volume::Linear(VOLUME * fade * ducked));
        }
    }
}
