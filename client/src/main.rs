//! Client de Castle: fenêtre, rendu pixel-art et saisie.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetClient, RepliconRenetPlugins,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
};
use castle_shared::*;

use art::{Art, HERO_ANCHOR, PROP_ANCHOR, Prop, SPRITE_SIZE, WalkCycle, direction_index, fx, hero_index, monster_index, object, object_index, projectile};

mod art;
mod camera;
mod dialogue;
mod gore;
mod i18n;
mod lighting;
mod level_intro;
mod title;
mod ui;

/// Vitesse de rattrapage visuel vers la position du serveur.
const SMOOTHING: f32 = 20.0;

fn main() {
    App::new()
        .add_plugins((
            // `nearest` garde les pixels nets.
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    // En développement on lit les assets du dépôt, quel que soit le dossier de lancement.
                    file_path: if cfg!(debug_assertions) {
                        concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into()
                    } else {
                        "assets".into()
                    },
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Castle".into(),
                        ..default()
                    }),
                    ..default()
                }),
            RepliconPlugins,
            RepliconRenetPlugins,
            CastleSharedPlugin,
            i18n::I18nPlugin,
            ui::UiPlugin,
            title::TitlePlugin,
            level_intro::LevelIntroPlugin,
            art::ArtPlugin,
            camera::CameraPlugin,
            lighting::LightingPlugin,
            gore::GorePlugin,
            dialogue::DialoguePlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        // Fenêtre au premier plan: rendu continu. En arrière-plan: le jeu se met presque au repos.
        .insert_resource(bevy::winit::WinitSettings::game())
        .add_systems(Startup, connect)
        .add_observer(mark_local_player)
        .add_observer(spawn_attack_fx)
        .add_systems(
            Update,
            (
                send_input.run_if(in_state(ClientState::Connected)).run_if(level_intro::player_can_act),
                raise_active_hero.after(smooth_positions),
                (dress_new_entities, fade_downed).chain(),
                smooth_positions,
                draw_health_bars,
                draw_effects,
                update_secrets,
                update_props,
                fade_attack_fx,
                animate_projectiles,
                (projectile_trails, fade_particles).chain(),
            ),
        )
        .run();
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

fn connect(mut commands: Commands, channels: Res<RepliconChannels>) -> Result {
    // L'adresse du serveur est le premier argument qui n'est pas une option (`--gore`...).
    let addr: SocketAddr = match std::env::args().skip(1).find(|a| !a.starts_with("--")) {
        Some(arg) => arg.parse()?,
        None => (Ipv4Addr::LOCALHOST, DEFAULT_PORT).into(),
    };

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

/// Envoie direction, attaque et pouvoir quand ils changent, et le héros choisi (touches 1 à 4).
fn send_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut last: Local<(Vec2, bool, bool, bool)>,
    dialogue: Res<dialogue::DialogueState>,
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
    let dir = dir.normalize_or_zero();
    let attack = keys.pressed(KeyCode::Space);
    let special = keys.pressed(KeyCode::KeyE);
    let interact = keys.pressed(KeyCode::KeyF);
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
    if dialogue.is_voting() {
        return;
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
    portals: Query<(Entity, &Position), Added<Portal>>,
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
            art.monsters.sprite_sized(monster_index(monster.0, direction_index(Vec2::NEG_Y), 0), Vec2::splat(SPRITE_SIZE)),
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
        };
        commands.entity(entity).insert((
            prop(index),
            Transform::from_translation(pos.0.extend(0.6)),
        ));
    }
    for (entity, pos, interactable) in &interactables {
        // La porte royale est dessinée par `doors`.
        let index = match interactable.kind {
            InteractKind::Lever => object::LEVER,
            InteractKind::Rune => object::RUNE,
            InteractKind::Mechanism => object::MECHANISM,
            InteractKind::Push => continue,
        };
        commands.entity(entity).insert((
            prop(index),
            Transform::from_translation(pos.0.extend(0.4)),
        ));
    }
    for (entity, pos) in &portals {
        commands.entity(entity).insert((
            prop(object::PORTAL),
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
            // Dessiné vers la droite: on le tourne dans le sens du tir.
            Transform {
                translation: pos.0.extend(1.1),
                rotation: Quat::from_rotation_z(shot.dir.to_angle()),
                ..default()
            },
        ));
    }
}

/// Un héros à terre devient translucide.
fn fade_downed(mut players: Query<(&mut Sprite, Has<Downed>), With<Player>>) {
    for (mut sprite, downed) in &mut players {
        let color = Color::WHITE.with_alpha(if downed { 0.35 } else { 1.0 });
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

/// Leviers et pièges ne se voient qu'à proximité d'un héros qui détecte les secrets
/// (Ensorceleur ou Rôdeur, de n'importe quel joueur).
fn update_secrets(
    detectors: Query<(&Position, &HeroClass), With<Player>>,
    mut secrets: Query<(&Position, &mut Visibility, Option<&Interactable>), Or<(With<Trap>, With<Interactable>)>>,
) {
    let detectors: Vec<Vec2> = detectors
        .iter()
        .filter(|(_, class)| class.detects_secrets())
        .map(|(p, _)| p.0)
        .collect();
    for (pos, mut visibility, interactable) in &mut secrets {
        if interactable.is_some_and(|i| !i.kind.is_secret()) {
            continue;
        }
        let seen = interactable.is_some_and(|i| i.done)
            || detectors.iter().any(|d| d.distance(pos.0) <= DETECT_RADIUS);
        let target = if seen { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != target {
            *visibility = target;
        }
    }
}

/// Apparence des objets selon leur état: piège désarmé, étape terminée.
fn update_props(
    mut traps: Query<(&Trap, &mut Prop), Without<Interactable>>,
    mut interactables: Query<(&Interactable, &mut Sprite), Without<Trap>>,
) {
    for (trap, mut prop) in &mut traps {
        let row = if trap.armed { object::TRAP_ARMED } else { object::TRAP_OFF };
        if prop.0 != row {
            prop.0 = row;
        }
    }
    for (interactable, mut sprite) in &mut interactables {
        let color = Color::WHITE.with_alpha(if interactable.done { 0.35 } else { 1.0 });
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
) {
    // Avancement d'une action en cours (levier, rune, mécanisme, porte).
    for (pos, interactable) in &actions {
        if interactable.done || interactable.progress <= 0.0 {
            continue;
        }
        let origin = pos.0 + Vec2::new(-8.0, 12.0);
        gizmos.line_2d(origin, origin + Vec2::X * 16.0, Color::from(css::DARK_SLATE_GRAY));
        gizmos.line_2d(origin, origin + Vec2::X * 16.0 * interactable.progress, Color::from(css::GOLD));
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

/// Effet visuel bref d'un coup au corps à corps.
#[derive(Component)]
struct AttackFxLife(Timer);

fn spawn_attack_fx(fx_event: On<AttackFx>, mut commands: Commands, art: Res<Art>) {
    let Attack::Melee { range, .. } = fx_event.class.stats().attack else {
        return;
    };
    // Le croissant d'épée est centré sur le héros; l'onde de marteau éclate au bout de sa portée.
    let (index, reach) = match fx_event.class {
        HeroClass::Forgebraise => (fx::SMASH, range - 2.7),
        _ => (fx::SLASH, 14.0),
    };
    commands.spawn((
        art.fx.sprite_sized(index, Vec2::splat(32.0)),
        Transform {
            translation: (fx_event.pos + fx_event.dir * reach).extend(1.2),
            rotation: Quat::from_rotation_z(fx_event.dir.to_angle()),
            ..default()
        },
        AttackFxLife(Timer::from_seconds(0.16, TimerMode::Once)),
    ));
}

fn fade_attack_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut fx: Query<(Entity, &mut AttackFxLife, &mut Sprite)>,
) {
    for (entity, mut life, mut sprite) in &mut fx {
        // L'effet s'estompe au fil de sa courte durée.
        sprite.color = Color::WHITE.with_alpha(1.0 - life.0.fraction());
        if life.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Lisse le déplacement entre deux mises à jour du serveur.
pub fn smooth_positions(time: Res<Time>, mut players: Query<(&Position, &mut Transform)>) {
    let t = 1.0 - (-SMOOTHING * time.delta_secs()).exp();
    for (pos, mut transform) in &mut players {
        let z = transform.translation.z;
        let target = pos.0.extend(z);
        transform.translation = transform.translation.lerp(target, t);
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
                1.05,
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
