//! Musique de fond: « Processional March » sur l'écran titre (menus et connexion), « Shadowed Corridors » en jeu.

use bevy::{audio::Volume, prelude::*};
use bevy_replicon::prelude::ClientState;

const TITLE_TRACK: &str = "audio/title.ogg";
const GAME_TRACK: &str = "audio/game.ogg";
const VOLUME: f32 = 0.5;

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, switch_track);
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
