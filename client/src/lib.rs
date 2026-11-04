//! Client de Castle: fenêtre, rendu pixel-art et saisie.
//!
//! Bibliothèque lancée par `run`: le programme `castle_client` (ordinateur) et l'app iOS (`mobile/`) l'appellent tous deux.

use std::time::Duration;

use bevy::{color::palettes::css, math::StableInterpolate, prelude::*};
use castle_shared::*;

use art::{Art, Prop, SPRITE_SIZE, WalkCycle, object, projectile};
use live3d::ModelTint;

pub use decor3d::{DecorCamera, LAYER as DECOR_LAYER};

mod art;
mod boss;
mod camera;
mod controls;
mod dialogue;
mod dialogue_script;
#[cfg(target_os = "ios")]
mod gamecontroller;
#[cfg(target_os = "ios")]
mod prefs;
mod firstperson;
mod fx;
mod i18n;
mod lighting;
mod decor3d;
pub mod editor_view;
mod live3d;
mod merged;
mod prebake;
mod pause;
mod puzzle;
mod settings;
mod sky;
pub mod sim;
mod sprite_meta;
mod sprite_meta_lite;
mod level_intro;
mod music;
mod sfx;
mod quest;
mod title;
mod touch;
mod weather;
mod ui;

/// Réglages de la salve du Rôdeur lus par les effets (nombre de flèches, angle de l'éventail en radians).
pub(crate) fn balance_arrows() -> usize {
    balance().abilities.volley_arrows.round().max(1.0) as usize
}

pub(crate) fn balance_spread() -> f32 {
    balance().abilities.volley_spread.to_radians()
}

/// Vitesse de rattrapage visuel vers la position du serveur.
const SMOOTHING: f32 = 20.0;
/// Hauteur des bras d'un héros au-dessus de sa position au sol, en unités du monde: les projectiles en partent.
const ARM_HEIGHT: f32 = 6.0;
/// Tant qu'un tir vers le haut de l'écran est à moins de cette distance de son départ, il passe derrière le héros (de
/// dos, on ne le voit pas sortir de son corps).
const BODY_CLEAR: f32 = 16.0;

/// Décalage vertical d'affichage, en unités du monde (la position réelle, au sol, ne change pas).
#[derive(Component)]
pub struct Lift(f32);

/// Point de départ d'un projectile.
#[derive(Component)]
struct Launch(Vec2);

/// Écrit dans le journal toute exception Objective-C au moment où elle est levée (Metal en lève quand il refuse une opération):
/// sinon Rust ne voit qu'une « exception étrangère » et abandonne sans dire laquelle.
#[cfg(target_os = "ios")]
fn log_objc_exceptions() {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe extern "C" {
        fn objc_setExceptionPreprocessor(
            preprocessor: extern "C" fn(*mut AnyObject) -> *mut AnyObject,
        ) -> *mut std::ffi::c_void;
    }
    extern "C" fn preprocess(exception: *mut AnyObject) -> *mut AnyObject {
        // SAFETY: `exception` est l'objet que le runtime Objective-C vient de lever; on ne fait que lire son nom et sa raison.
        unsafe {
            let text = |object: *mut AnyObject| -> String {
                if object.is_null() {
                    return String::new();
                }
                let chars: *const std::ffi::c_char = msg_send![object, UTF8String];
                if chars.is_null() { String::new() } else { std::ffi::CStr::from_ptr(chars).to_string_lossy().into_owned() }
            };
            let name: *mut AnyObject = msg_send![exception, name];
            let reason: *mut AnyObject = msg_send![exception, reason];
            eprintln!("EXCEPTION OBJC: {} - {}", text(name), text(reason));
            // La pile au moment où l'exception est levée.
            let symbols: *mut AnyObject = msg_send![objc2::class!(NSThread), callStackSymbols];
            if !symbols.is_null() {
                let count: usize = msg_send![symbols, count];
                for i in 0..count.min(40) {
                    let line: *mut AnyObject = msg_send![symbols, objectAtIndex: i];
                    eprintln!("   {}", text(line));
                }
            }
        }
        exception
    }
    // SAFETY: `preprocess` a la signature attendue par le runtime Objective-C.
    unsafe { objc_setExceptionPreprocessor(preprocess) };
}

/// Durée de la mise à jour du monde (de `First` à `Last`) sur l'image en cours, cumulée sur la période du rapport.
#[derive(Resource, Default)]
struct UpdateTimer {
    started: Option<std::time::Instant>,
    total: Duration,
    frames: u32,
}

fn update_begins(mut timer: ResMut<UpdateTimer>) {
    timer.started = Some(std::time::Instant::now());
}

fn update_ends(mut timer: ResMut<UpdateTimer>) {
    if let Some(started) = timer.started.take() {
        timer.total += started.elapsed();
        timer.frames += 1;
    }
}

/// Toutes les cinq secondes, écrit dans le journal la cadence d'affichage, le temps de mise à jour du monde (CPU, fil principal),
/// le nombre d'entités et d'objets visibles, et les temps des passes de rendu quand l'appareil les mesure (pour juger la fluidité
/// sur les petits appareils).
fn log_frame_rate(
    time: Res<Time>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    entities: Query<Entity>,
    visible: Query<&ViewVisibility>,
    windows: Query<&Window>,
    mut timer: ResMut<UpdateTimer>,
    mut next: Local<f32>,
) {
    use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 5.0;
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let frame = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let update = if timer.frames > 0 { timer.total.as_secs_f64() * 1000.0 / timer.frames as f64 } else { 0.0 };
    *timer = UpdateTimer::default();
    let window = windows.iter().next().map(|w| format!("{}x{} (échelle {})", w.physical_width(), w.physical_height(), w.scale_factor())).unwrap_or_default();
    info!(
        "cadence: {fps:.0} images/s ({frame:.1} ms), mise à jour {update:.1} ms, fenêtre {window}, {} entités, {} visibles",
        entities.iter().count(),
        visible.iter().filter(|v| v.get()).count()
    );
    // Passes de rendu (CPU et GPU), si l'appareil sait les chronométrer.
    let mut passes: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.path().as_str().starts_with("render/"))
        .filter_map(|d| d.smoothed().map(|v| format!("{} {v:.2}{}", d.path(), d.suffix)))
        .collect();
    passes.sort();
    if !passes.is_empty() {
        info!("rendu: {}", passes.join(" | "));
    }
}

/// Journal du jeu. Sur iOS et tvOS, le journal système n'est pas lisible depuis le Mac sans outil tiers: on l'écrit aussi dans
/// `tmp/castle.log` du conteneur de l'app (à récupérer avec `devicectl device copy from`), avec les paniques.
fn log_plugin() -> bevy::log::LogPlugin {
    #[cfg(target_os = "ios")]
    {
        fn file_layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
            use bevy::log::tracing_subscriber::{self, Layer};
            let path = std::env::temp_dir().join("castle.log");
            let file = std::fs::File::create(&path).ok()?;
            // Les messages d'arrêt de Rust (exceptions étrangères, abandon) vont à la sortie d'erreur: on la redirige aussi.
            {
                use std::os::fd::AsRawFd;
                unsafe extern "C" {
                    fn dup2(from: i32, to: i32) -> i32;
                }
                // SAFETY: `file` est un descripteur ouvert; 2 est la sortie d'erreur du processus.
                unsafe { dup2(file.as_raw_fd(), 2) };
            }
            let panics = path.clone();
            std::panic::set_hook(Box::new(move |info| {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&panics) {
                    let _ = writeln!(file, "PANIC: {info}");
                }
            }));
            Some(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(file)).boxed())
        }
        return bevy::log::LogPlugin { custom_layer: file_layer, ..default() };
    }
    #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
    bevy::log::LogPlugin::default()
}

/// Lance le jeu (ne revient qu'à la fermeture).
/// Aperçu du décor 3D sans lancer le jeu (`cargo run -p castle_client --bin decor_preview`).
#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub fn decor_preview() {
    decor3d::preview::run();
}

/// Le programme de réglage de la lumière (`./lumiere.sh`).
#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub fn light_tuner() {
    decor3d::tuner::run();
}

pub fn run() {
    build_app().run();
}

/// L'application du jeu, prête à tourner (l'éditeur de niveaux y ajoute ses panneaux).
pub fn build_app() -> App {
    #[cfg(target_os = "ios")]
    {
        log_objc_exceptions();
        prefs::use_on_tv();
    }
    // Assets du dépôt si on lance depuis une compilation du dépôt, sinon à côté du programme.
    // Un dossier `assets` à côté du programme (version livrée) passe avant celui du dépôt.
    // Dans un paquet macOS (`Castle.app/Contents/MacOS/castle_client`), ils sont dans `Contents/Resources`.
    let beside_exe = std::env::current_exe().ok().and_then(|exe| {
        let dir = exe.parent()?;
        let bundled = dir.parent()?.join("Resources/assets");
        Some(if bundled.exists() { bundled } else { dir.join("assets") })
    });
    let dev_assets = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    let assets_path = match beside_exe {
        Some(dir) if dir.exists() => dir.to_string_lossy().into_owned(),
        _ if std::path::Path::new(dev_assets).exists() => dev_assets.to_string(),
        _ => "assets".to_string(),
    };
    let mut app = App::new();
    app
        .add_plugins((
            // `nearest` garde les pixels nets.
            DefaultPlugins
                .set(log_plugin())
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: assets_path.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(window()),
                    ..default()
                }),
            sim::SimPlugin,
            i18n::I18nPlugin,
            ui::UiPlugin,
            title::TitlePlugin,
            pause::PausePlugin,
            quest::QuestPlugin,
            level_intro::LevelIntroPlugin,
            art::ArtPlugin,
            camera::CameraPlugin,
            firstperson::FirstPersonPlugin,
            sky::SkyPlugin,
            lighting::LightingPlugin,
            dialogue::DialoguePlugin,
        ))
        .add_plugins((
            settings::SettingsPlugin,
            controls::ControlsPlugin,
            puzzle::PuzzlePlugin,
            boss::BossPlugin,
            live3d::Live3dPlugin,
            decor3d::Decor3dPlugin,
            fx::FxPlugin,
            prebake::PrebakePlugin,
            touch::TouchPlugin,
            music::MusicPlugin,
            sfx::SfxPlugin,
            weather::WeatherPlugin,
        ))
        .add_plugins({
            #[cfg(target_os = "ios")]
            {
                gamecontroller::GameControllerPlugin
            }
            #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
            {
                ()
            }
        })
        .insert_resource(ClearColor(Color::BLACK))
        // Fenêtre au premier plan: rendu continu. En arrière-plan: le jeu se met presque au repos.
        .insert_resource(bevy::winit::WinitSettings::game())
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .init_resource::<UpdateTimer>()
        .add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin)
        .add_systems(First, update_begins)
        .add_systems(Last, update_ends)
        .add_systems(Update, log_frame_rate)
        .add_observer(mark_local_player)
        .add_systems(
            Update,
            (
                send_input.run_if(sim::in_game).run_if(level_intro::player_can_act),
                layer_projectiles.after(smooth_positions),
                update_exit,
                (dress_new_entities, fade_downed).chain(),
                smooth_positions,
                draw_health_bars,
                draw_effects,
                update_props,
                animate_projectiles,
                (projectile_trails, fade_particles).chain(),
            ),
        );
    app
}

/// Fenêtre du jeu sur iOS: tout l'écran, sans barre d'état ni indicateur d'accueil; le geste de retour à l'accueil
/// demande deux glissés, pour ne pas quitter en pleine partie.
#[cfg(target_os = "ios")]
fn window() -> Window {
    use bevy::window::{ScreenEdge, WindowMode};
    Window {
        title: "Castle".into(),
        resizable: false,
        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
        prefers_home_indicator_hidden: true,
        prefers_status_bar_hidden: true,
        preferred_screen_edges_deferring_system_gestures: ScreenEdge::Bottom,
        ..default()
    }
}

/// Fenêtre du jeu sur ordinateur.
#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
fn window() -> Window {
    Window {
        title: "Castle".into(),
        // `CASTLE_NOVSYNC=1`: pas de synchronisation verticale, pour mesurer ce que le moteur peut vraiment faire.
        present_mode: if std::env::var_os("CASTLE_NOVSYNC").is_some() { bevy::window::PresentMode::AutoNoVsync } else { default() },
        ..default()
    }
}

/// Le héros du joueur de ce client.
#[derive(Component)]
pub struct LocalPlayer;

/// Équipe du joueur de ce client.
#[derive(Resource)]
pub struct LocalParty(pub u32);

/// Le serveur indique quel héros on dirige (au départ, ou après un changement de héros).
fn mark_local_player(you: On<YouAre>, mut commands: Commands, previous: Query<Entity, With<LocalPlayer>>) {
    for entity in &previous {
        commands.entity(entity).remove::<LocalPlayer>();
    }
    commands.entity(you.hero).insert(LocalPlayer);
    commands.insert_resource(LocalParty(you.party));
}

/// Le tir maintenu: depuis combien de temps, et si le pouvoir spécial a déjà été lancé pendant cette pression.
#[derive(Default)]
struct FireHold {
    held: f32,
    special_sent: bool,
}

/// La direction demandée à l'écran (haut = vers le haut de l'écran) en direction du monde, quand la caméra est tournée de `yaw`
/// autour du décor (0: elle est au sud et regarde vers le nord). Avec un angle nul, rien ne change.
fn screen_to_world(direction: Vec2, yaw: f32) -> Vec2 {
    Vec2::from_angle(yaw).rotate(direction)
}

/// Envoie direction, tir, pouvoir et action quand ils changent, et le héros choisi.
///
/// Les commandes sont les mêmes partout (voir `controls.rs`): Espace / B tire, et maintenu assez longtemps lance le pouvoir
/// spécial à la place (à la manette, A le lance directement); Maj / X agit; Ctrl / Y passe au héros suivant. La manette et les boutons à l'écran « appuient » sur ces
/// mêmes touches (`touch.rs`). 1 à 4 restent un raccourci discret au clavier pour choisir directement un héros.
fn send_input(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut last: Local<(Vec2, bool, bool, bool)>,
    mut fire: Local<FireHold>,
    dialogue: Res<dialogue::DialogueState>,
    pause: Res<pause::PauseMenu>,
    stick: Res<touch::Stick>,
    party: Option<Res<LocalParty>>,
    active: Option<Single<&HeroClass, With<LocalPlayer>>>,
    heroes: Query<(&HeroClass, &PartyId, Has<Downed>), With<Player>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    // Menu de pause ouvert: on lâche tout, et on ne change pas de héros.
    let held = |key: KeyCode| !pause.open && keys.pressed(key);
    // Sans touche de direction, le joystick tactile ou la manette: on garde sa direction, arrondie à 1/32 de tour
    // pour ne pas envoyer un message au serveur à chaque frémissement du pouce.
    if dir == Vec2::ZERO && stick.0 != Vec2::ZERO {
        let step = std::f32::consts::TAU / 32.0;
        dir = Vec2::from_angle((stick.0.to_angle() / step).round() * step);
    }
    let dir = if pause.open { Vec2::ZERO } else { screen_to_world(dir.normalize_or_zero(), settings::view_yaw()) };
    // Le tir: une pression brève frappe; maintenue, elle lance le pouvoir (une fois) à la place de frapper.
    let fire_down = held(KeyCode::Space);
    if fire_down {
        fire.held += time.delta_secs();
    } else {
        *fire = FireHold::default();
    }
    let long_press = fire_down && fire.held >= controls::SPECIAL_HOLD;
    let attack = fire_down && !long_press;
    let special_pulse = long_press && !std::mem::replace(&mut fire.special_sent, true);
    // E (la manette: A) lance aussi le pouvoir, mais pas pendant un dialogue (A sert alors à continuer).
    let special = special_pulse || (held(KeyCode::KeyE) && !dialogue.showing());
    let interact = held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight);
    if (dir, attack, special, interact) != *last {
        *last = (dir, attack, special, interact);
        commands.trigger(PlayerInput {
            direction: dir,
            attack,
            special,
            interact,
        });
    }

    // Pendant un vote de dialogue, 1 à 4 servent à voter et non à changer de héros.
    if dialogue.is_voting() || pause.open {
        return;
    }
    // Ctrl (Y à la manette): le héros vivant suivant de l'équipe.
    if keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::ControlRight) {
        if let (Some(party), Some(active)) = (&party, &active) {
            let order = HeroClass::ALL;
            let at = order.iter().position(|class| class == **active).unwrap_or(0);
            let next = (1..order.len())
                .map(|step| order[(at + step) % order.len()])
                .find(|class| heroes.iter().any(|(c, id, downed)| c == class && id.0 == party.0 && !downed));
            if let Some(class) = next {
                commands.trigger(SwitchHero(class));
            }
        }
    }
    for (key, class) in [
        (KeyCode::Digit1, HeroClass::Knight),
        (KeyCode::Digit2, HeroClass::Sorcerer),
        (KeyCode::Digit3, HeroClass::Ranger),
        (KeyCode::Digit4, HeroClass::Forgebraise),
    ] {
        if keys.just_pressed(key) {
            commands.trigger(SwitchHero(class));
        }
    }
}

fn class_color(class: HeroClass) -> Color {
    match class {
        HeroClass::Knight => css::STEEL_BLUE,
        HeroClass::Sorcerer => css::MEDIUM_ORCHID,
        HeroClass::Ranger => css::FOREST_GREEN,
        HeroClass::Forgebraise => css::ORANGE_RED,
    }
    .into()
}

/// Donne un sprite aux entités qui viennent d'être répliquées.
fn dress_new_entities(
    mut commands: Commands,
    art: Res<Art>,
    players: Query<(Entity, &Position, Option<&HeroClass>), Added<Player>>,
    monsters: Query<(Entity, &Position, &Monster), Added<Monster>>,
    generators: Query<(Entity, &Position), Added<Generator>>,
    pickups: Query<(Entity, &Position, &Pickup), Added<Pickup>>,
    projectiles: Query<(Entity, &Position, &Projectile), Added<Projectile>>,
    interactables: Query<(Entity, &Position, &Interactable), Added<Interactable>>,
    doors: Query<(Entity, &Position, &Door), Added<Door>>,
    traps: Query<(Entity, &Position), Added<Trap>>,
    exits: Query<(Entity, &Position, &LevelExit), Added<LevelExit>>,
    portcullises: Query<(Entity, &Position, &Portcullis), Added<Portcullis>>,
    landmarks: Query<(Entity, &Position, &Landmark), Added<Landmark>>,
) {
    // Un héros ou un monstre n'a pas de sprite: son modèle 3D le dessine. Il garde son cycle de marche, qui choisit sa pose.
    for (entity, pos, _) in &players {
        commands.entity(entity).insert((Visibility::default(), WalkCycle::default(), Transform::from_translation(pos.0.extend(1.0))));
    }
    for (entity, pos, _) in &monsters {
        commands.entity(entity).insert((Visibility::default(), WalkCycle::default(), Transform::from_translation(pos.0.extend(1.0))));
    }
    for (entity, pos) in &generators {
        commands.entity(entity).insert((Prop(object::GENERATOR), Visibility::default(), Transform::from_translation(pos.0.extend(0.5))));
    }
    for (entity, pos, pickup) in &pickups {
        let index = match pickup.0 {
            PickupKind::Coin => object::COIN,
            PickupKind::Food => object::FOOD,
            PickupKind::Key => object::KEY,
            PickupKind::Potion => object::POTION,
            PickupKind::Ingredient => object::INGREDIENT,
            PickupKind::GearPart => object::GEAR_PART,
            PickupKind::TargetDisc => object::TARGET_DISC,
        };
        commands.entity(entity).insert((Prop(index), Visibility::default(), Transform::from_translation(pos.0.extend(0.6))));
    }
    for (entity, pos, interactable) in &interactables {
        // La porte royale est dessinée par `doors`. Les panneaux sont dans le mur: vus de face, centrés sur leur tuile
        // (leur état est ensuite suivi par `update_props`).
        let row = match interactable.kind {
            InteractKind::Target => object::WALL_TARGET_EMPTY,
            InteractKind::Rune => object::WALL_RUNE,
            InteractKind::Mechanism => object::WALL_GEARS_JAMMED,
            InteractKind::Push => continue,
        };
        let (sprite, anchor) = art.prop_sprite(row, 0, Vec2::splat(SPRITE_SIZE));
        commands.entity(entity).insert((
            sprite,
            Prop(row),
            anchor,
            Transform::from_translation(pos.0.extend(0.4)),
        ));
    }
    // La herse et les portes sont dessinées dans la scène 3D (voir `decor3d::gate`).
    for (entity, pos, _) in &portcullises {
        commands.entity(entity).insert((Transform::from_translation(pos.0.extend(0.75)), Visibility::default()));
    }
    for (entity, pos, _) in &landmarks {
        commands.entity(entity).insert((Visibility::default(), Transform::from_translation(pos.0.extend(1.0))));
    }
    for (entity, pos, _) in &exits {
        commands.entity(entity).insert((Visibility::default(), Transform::from_translation(pos.0.extend(0.3))));
    }
    for (entity, pos) in &traps {
        commands.entity(entity).insert((Prop(object::TRAP_ARMED), Visibility::default(), Transform::from_translation(pos.0.extend(0.3))));
    }
    for (entity, pos, _) in &doors {
        commands.entity(entity).insert((Transform::from_translation(pos.0.extend(0.7)), Visibility::default()));
    }
    for (entity, pos, shot) in &projectiles {
        let index = match shot.class {
            HeroClass::Sorcerer => projectile::ORB_A,
            HeroClass::Forgebraise => projectile::BOLT,
            _ => projectile::ARROW,
        };
        commands.entity(entity).insert((
            art.projectiles.sprite_sized(index, Vec2::splat(22.0)),
            Lift(ARM_HEIGHT),
            Launch(pos.0),
            // Dessiné vers la droite: on le tourne dans le sens du tir.
            Transform {
                translation: (pos.0 + Vec2::new(0.0, ARM_HEIGHT)).extend(1.1),
                rotation: Quat::from_rotation_z(shot.dir.to_angle()),
                ..default()
            },
        ));
    }
}

/// La sortie change de modèle quand elle s'ouvre (pont réparé, barrière ouverte...), géré par `live3d`; l'escalier, qui reste le même,
/// est grisé tant qu'il est fermé.
fn update_exit(mut commands: Commands, exits: Query<(Entity, &LevelExit, Option<&ModelTint>)>) {
    for (entity, exit, tint) in &exits {
        let dim = exit.kind == ExitKind::Stairs && !exit.open;
        let color = if dim { Color::srgb(0.5, 0.5, 0.58) } else { Color::WHITE };
        if tint.map(|tint| tint.0) != Some(color) {
            commands.entity(entity).insert(ModelTint(color));
        }
    }
}

/// Un héros à terre devient translucide.
/// Un héros à terre est translucide; sous potion de régénération il pulse en vert.
fn fade_downed(mut commands: Commands, time: Res<Time>, mut players: Query<(Entity, Option<&ModelTint>, Has<Downed>, Has<Regenerating>), With<Player>>) {
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 6.0).sin();
    for (entity, tint, downed, regenerating) in &mut players {
        let color = if downed {
            Color::WHITE.with_alpha(0.35)
        } else if regenerating {
            Color::srgb(0.65 + 0.2 * pulse, 1.0, 0.7 + 0.2 * pulse)
        } else {
            Color::WHITE
        };
        if tint.map(|tint| tint.0) != Some(color) {
            commands.entity(entity).insert(ModelTint(color));
        }
    }
}

/// Apparence des objets selon leur état: piège réveillé ou non, panneaux d'énigme (support vide ou cible garnie, mécanisme
/// grippé ou réparé), rune éteinte une fois l'enchantement dissipé.
fn update_props(
    mut traps: Query<(&Trap, &mut Prop), Without<Interactable>>,
    mut interactables: Query<(&Interactable, &mut Sprite, &mut Prop), Without<Trap>>,
) {
    for (trap, mut prop) in &mut traps {
        let row = if trap.armed { object::TRAP_ARMED } else { object::TRAP_OFF };
        if prop.0 != row {
            prop.0 = row;
        }
    }
    for (interactable, mut sprite, mut prop) in &mut interactables {
        let row = match interactable.kind {
            InteractKind::Target if interactable.done => object::WALL_TARGET_HIT,
            InteractKind::Target if interactable.mounted => object::WALL_TARGET_MOUNTED,
            InteractKind::Target => object::WALL_TARGET_EMPTY,
            InteractKind::Rune => object::WALL_RUNE,
            InteractKind::Mechanism if interactable.done => object::WALL_GEARS_FIXED,
            InteractKind::Mechanism => object::WALL_GEARS_JAMMED,
            InteractKind::Push => continue,
        };
        if prop.0 != row {
            prop.0 = row;
        }
        // La rune s'éteint quand l'enchantement est dissipé; les autres panneaux restent vifs, leur dessin change.
        let color = Color::WHITE.with_alpha(if interactable.kind == InteractKind::Rune && interactable.done { 0.35 } else { 1.0 });
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

/// Barres de vie au-dessus des héros et des monstres.
fn draw_health_bars(mut gizmos: Gizmos, entities: Query<(&Transform, &Health), Without<Player>>) {
    const WIDTH: f32 = 14.0;
    // Seuls les ennemis et les générateurs blessés ont une barre: celles des héros sont dans l'interface.
    for (transform, health) in &entities {
        if health.current >= health.max {
            continue;
        }
        let origin = transform.translation.truncate() + Vec2::new(-WIDTH / 2.0, 10.0);
        let ratio = (health.current / health.max).clamp(0.0, 1.0);
        gizmos.line_2d(origin, origin + Vec2::X * WIDTH, Color::from(css::DARK_RED));
        if ratio > 0.0 {
            gizmos.line_2d(origin, origin + Vec2::X * WIDTH * ratio, Color::from(css::LIME));
        }
    }
}

/// Jauges des actions en cours et enchantement de la porte royale (les pouvoirs des héros ont leurs effets dans `fx`).
fn draw_effects(
    mut gizmos: Gizmos,
    actions: Query<(&Position, &Interactable)>,
    wards: Query<(&Position, &Ward)>,
    time: Res<Time>,
) {
    // Avancement d'une action en cours (cible, rune, mécanisme, porte), dans la couleur de la classe qui agit. Les
    // panneaux sont dans le mur: la jauge est dessinée sur le sol, devant eux.
    for (pos, interactable) in &actions {
        if interactable.done || interactable.progress <= 0.0 {
            continue;
        }
        let below = if interactable.kind == InteractKind::Push { 12.0 } else { -12.0 };
        let origin = pos.0 + Vec2::new(-8.0, below);
        gizmos.line_2d(origin, origin + Vec2::X * 16.0, Color::from(css::DARK_SLATE_GRAY));
        gizmos.line_2d(origin, origin + Vec2::X * 16.0 * interactable.progress, class_color(interactable.kind.class()));
    }
    // L'enchantement de la porte royale: cercles violets qui palpitent, et des runes qui tournent sur le bord.
    let t = time.elapsed_secs();
    for (pos, ward) in &wards {
        let violet = Color::from(css::MEDIUM_ORCHID);
        let breathe = 0.5 + 0.5 * (t * 2.5).sin();
        for (k, scale) in [1.0f32, 0.9, 0.78].into_iter().enumerate() {
            gizmos.circle_2d(Isometry2d::from_translation(pos.0), ward.radius * scale, violet.with_alpha(0.25 + 0.3 * breathe - 0.07 * k as f32));
        }
        for k in 0..16 {
            let angle = k as f32 * std::f32::consts::TAU / 16.0 + t * 0.4;
            let (dir, outer) = (Vec2::from_angle(angle), ward.radius);
            gizmos.line_2d(pos.0 + dir * (outer - 5.0), pos.0 + dir * (outer + 1.0), violet.with_alpha(0.6));
        }
    }
}

/// Lisse le déplacement entre deux mises à jour du serveur.
pub fn smooth_positions(time: Res<Time>, mut players: Query<(&Position, &mut Transform, Option<&Lift>)>) {
    for (pos, mut transform, lift) in &mut players {
        let z = transform.translation.z;
        let target = (pos.0 + Vec2::new(0.0, lift.map_or(0.0, |l| l.0))).extend(z);
        transform.translation.smooth_nudge(&target, SMOOTHING, time.delta_secs());
    }
}

/// Un tir vers le haut de l'écran part de dos: il reste derrière le héros jusqu'à s'en être éloigné, au lieu de sortir de
/// son dos. Les autres tirs sont dessinés devant.
fn layer_projectiles(mut shots: Query<(&mut Transform, &Position, &Launch, &Projectile)>) {
    for (mut transform, pos, launch, shot) in &mut shots {
        let behind = shot.dir.y > 0.3 && pos.0.distance(launch.0) < BODY_CLEAR;
        let z = if behind { 0.99 } else { 1.1 };
        if transform.translation.z != z {
            transform.translation.z = z;
        }
    }
}

/// L'orbe magique pulse entre deux images.
fn animate_projectiles(time: Res<Time>, mut shots: Query<(&mut Sprite, &Projectile)>) {
    let pulse = (time.elapsed_secs() * 12.0) as usize % 2;
    for (mut sprite, shot) in &mut shots {
        if shot.class != HeroClass::Sorcerer {
            continue;
        }
        let index = if pulse == 0 { projectile::ORB_A } else { projectile::ORB_B };
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
            }
        }
    }
}

/// Petite particule qui rétrécit et disparaît: étincelles derrière l'orbe magique et le carreau de tourelle.
#[derive(Component)]
struct Particle(Timer);

fn projectile_trails(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    shots: Query<(&Transform, &Projectile)>,
) {
    *since_last += time.delta_secs();
    if *since_last < 0.035 {
        return;
    }
    *since_last = 0.0;
    for (transform, shot) in &shots {
        let color = match shot.class {
            HeroClass::Sorcerer => css::AQUA,
            HeroClass::Forgebraise => css::ORANGE,
            _ => continue,
        };
        // Un léger décalage aléatoire (tiré de la position) évite une traîne parfaitement droite.
        let wobble = ((transform.translation.x * 7.3 + transform.translation.y * 3.1).sin()) * 1.5;
        commands.spawn((
            Sprite::from_color(Color::from(color), Vec2::splat(2.0)),
            Transform::from_xyz(
                transform.translation.x - shot.dir.x * 4.0 + wobble,
                transform.translation.y - shot.dir.y * 4.0 + wobble,
                transform.translation.z - 0.05,
            ),
            Particle(Timer::from_seconds(0.28, TimerMode::Once)),
        ));
    }
}

fn fade_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut particle, mut transform, mut sprite) in &mut particles {
        particle.0.tick(time.delta());
        let remaining = 1.0 - particle.0.fraction();
        transform.scale = Vec3::splat(remaining.max(0.1));
        sprite.color = sprite.color.with_alpha(remaining);
        if particle.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-5
    }

    #[test]
    fn without_a_camera_angle_the_directions_are_unchanged() {
        for dir in [Vec2::Y, Vec2::X, Vec2::new(0.6, -0.8)] {
            assert!(close(screen_to_world(dir, 0.0), dir));
        }
    }

    /// Une caméra tournée vers l'est (angle de +90°) regarde vers l'ouest: le haut de l'écran est l'ouest, la droite est le nord.
    #[test]
    fn up_on_the_screen_follows_the_camera() {
        let quarter = std::f32::consts::FRAC_PI_2;
        assert!(close(screen_to_world(Vec2::Y, quarter), Vec2::NEG_X));
        assert!(close(screen_to_world(Vec2::X, quarter), Vec2::Y));
        // Un angle de -15° (la caméra un peu à l'ouest du sud) fait monter vers le nord-est.
        let up = screen_to_world(Vec2::Y, (-15.0f32).to_radians());
        assert!(up.x > 0.2 && up.y > 0.9, "{up}");
        assert!((up.length() - 1.0).abs() < 1e-5);
    }
}
