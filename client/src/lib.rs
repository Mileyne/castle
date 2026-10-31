//! Client de Castle: fenêtre, rendu pixel-art et saisie.
//!
//! Bibliothèque lancée par `run`: le programme `castle_client` (ordinateur) et l'app iOS (`mobile/`) l'appellent tous deux.

use std::net::{Ipv4Addr, UdpSocket};
use std::time::SystemTime;

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetClient, RepliconRenetPlugins,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
};
use castle_shared::*;

use art::{Art, HERO_ANCHOR, PROP_ANCHOR, Prop, SPRITE_SIZE, WALL_ANCHOR, WalkCycle, direction_index, hero_index, monster_index, monster_scale, object, object_index, projectile};

mod art;
mod camera;
mod controls;
mod dialogue;
#[cfg(target_os = "ios")]
mod gamecontroller;
mod gore;
mod host;
mod i18n;
mod lighting;
mod pause;
mod puzzle;
mod settings;
mod level_intro;
mod music;
mod quest;
mod title;
mod touch;
mod weather;
mod ui;

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

/// Lance le jeu (ne revient qu'à la fermeture).
pub fn run() {
    // Assets du dépôt si on lance depuis une compilation du dépôt, sinon à côté du programme.
    // Un dossier `assets` à côté du programme (version livrée) passe avant celui du dépôt.
    let beside_exe = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("assets")));
    let dev_assets = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    let assets_path = match beside_exe {
        Some(dir) if dir.exists() => dir.to_string_lossy().into_owned(),
        _ if std::path::Path::new(dev_assets).exists() => dev_assets.to_string(),
        _ => "assets".to_string(),
    };
    App::new()
        .add_plugins((
            // `nearest` garde les pixels nets.
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: assets_path.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(window()),
                    ..default()
                }),
            RepliconPlugins,
            RepliconRenetPlugins,
            CastleSharedPlugin,
            i18n::I18nPlugin,
            ui::UiPlugin,
            title::TitlePlugin,
            pause::PausePlugin,
            quest::QuestPlugin,
            level_intro::LevelIntroPlugin,
            art::ArtPlugin,
            camera::CameraPlugin,
            lighting::LightingPlugin,
            gore::GorePlugin,
            dialogue::DialoguePlugin,
        ))
        .add_plugins((settings::SettingsPlugin, controls::ControlsPlugin, puzzle::PuzzlePlugin, touch::TouchPlugin, music::MusicPlugin, weather::WeatherPlugin))
        .add_plugins({
            #[cfg(target_os = "ios")]
            {
                gamecontroller::GameControllerPlugin
            }
            #[cfg(not(target_os = "ios"))]
            {
                ()
            }
        })
        .insert_resource(ClearColor(Color::BLACK))
        // Fenêtre au premier plan: rendu continu. En arrière-plan: le jeu se met presque au repos.
        .insert_resource(bevy::winit::WinitSettings::game())
        .add_systems(Startup, connect_from_command_line)
        .add_systems(Update, begin_connection.run_if(resource_exists::<host::ConnectAt>))
        .add_observer(mark_local_player)
        .add_systems(
            Update,
            (
                send_input.run_if(in_state(ClientState::Connected)).run_if(level_intro::player_can_act),
                raise_active_hero.after(smooth_positions),
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
        )
        .run();
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
#[cfg(not(target_os = "ios"))]
fn window() -> Window {
    Window {
        title: "Castle".into(),
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

/// Une adresse en argument (ou `--no-server`) rejoint directement un serveur, sans passer par le menu.
fn connect_from_command_line(mut commands: Commands) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let addr = match args.iter().find(|a| !a.starts_with("--")) {
        Some(arg) => arg.parse().ok(),
        None if args.iter().any(|a| a == "--no-server") => Some((Ipv4Addr::LOCALHOST, DEFAULT_PORT).into()),
        None => None,
    };
    if let Some(addr) = addr {
        commands.insert_resource(host::ConnectAt::now(addr));
    }
}

/// Ouvre la connexion au serveur une fois l'heure venue (le serveur local a besoin d'un instant pour démarrer).
fn begin_connection(mut commands: Commands, channels: Res<RepliconChannels>, request: Res<host::ConnectAt>) -> Result {
    if std::time::Instant::now() < request.at {
        return Ok(());
    }
    let addr = request.addr;
    commands.remove_resource::<host::ConnectAt>();

    let client = RenetClient::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    let transport = NetcodeClientTransport::new(
        current_time,
        ClientAuthentication::Unsecure {
            client_id: current_time.as_millis() as u64,
            protocol_id: PROTOCOL_ID,
            server_addr: addr,
            user_data: None,
        },
        socket,
    )?;

    info!("connecting to {addr}");
    commands.insert_resource(client);
    commands.insert_resource(transport);
    Ok(())
}

/// Le tir maintenu: depuis combien de temps, et si le pouvoir spécial a déjà été lancé pendant cette pression.
#[derive(Default)]
struct FireHold {
    held: f32,
    special_sent: bool,
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
    let dir = if pause.open { Vec2::ZERO } else { dir.normalize_or_zero() };
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
        commands.client_trigger(PlayerInput {
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
                commands.client_trigger(SwitchHero(class));
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
            commands.client_trigger(SwitchHero(class));
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
    turrets: Query<(Entity, &Position), Added<Turret>>,
    projectiles: Query<(Entity, &Position, &Projectile), Added<Projectile>>,
    interactables: Query<(Entity, &Position, &Interactable), Added<Interactable>>,
    doors: Query<(Entity, &Position, &Door), Added<Door>>,
    traps: Query<(Entity, &Position), Added<Trap>>,
    exits: Query<(Entity, &Position, &LevelExit), Added<LevelExit>>,
    portcullises: Query<(Entity, &Position, &Portcullis), Added<Portcullis>>,
) {
    // Un objet animé: sprite de la ligne `row` de la planche, ancré au sol.
    let prop = |row: usize| {
        (
            art.objects.sprite_sized(object_index(row, 0), Vec2::splat(SPRITE_SIZE)),
            Prop(row),
            PROP_ANCHOR,
        )
    };
    for (entity, pos, class) in &players {
        // Au départ le héros regarde vers le sud (vers le spectateur).
        let index = hero_index(class.copied().unwrap_or_default(), direction_index(Vec2::NEG_Y), 0);
        commands.entity(entity).insert((
            art.heroes.sprite_sized(index, Vec2::splat(SPRITE_SIZE)),
            HERO_ANCHOR,
            WalkCycle::default(),
            Transform::from_translation(pos.0.extend(1.0)),
        ));
    }
    for (entity, pos, monster) in &monsters {
        commands.entity(entity).insert((
            art.monsters.sprite_sized(monster_index(monster.0, direction_index(Vec2::NEG_Y), 0), Vec2::splat(SPRITE_SIZE * monster_scale(monster.0))),
            PROP_ANCHOR,
            WalkCycle::default(),
            Transform::from_translation(pos.0.extend(0.9)),
        ));
    }
    for (entity, pos) in &generators {
        commands.entity(entity).insert((
            prop(object::GENERATOR),
            Transform::from_translation(pos.0.extend(0.5)),
        ));
    }
    for (entity, pos) in &turrets {
        commands.entity(entity).insert((
            prop(object::TURRET),
            Transform::from_translation(pos.0.extend(0.8)),
        ));
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
        commands.entity(entity).insert((
            prop(index),
            Transform::from_translation(pos.0.extend(0.6)),
        ));
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
        commands.entity(entity).insert((
            art.objects.sprite_sized(object_index(row, 0), Vec2::splat(SPRITE_SIZE)),
            Prop(row),
            WALL_ANCHOR,
            Transform::from_translation(pos.0.extend(0.4)),
        ));
    }
    // La herse: deux moitiés côte à côte, comme la porte.
    for (entity, pos, herse) in &portcullises {
        let axis = if herse.vertical { Vec2::Y } else { Vec2::X };
        commands
            .entity(entity)
            .insert((Transform::from_translation(pos.0.extend(0.75)), Visibility::default()))
            .with_children(|parent| {
                for side in [-1.0, 1.0] {
                    parent.spawn((
                        prop(object::PORTCULLIS),
                        Transform::from_translation((axis * side * TILE_SIZE / 2.0).extend(0.0)),
                    ));
                }
            });
    }
    for (entity, pos, exit) in &exits {
        commands.entity(entity).insert((
            art.exits.sprite_sized(art::exit_index(art::exit_row(exit.kind, exit.open), 0), Vec2::splat(art::EXIT_SIZE)),
            PROP_ANCHOR,
            Transform::from_translation(pos.0.extend(0.3)),
        ));
    }
    for (entity, pos) in &traps {
        commands.entity(entity).insert((
            prop(object::TRAP_ARMED),
            Transform::from_translation(pos.0.extend(0.3)),
        ));
    }
    for (entity, pos, door) in &doors {
        // Une porte fait deux tuiles, côte à côte ou l'une sur l'autre selon le couloir.
        let index_for = |side: f32| match door.kind {
            DoorKind::Locked => object::DOOR_LOCKED,
            DoorKind::Royal if side < 0.0 => object::DOOR_ROYAL,
            DoorKind::Royal => object::DOOR_ROYAL_EMBLEM,
        };
        let axis = if door.vertical { Vec2::Y } else { Vec2::X };
        commands
            .entity(entity)
            .insert((Transform::from_translation(pos.0.extend(0.7)), Visibility::default()))
            .with_children(|parent| {
                for side in [-1.0, 1.0] {
                    parent.spawn((
                        prop(index_for(side)),
                        Transform::from_translation((axis * side * TILE_SIZE / 2.0).extend(0.0)),
                    ));
                }
            });
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

/// La sortie change de dessin quand elle s'ouvre (pont réparé, barrière ouverte...) et vacille (torches, bannières); l'escalier,
/// qui reste le même, est grisé tant qu'il est fermé.
fn update_exit(time: Res<Time>, mut exits: Query<(&LevelExit, &mut Sprite)>) {
    let frame = (time.elapsed_secs() * 12.0) as usize % art::EXIT_FRAMES;
    for (exit, mut sprite) in &mut exits {
        let index = art::exit_index(art::exit_row(exit.kind, exit.open), frame);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            if atlas.index != index {
                atlas.index = index;
            }
        }
        let dim = exit.kind == ExitKind::Stairs && !exit.open;
        let color = if dim { Color::srgb(0.5, 0.5, 0.58) } else { Color::WHITE };
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

/// Un héros à terre devient translucide.
/// Un héros à terre est translucide; sous potion de régénération il pulse en vert.
fn fade_downed(time: Res<Time>, mut players: Query<(&mut Sprite, Has<Downed>, Has<Regenerating>), With<Player>>) {
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 6.0).sin();
    for (mut sprite, downed, regenerating) in &mut players {
        let color = if downed {
            Color::WHITE.with_alpha(0.35)
        } else if regenerating {
            Color::srgb(0.65 + 0.2 * pulse, 1.0, 0.7 + 0.2 * pulse)
        } else {
            Color::WHITE
        };
        if sprite.color != color {
            sprite.color = color;
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

/// Zones d'effet et rempart, dessinés en cercles concentriques.
fn draw_effects(
    mut gizmos: Gizmos,
    zones: Query<(&Position, &Zone)>,
    shielded: Query<&Transform, With<Shielded>>,
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
    for (pos, zone) in &zones {
        let color = class_color(zone.class);
        for k in [1.0, 0.66, 0.33] {
            gizmos.circle_2d(
                Isometry2d::from_translation(pos.0),
                zone.radius * k,
                color.with_alpha(0.4 + 0.3 * (1.0 - k)),
            );
        }
    }
    for transform in &shielded {
        gizmos.circle_2d(
            Isometry2d::from_translation(transform.translation.truncate()),
            HERO_HALF_SIZE + 4.0,
            Color::from(css::DEEP_SKY_BLUE),
        );
    }
}

/// Lisse le déplacement entre deux mises à jour du serveur.
pub fn smooth_positions(time: Res<Time>, mut players: Query<(&Position, &mut Transform, Option<&Lift>)>) {
    let t = 1.0 - (-SMOOTHING * time.delta_secs()).exp();
    for (pos, mut transform, lift) in &mut players {
        let z = transform.translation.z;
        let target = (pos.0 + Vec2::new(0.0, lift.map_or(0.0, |l| l.0))).extend(z);
        transform.translation = transform.translation.lerp(target, t);
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

/// Le héros dirigé par le joueur est dessiné au premier plan, devant les autres héros et les monstres.
fn raise_active_hero(mut heroes: Query<(&mut Transform, Has<LocalPlayer>), With<Player>>) {
    for (mut transform, active) in &mut heroes {
        let z = if active { 1.08 } else { 1.0 };
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
