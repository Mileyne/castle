//! Musique de fond: « Processional March » sur l'écran titre (menus et connexion), « Shadowed Corridors » en jeu.

use bevy::{audio::Volume, prelude::*};
use bevy_replicon::prelude::ClientState;

const TITLE_TRACK: &str = "audio/title.ogg";
const GAME_TRACK: &str = "audio/game.ogg";
const VOLUME: f32 = 0.5;

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (switch_track, duck_music));
    }
}

#[derive(Component)]
struct Music(&'static str);

/// Lance la piste qui correspond à l'état du client, et arrête l'autre.
fn switch_track(
    state: Res<State<ClientState>>,
    playing: Query<(Entity, &Music)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let wanted = if *state.get() == ClientState::Connected { GAME_TRACK } else { TITLE_TRACK };
    let mut already = false;
    for (entity, music) in &playing {
        if music.0 == wanted {
            already = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    if !already {
        commands.spawn((
            Music(wanted),
            AudioPlayer::new(assets.load::<AudioSource>(wanted)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(VOLUME)),
        ));
    }
}

/// De combien la musique baisse pendant qu'une voix parle (environ 6 dB), et la vitesse du fondu (part du volume par seconde).
const DUCKED: f32 = 0.5;
const DUCK_RATE: f32 = 2.5;

/// Baisse la musique en douceur tant qu'une voix parle, puis la remonte.
fn duck_music(
    time: Res<Time>,
    voices: Query<(), With<crate::dialogue::Voice>>,
    mut music: Query<&mut AudioSink, With<Music>>,
    mut level: Local<Option<f32>>,
) {
    let target = if voices.is_empty() { 1.0 } else { DUCKED };
    let current = level.unwrap_or(1.0);
    let next = if current < target { (current + DUCK_RATE * time.delta_secs()).min(target) } else { (current - DUCK_RATE * time.delta_secs()).max(target) };
    if next == current && level.is_some() {
        return;
    }
    *level = Some(next);
    for mut sink in &mut music {
        sink.set_volume(Volume::Linear(VOLUME * next));
    }
}
