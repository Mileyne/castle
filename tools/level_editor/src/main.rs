//! L'éditeur de niveaux: le jeu lui-même (même rendu, mêmes lumières), avec des panneaux egui par-dessus et des outils qui visent le
//! sol de la scène. Le niveau modifié remplace celui du jeu à chaud (`castle_shared::override_level`, `sim::ReloadLevel`).
//!
//! `./editeur.sh [--niveau=N]` (ou `cargo run --release -p level_editor -- --niveau=N`)

mod items;
mod overlay;
mod panels;
mod state;
mod tools;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use castle_shared::{Room, SetPause};

use castle_client::editor_view::EditorCamera;
use state::{Editing, Tools};

fn main() {
    let first: u32 = std::env::args().find_map(|a| a.strip_prefix("--niveau=")?.parse().ok()).unwrap_or(1);
    // Le jeu démarre tout seul sur ce niveau, dans une sauvegarde jetable.
    let saves = std::env::temp_dir().join(format!("castle_editor_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&saves);
    // SAFETY: rien d'autre ne tourne encore, l'application n'est pas construite.
    unsafe {
        std::env::set_var("CASTLE_SAVE_DIR", &saves);
        std::env::set_var("CASTLE_AUTOSTART", "editeur");
        std::env::set_var("CASTLE_START_LEVEL", first.to_string());
    }
    castle_client::editor_view::set_editor();
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/levels"));
    let mut app = castle_client::build_app();
    app.add_plugins(EguiPlugin::default())
        .insert_resource(EguiGlobalSettings { auto_create_primary_context: false, ..default() })
        .insert_resource(Editing::new(dir, first))
        .init_resource::<Tools>()
        .init_resource::<state::PanelRects>()
        .init_resource::<EditorCamera>()
        .add_systems(Startup, (setup_window, overlay::setup_gizmos))
        .add_systems(
            Update,
            (attach_egui, freeze, hide_interface, tools::camera_controls, tools::pointer_tools, tools::sync_scene, overlay::draw_overlay).chain(),
        )
        .add_systems(EguiPrimaryContextPass, panels::panels);
    app.run();
}

fn setup_window(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.title = "Castle — Éditeur de niveaux".into();
    window.resolution.set(1360.0, 840.0);
    window.position = bevy::window::WindowPosition::Centered(MonitorSelection::Primary);
}

/// Les panneaux se dessinent avec la caméra des sprites, celle qui passe en dernier.
fn attach_egui(mut commands: Commands, cameras: Query<Entity, (With<Camera2d>, Without<PrimaryEguiContext>)>) {
    for camera in &cameras {
        commands.entity(camera).insert(PrimaryEguiContext);
    }
}

/// Le monde reste figé: on regarde le niveau, on n'y joue pas.
fn freeze(mut commands: Commands, room: Option<Res<Room>>, mut done: Local<bool>) {
    if room.is_some() && !*done {
        *done = true;
        commands.trigger(SetPause(true));
    }
}

/// L'interface du jeu (barres, bandeaux, dialogues) ne figure pas dans l'éditeur.
fn hide_interface(mut roots: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>) {
    for mut visibility in &mut roots {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}
