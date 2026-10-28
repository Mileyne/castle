//! Serveur dédié de Castle: simulation autoritaire, sans fenêtre.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;
use std::time::SystemTime;

use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetServer, RepliconRenetPlugins,
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::ConnectionConfig,
};
use castle_shared::*;

/// Compagnons: distance de détection des ennemis, portée de tir, retour auprès du chef.
const AI_AGGRO: f32 = 110.0;
/// Un itinéraire est recalculé quand le but a bougé de plus que cela, ou après ce délai.
const ROUTE_GOAL_DRIFT: f32 = 24.0;
const ROUTE_MAX_AGE: f32 = 0.6;
const AI_SHOT_REACH: f32 = 150.0;
const AI_LEASH: f32 = 220.0;
/// Les compagnons n'engagent que les ennemis à cette distance du héros dirigé.
const AI_GROUP_RADIUS: f32 = 170.0;
/// Les corps se bloquent: marge ajoutée à la portée de frappe d'un monstre.
const MONSTER_REACH_SLACK: f32 = 6.0;

/// Distance à laquelle un allié relève un héros à terre, et temps nécessaire.
const REVIVE_RANGE: f32 = 20.0;
const REVIVE_TIME: f32 = 1.5;

const GENERATOR_HEALTH: f32 = 40.0;
/// Un générateur ne produit que si un héros est à moins de cette distance.
const GENERATOR_ACTIVATION: f32 = 220.0;
const GENERATOR_INTERVAL: f32 = 3.5;
/// Nombre maximal de monstres vivants dans la salle.
const MAX_MONSTERS: usize = 12;
/// Le portail s'active quand tous les héros debout sont dans ce rayon, pendant ce délai.
const PORTAL_RADIUS: f32 = 36.0;
const PORTAL_DELAY: f32 = 1.5;

/// Rempart: rayon d'effet, durée et part des dégâts qui passe encore.
const BULWARK_RADIUS: f32 = 90.0;
const BULWARK_DURATION: f32 = 6.0;
const BULWARK_DAMAGE_FACTOR: f32 = 0.5;
/// Tempête Arcane: décalage devant la sorcière, rayon, dégâts.
const ARCANE_OFFSET: f32 = 56.0;
const ARCANE_RADIUS: f32 = 48.0;
const ARCANE_DAMAGE: f32 = 35.0;
/// Pluie de Flèches: décalage, rayon, dégâts par impulsion, intervalle et durée.
const RAIN_OFFSET: f32 = 80.0;
const RAIN_RADIUS: f32 = 56.0;
const RAIN_DAMAGE: f32 = 6.0;
const RAIN_INTERVAL: f32 = 0.25;
const RAIN_DURATION: f32 = 2.0;
/// Tourelle: durée de vie, portée, cadence, dégâts et vitesse des tirs.
const TURRET_LIFETIME: f32 = 15.0;
const TURRET_RANGE: f32 = 110.0;
const TURRET_COOLDOWN: f32 = 0.6;
const TURRET_DAMAGE: f32 = 7.0;
const TURRET_SHOT_SPEED: f32 = 220.0;

/// Pièges: dégâts, délai entre deux déclenchements, rayon de déclenchement, temps de désamorçage.
const TRAP_DAMAGE: f32 = 12.0;
const TRAP_COOLDOWN: f32 = 1.0;
const TRAP_TRIGGER_RADIUS: f32 = 8.0;
const TRAP_DISARM_TIME: f32 = 0.6;
/// Une porte à clé s'ouvre quand un héros muni d'une clé s'en approche à moins de cette distance.
const DOOR_OPEN_RANGE: f32 = 16.0;

/// Dialogues: pause minimale entre deux répliques, délai de vote, durée maximale sans activité.
const DIALOGUE_ADVANCE_COOLDOWN: f32 = 0.25;
const DIALOGUE_VOTE_TIMEOUT: f32 = 15.0;
const DIALOGUE_IDLE_TIMEOUT: f32 = 90.0;

const PICKUP_RANGE: f32 = 10.0;
const FOOD_HEAL: f32 = 25.0;

/// Direction demandée par le joueur, jamais répliquée.
#[derive(Component, Default)]
struct MoveDir(Vec2);

#[derive(Component, Default)]
struct Attacking(bool);

/// Temps restant avant la prochaine attaque.
#[derive(Component, Default)]
struct Cooldown(f32);

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
struct Damage(f32);

#[derive(Component)]
struct Lifetime(f32);

/// Progression de la réanimation d'un héros à terre, en secondes.
#[derive(Component, Default)]
struct Revive(f32);

/// État de la touche du pouvoir spécial; `queued` devient vrai sur chaque nouvel appui.
#[derive(Component, Default)]
struct SpecialInput {
    held: bool,
    queued: bool,
}

#[derive(Component, Default)]
struct SpecialCooldown(f32);

#[derive(Component)]
struct ShieldTimer(f32);

/// Durée restante d'une zone et rythme de ses dégâts.
#[derive(Component)]
struct ZoneLife(f32);

#[derive(Component)]
struct ZoneDamage {
    damage: f32,
    interval: f32,
    timer: f32,
}

#[derive(Component)]
struct TurretLife(f32);

#[derive(Component)]
struct TurretOwner(Entity);

/// Héros actuellement dirigé par un joueur (les autres de son équipe suivent et se défendent seuls).
#[derive(Component)]
struct Controlled;

/// Sur l'entité de connexion d'un client: le héros qu'il dirige, et son équipe.
#[derive(Component)]
struct ActiveHero(Entity);

#[derive(Component)]
struct ClientParty(u32);

#[derive(Resource, Default)]
struct NextParty(u32);

/// Itinéraire d'un compagnon vers sa cible: points de passage calculés par `Room::find_path`.
#[derive(Component, Default)]
struct Route {
    goal: Vec2,
    waypoints: Vec<Vec2>,
    /// Temps depuis le dernier calcul; l'itinéraire est refait régulièrement car le chef bouge.
    age: f32,
}

/// Coup en préparation: l'effet (dégâts, flèche partie) a lieu à l'instant de la frappe de l'animation d'arme.
#[derive(Component)]
struct PendingAttack {
    hero: Entity,
    timer: f32,
}

/// Délai entre le début du geste et l'instant de la frappe, en secondes.
const ATTACK_WINDUP: f32 = 0.2;

/// Touche d'action maintenue par le joueur.
#[derive(Component, Default)]
struct Interacting(bool);

/// Temps pendant lequel l'action est maintenue sur un objet, en secondes.
#[derive(Component, Default)]
struct Hold(f32);

/// Obstacle rectangulaire (demi-dimensions), par exemple une porte fermée.
#[derive(Component)]
struct Solid(Vec2);

/// Délai restant avant qu'un piège puisse se redéclencher.
#[derive(Component, Default)]
struct TrapCooldown(f32);

/// Demi-côté de la boîte qui reçoit les coups (monstres et générateurs).
#[derive(Component)]
struct Hurtbox(f32);

/// Temps avant la prochaine apparition d'un monstre.
#[derive(Component)]
struct SpawnTimer(f32);

/// Scène de dialogue en cours. Le serveur ne la joue pas: il la déclenche, fait avancer tout le monde
/// en même temps et compte les votes. Les textes vivent côté client.
#[derive(Resource, Default)]
struct DialogueSession {
    active: Option<String>,
    advance_cooldown: f32,
    votes: HashMap<Entity, u32>,
    vote_timer: f32,
    idle: f32,
}

/// Déclenche un dialogue la première fois qu'un héros reste dans la zone.
#[derive(Component)]
struct DialogueTrigger {
    node: &'static str,
    radius: f32,
    /// Temps de présence nécessaire avant le déclenchement.
    delay: f32,
    fired: bool,
}

/// Petit générateur pseudo-aléatoire (xorshift), suffisant pour le butin et les variations.
#[derive(Resource)]
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn main() {
    // Lancé par le client: on s'arrête dès que son tube d'entrée se ferme (client fermé ou tué).
    if std::env::var_os("CASTLE_EMBEDDED").is_some() {
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            std::process::exit(0);
        });
    }

    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        | 1;
    App::new()
        .add_plugins((
            // Sans cadence, la boucle d'un serveur sans fenêtre tourne à vide et occupe un cœur entier.
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(8))),
            StatesPlugin,
            bevy::log::LogPlugin::default(),
            RepliconPlugins,
            RepliconRenetPlugins,
            CastleSharedPlugin,
        ))
        .insert_resource(Rng(seed))
        .init_resource::<DialogueSession>()
        .init_resource::<NextParty>()
        .add_systems(Startup, (start_server, start_first_expedition))
        .add_systems(
            FixedUpdate,
            (
                // Actions des héros, puis effets, puis monde et dialogues.
                (
                    move_players,
                    player_attacks,
                    resolve_attacks,
                    auto_switch,
                    hero_ai,
                    leash_followers,
                    regen_mana,
                    player_specials,
                    tick_shields,
                    run_zones,
                    run_turrets,
                    move_projectiles,
                )
                    .chain(),
                (
                    run_generators,
                    run_dialogue_triggers,
                    tick_dialogue,
                    run_interactions,
                    run_traps,
                    open_locked_doors,
                    monster_ai,
                    reap_dead,
                    collect_pickups,
                    collect_relics,
                    rescue_captives,
                    open_portal,
                    down_players,
                    revive_players,
                    check_wipe,
                    run_portal,
                )
                    .chain(),
            )
                .chain(),
        )
        .add_observer(spawn_party)
        .add_observer(despawn_party)
        .add_observer(switch_hero)
        .add_observer(new_game)
        .add_observer(apply_input)
        .add_observer(dialogue_advance)
        .add_observer(dialogue_vote)
        .add_observer(dialogue_finished)
        .run();
}

fn start_server(mut commands: Commands, channels: Res<RepliconChannels>) -> Result {
    let port = std::env::args()
        .nth(1)
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT);

    let server = RenetServer::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });

    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
    let transport = NetcodeServerTransport::new(
        ServerConfig {
            current_time,
            max_clients: MAX_PLAYERS,
            protocol_id: PROTOCOL_ID,
            authentication: ServerAuthentication::Unsecure,
            public_addresses: Default::default(),
        },
        socket,
    )?;

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("Castle server listening on UDP port {port}");
    Ok(())
}

/// Un client authentifié reçoit une équipe de quatre héros; le Chevalier est dirigé au départ.
fn spawn_party(
    add: On<Add, AuthorizedClient>,
    mut commands: Commands,
    layout: Res<Layout>,
    mut next_party: ResMut<NextParty>,
    heroes: Query<(), With<Player>>,
) {
    let party = next_party.0;
    next_party.0 += 1;
    info!("player joined ({:?}), party {party}", add.entity);
    let first_slot = heroes.iter().count();
    let mut active = None;
    for (i, class) in HeroClass::ALL.into_iter().enumerate() {
        let stats = class.stats();
        let hero = commands
            .spawn((
                Replicated,
                Player,
                PartyId(party),
                class,
                Health::full(stats.max_hp as f32),
                Mana::full(stats.max_mana),
                Gold::default(),
                Keys::default(),
                Position(spawn_point(first_slot + i, &layout)),
            ))
            .insert((
                MoveDir::default(),
                Facing(Vec2::X),
                Attacking::default(),
                Cooldown::default(),
                Revive::default(),
                SpecialInput::default(),
                SpecialCooldown::default(),
                Interacting::default(),
                Route::default(),
                Strikes::default(),
            ))
            .id();
        if i == 0 {
            commands.entity(hero).insert(Controlled);
            active = Some(hero);
        }
    }
    let hero = active.expect("une équipe a au moins un héros");
    commands.entity(add.entity).insert((ActiveHero(hero), ClientParty(party)));
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(add.entity)),
        message: YouAre { hero, party },
    });
}

/// Quand un client part, son équipe disparaît avec lui.
fn despawn_party(
    remove: On<Remove, ConnectedClient>,
    mut commands: Commands,
    clients: Query<&ClientParty>,
    heroes: Query<(Entity, &PartyId), With<Player>>,
) {
    let Ok(party) = clients.get(remove.entity) else {
        return;
    };
    for (entity, id) in &heroes {
        if id.0 == party.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn apply_input(
    input: On<FromClient<PlayerInput>>,
    clients: Query<&ActiveHero>,
    mut heroes: Query<
        (&mut MoveDir, &mut Facing, &mut Attacking, &mut SpecialInput, &mut Interacting),
        (With<Controlled>, Without<Downed>),
    >,
) {
    let Some(client) = input.client_id.entity() else {
        return;
    };
    let Ok(active) = clients.get(client) else {
        return;
    };
    let Ok((mut dir, mut facing, mut attacking, mut special, mut interacting)) =
        heroes.get_mut(active.0)
    else {
        return;
    };
    // Ne jamais faire confiance au client: direction finie et bornée.
    dir.0 = if input.direction.is_finite() {
        input.direction.clamp_length_max(1.0)
    } else {
        Vec2::ZERO
    };
    if let Some(d) = dir.0.try_normalize() {
        facing.0 = d;
    }
    attacking.0 = input.attack;
    interacting.0 = input.interact;
    if input.special && !special.held {
        special.queued = true;
    }
    special.held = input.special;
}

/// Le joueur prend la main sur un autre héros de son équipe; l'ancien redevient un compagnon.
fn switch_hero(
    choice: On<FromClient<SwitchHero>>,
    mut commands: Commands,
    mut clients: Query<(&mut ActiveHero, &ClientParty)>,
    heroes: Query<(Entity, &PartyId, &HeroClass, Has<Downed>), With<Player>>,
) {
    let Some(client) = choice.client_id.entity() else {
        return;
    };
    let Ok((mut active, party)) = clients.get_mut(client) else {
        return;
    };
    let target = heroes
        .iter()
        .find(|(_, id, class, downed)| id.0 == party.0 && **class == choice.0 && !downed)
        .map(|(entity, ..)| entity);
    let Some(target) = target.filter(|t| *t != active.0) else {
        return;
    };
    hand_over(&mut commands, active.0, target);
    active.0 = target;
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: YouAre { hero: target, party: party.0 },
    });
}

/// Retire le contrôle à `from` (qui s'arrête) et le donne à `to`.
fn hand_over(commands: &mut Commands, from: Entity, to: Entity) {
    commands.entity(from).remove::<Controlled>().insert((
        MoveDir::default(),
        Attacking::default(),
        Interacting::default(),
        SpecialInput::default(),
    ));
    commands.entity(to).insert(Controlled);
}

/// Si le héros dirigé tombe, le contrôle passe à un autre héros debout de l'équipe.
fn auto_switch(
    mut commands: Commands,
    mut clients: Query<(Entity, &mut ActiveHero, &ClientParty)>,
    heroes: Query<(Entity, &PartyId, Has<Downed>), With<Player>>,
) {
    for (client, mut active, party) in &mut clients {
        let down = heroes.get(active.0).map(|(_, _, downed)| downed).unwrap_or(true);
        if !down {
            continue;
        }
        let Some((next, ..)) = heroes
            .iter()
            .find(|(_, id, downed)| id.0 == party.0 && !downed)
        else {
            continue;
        };
        hand_over(&mut commands, active.0, next);
        active.0 = next;
        commands.server_trigger(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: YouAre { hero: next, party: party.0 },
        });
    }
}

fn move_players(
    time: Res<Time>,
    room: Res<Room>,
    doors: Query<(&Position, &Solid), Without<Player>>,
    bodies: Query<(&Position, &Hurtbox), Without<Player>>,
    mut players: Query<(&mut Position, &MoveDir, &HeroClass), (With<Player>, Without<Downed>)>,
) {
    // Portes, monstres et générateurs arrêtent les héros; les héros se traversent entre eux.
    let solids: Vec<_> = doors
        .iter()
        .map(|(p, s)| (p.0, s.0))
        .chain(bodies.iter().map(|(p, h)| (p.0, Vec2::splat(h.0))))
        .collect();
    for (mut pos, dir, class) in &mut players {
        if dir.0 == Vec2::ZERO {
            continue;
        }
        let delta = dir.0 * class.stats().speed * time.delta_secs();
        pos.0 = room.move_and_collide_solids(pos.0, delta, HERO_HALF_SIZE, &solids);
    }
}

fn player_attacks(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<
        (Entity, &Attacking, &HeroClass, &mut Cooldown, &mut Strikes),
        (With<Player>, Without<Downed>),
    >,
) {
    for (entity, attacking, class, mut cooldown, mut strikes) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if !attacking.0 || cooldown.0 > 0.0 {
            continue;
        }
        cooldown.0 = class.stats().attack.cooldown();
        // Le client joue le geste; l'effet suit à l'instant de la frappe (`resolve_attacks`).
        strikes.0 = strikes.0.wrapping_add(1);
        commands.spawn(PendingAttack { hero: entity, timer: ATTACK_WINDUP });
    }
}

/// Applique l'effet d'une attaque une fois le geste arrivé à sa frappe: coup au corps à corps ou tir.
fn resolve_attacks(
    mut commands: Commands,
    time: Res<Time>,
    mut pending: Query<(Entity, &mut PendingAttack)>,
    heroes: Query<(&Position, &Facing, &HeroClass), (With<Player>, Without<Downed>)>,
    mut targets: Query<(&Position, &mut Health, &Hurtbox), Without<Player>>,
) {
    for (entity, mut attack) in &mut pending {
        attack.timer -= time.delta_secs();
        if attack.timer > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        // Le héros peut être tombé ou parti entre-temps: le coup est alors perdu.
        let Ok((pos, facing, class)) = heroes.get(attack.hero) else {
            continue;
        };
        match class.stats().attack {
            Attack::Melee { range, damage, .. } => {
                for (target_pos, mut health, hurtbox) in &mut targets {
                    let to = target_pos.0 - pos.0;
                    let dist = to.length();
                    // Cône d'environ 150° devant le héros, ou au contact.
                    let in_front = dist < 8.0 || to.normalize_or_zero().dot(facing.0) > 0.25;
                    if dist <= range + hurtbox.0 && in_front {
                        health.current -= damage;
                    }
                }
                commands.server_trigger(ToClients {
                    targets: SendTargets::All,
                    message: AttackFx { pos: pos.0, dir: facing.0, class: *class },
                });
            }
            Attack::Shot { speed, damage, lifetime, .. } => {
                commands.spawn((
                    Replicated,
                    Projectile { class: *class, dir: facing.0 },
                    Position(pos.0 + facing.0 * 8.0),
                    Velocity(facing.0 * speed),
                    Damage(damage),
                    Lifetime(lifetime),
                ));
            }
        }
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    room: Res<Room>,
    mut projectiles: Query<
        (Entity, &mut Position, &Velocity, &Damage, &mut Lifetime),
        With<Projectile>,
    >,
    mut targets: Query<
        (&Position, &mut Health, &Hurtbox),
        (Without<Player>, Without<Projectile>),
    >,
) {
    for (entity, mut pos, velocity, damage, mut lifetime) in &mut projectiles {
        let next = pos.0 + velocity.0 * time.delta_secs();
        lifetime.0 -= time.delta_secs();
        if lifetime.0 <= 0.0 || room.overlaps_wall(next, PROJECTILE_RADIUS) {
            commands.entity(entity).despawn();
            continue;
        }
        pos.0 = next;
        let hit = targets
            .iter_mut()
            .find(|(t, _, hurtbox)| t.0.distance(next) <= hurtbox.0 + PROJECTILE_RADIUS);
        if let Some((_, mut health, _)) = hit {
            health.current -= damage.0;
            commands.entity(entity).despawn();
        }
    }
}

/// Les générateurs produisent des monstres tant qu'un héros est proche.
fn run_generators(
    mut commands: Commands,
    time: Res<Time>,
    room: Res<Room>,
    mut rng: ResMut<Rng>,
    mut generators: Query<(&Position, &mut SpawnTimer), With<Generator>>,
    players: Query<&Position, (With<Player>, Without<Downed>, Without<Generator>)>,
    monsters: Query<(), With<Monster>>,
) {
    let mut alive = monsters.iter().count();
    for (pos, mut timer) in &mut generators {
        let nearest = players
            .iter()
            .map(|p| p.0)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        let Some(target) = nearest.filter(|t| t.distance(pos.0) <= GENERATOR_ACTIVATION) else {
            continue;
        };
        timer.0 -= time.delta_secs();
        if timer.0 > 0.0 || alive >= MAX_MONSTERS {
            continue;
        }
        timer.0 = GENERATOR_INTERVAL;
        let kind = if rng.next_f32() < 0.7 {
            MonsterKind::Skeleton
        } else {
            MonsterKind::Bat
        };
        // Le monstre sort du côté du héros, sauf si un mur gêne.
        let out = pos.0 + (target - pos.0).normalize_or_zero() * (GENERATOR_HALF_SIZE + 6.0);
        let at = if room.overlaps_wall(out, kind.stats().half_size) { pos.0 } else { out };
        spawn_monster(&mut commands, kind, at);
        alive += 1;
    }
}

fn monster_ai(
    time: Res<Time>,
    room: Res<Room>,
    doors: Query<(&Position, &Solid), (Without<Monster>, Without<Player>)>,
    generators: Query<(&Position, &Hurtbox), (With<Generator>, Without<Monster>, Without<Player>)>,
    mut monsters: Query<
        (Entity, &mut Position, &mut Cooldown, &Monster),
        (Without<Player>, Without<Solid>, Without<Generator>),
    >,
    mut players: Query<
        (&Position, &mut Health, Has<Shielded>),
        (With<Player>, Without<Downed>, Without<Monster>),
    >,
) {
    let dt = time.delta_secs();
    // Obstacles fixes pour tous les monstres, puis les corps des monstres entre eux.
    let fixed: Vec<(Vec2, Vec2)> = doors
        .iter()
        .map(|(p, s)| (p.0, s.0))
        .chain(generators.iter().map(|(p, h)| (p.0, Vec2::splat(h.0))))
        .chain(players.iter().map(|(p, ..)| (p.0, Vec2::splat(HERO_HALF_SIZE))))
        .collect();
    let bodies: Vec<(Entity, Vec2, f32)> = monsters
        .iter()
        .map(|(e, p, _, m)| (e, p.0, m.0.stats().half_size))
        .collect();
    for (entity, mut pos, mut cooldown, monster) in &mut monsters {
        let stats = monster.0.stats();
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        let target = players
            .iter_mut()
            .map(|(p, h, shielded)| (p.0.distance(pos.0), p.0, h, shielded))
            .filter(|(d, ..)| *d <= stats.sight)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((dist, target_pos, mut health, shielded)) = target else {
            continue;
        };
        if dist <= stats.half_size + HERO_HALF_SIZE + MONSTER_REACH_SLACK {
            if cooldown.0 == 0.0 {
                let factor = if shielded { BULWARK_DAMAGE_FACTOR } else { 1.0 };
                health.current -= stats.damage * factor;
                cooldown.0 = stats.cooldown;
            }
        } else {
            let dir = (target_pos - pos.0).normalize_or_zero();
            let mut solids = fixed.clone();
            solids.extend(
                bodies
                    .iter()
                    .filter(|(e, ..)| *e != entity)
                    .map(|(_, p, h)| (*p, Vec2::splat(*h))),
            );
            pos.0 = room.move_and_collide_solids(
                pos.0,
                dir * stats.speed * dt,
                stats.half_size,
                &solids,
            );
        }
    }
}

fn spawn_pickup(commands: &mut Commands, kind: PickupKind, pos: Vec2, rng: &mut Rng) {
    let jitter = Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * 12.0;
    commands.spawn((Replicated, Pickup(kind), Position(pos + jitter)));
}

/// Retire ce qui est mort (monstres, générateurs) et laisse tomber du butin.
fn reap_dead(
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    dead: Query<
        (Entity, &Health, &Position, Has<Generator>, Has<Monster>),
        (With<Hurtbox>, Without<Player>),
    >,
    mut quests: Query<&mut Quest>,
) {
    for (entity, health, pos, is_generator, is_monster) in &dead {
        if health.current > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        if is_monster || is_generator {
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Hunt && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
        if is_generator {
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.6) });
            for _ in 0..3 {
                spawn_pickup(&mut commands, PickupKind::Coin, pos.0, &mut rng);
            }
            if rng.next_f32() < 0.5 {
                spawn_pickup(&mut commands, PickupKind::Key, pos.0, &mut rng);
            }
        } else {
            let roll = rng.next_f32();
            let kind = if roll < 0.45 {
                Some(PickupKind::Coin)
            } else if roll < 0.55 {
                Some(PickupKind::Food)
            } else if roll < 0.58 {
                Some(PickupKind::Key)
            } else {
                None
            };
            if let Some(kind) = kind {
                spawn_pickup(&mut commands, kind, pos.0, &mut rng);
            }
        }
    }
}

/// Ramasse les reliques de la quête de récupération.
fn collect_relics(
    mut commands: Commands,
    players: Query<&Position, (With<Player>, Without<Downed>)>,
    relics: Query<(Entity, &Position), (With<Relic>, Without<Player>)>,
    mut quests: Query<&mut Quest>,
) {
    for (entity, relic_pos) in &relics {
        if players.iter().any(|pos| pos.0.distance(relic_pos.0) <= PICKUP_RANGE) {
            commands.entity(entity).despawn();
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Retrieve && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
    }
}

/// Un héros qui reste près d'un prisonnier le délivre.
fn rescue_captives(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<&Position, (With<Player>, Without<Downed>)>,
    mut captives: Query<(Entity, &Position, &mut RescueHold), (With<Captive>, Without<Player>)>,
    mut quests: Query<&mut Quest>,
) {
    for (entity, captive_pos, mut hold) in &mut captives {
        if players.iter().any(|pos| pos.0.distance(captive_pos.0) <= RESCUE_RANGE) {
            hold.0 += time.delta_secs();
        } else {
            hold.0 = (hold.0 - time.delta_secs()).max(0.0);
        }
        if hold.0 >= RESCUE_TIME {
            commands.entity(entity).despawn();
            for mut quest in &mut quests {
                if quest.kind == QuestKind::Rescue && !quest.is_done() {
                    quest.progress += 1;
                }
            }
        }
    }
}

/// L'objectif est rempli: le portail de sortie apparaît dans la salle de sortie, avec une secousse.
fn open_portal(
    mut commands: Commands,
    layout: Res<Layout>,
    mut quests: Query<&mut Quest>,
    portals: Query<(), With<Portal>>,
    remaining: Query<(), Or<(With<Monster>, With<Generator>)>>,
) {
    // Plus rien à combattre: la chasse est réputée accomplie, pour ne jamais bloquer la sortie.
    if remaining.is_empty() {
        for mut quest in &mut quests {
            if quest.kind == QuestKind::Hunt && !quest.is_done() {
                quest.progress = quest.goal;
            }
        }
    }
    if quests.iter_mut().any(|quest| quest.is_changed() && quest.is_done()) && portals.is_empty() {
        let exit = &layout.rooms[layout.exit];
        commands.spawn((Replicated, Portal, Position(tile_to_world(exit.center()))));
        commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.5) });
    }
}

fn collect_pickups(
    mut commands: Commands,
    mut players: Query<
        (&Position, &mut Health, &mut Gold, &mut Keys),
        (With<Player>, Without<Downed>),
    >,
    pickups: Query<(Entity, &Position, &Pickup), Without<Player>>,
) {
    for (entity, pickup_pos, pickup) in &pickups {
        for (pos, mut health, mut gold, mut keys) in &mut players {
            if pos.0.distance(pickup_pos.0) > PICKUP_RANGE {
                continue;
            }
            match pickup.0 {
                PickupKind::Coin => gold.0 += 1,
                PickupKind::Key => keys.0 += 1,
                PickupKind::Food => {
                    // La nourriture reste au sol si le héros est déjà en pleine forme.
                    if health.current >= health.max {
                        continue;
                    }
                    health.current = (health.current + FOOD_HEAL).min(health.max);
                }
            }
            commands.entity(entity).despawn();
            break;
        }
    }
}

fn down_players(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut Health, &mut MoveDir, &mut Attacking, &mut Interacting),
        (With<Player>, Without<Downed>),
    >,
) {
    for (entity, mut health, mut dir, mut attacking, mut interacting) in &mut players {
        if health.current <= 0.0 {
            health.current = 0.0;
            dir.0 = Vec2::ZERO;
            attacking.0 = false;
            interacting.0 = false;
            commands.entity(entity).insert(Downed);
        }
    }
}

/// Un allié debout à proximité relève un héros à terre au bout de quelques secondes.
fn revive_players(
    mut commands: Commands,
    time: Res<Time>,
    mut downed: Query<(Entity, &Position, &mut Revive, &mut Health), With<Downed>>,
    standing: Query<&Position, (With<Player>, Without<Downed>)>,
) {
    for (entity, pos, mut revive, mut health) in &mut downed {
        let helped = standing.iter().any(|p| p.0.distance(pos.0) <= REVIVE_RANGE);
        if helped {
            revive.0 += time.delta_secs();
        } else {
            revive.0 = (revive.0 - time.delta_secs()).max(0.0);
        }
        if revive.0 >= REVIVE_TIME {
            revive.0 = 0.0;
            health.current = health.max * 0.5;
            commands.entity(entity).remove::<Downed>();
        }
    }
}

/// Tous les héros à terre: la salle est réinitialisée.
fn check_wipe(
    mut commands: Commands,
    layout: Res<Layout>,
    mut players: Query<(Entity, &mut Position, &mut Health, Has<Downed>), With<Player>>,
    mut generators: Query<&mut Health, (With<Generator>, Without<Player>)>,
    transient: Query<Entity, Or<(With<Monster>, With<Projectile>, With<Pickup>, With<Zone>, With<Turret>)>>,
) {
    if players.is_empty() || !players.iter().all(|(.., downed)| downed) {
        return;
    }
    info!("all heroes down, resetting the room");
    for (slot, (entity, mut pos, mut health, _)) in players.iter_mut().enumerate() {
        pos.0 = spawn_point(slot, &layout);
        health.current = health.max;
        commands.entity(entity).remove::<Downed>();
    }
    for mut health in &mut generators {
        health.current = health.max;
    }
    for entity in &transient {
        commands.entity(entity).despawn();
    }
}

/// Point d'apparition d'un héros dans la salle de départ: quatre par rangée.
fn spawn_point(slot: usize, layout: &Layout) -> Vec2 {
    let center = tile_to_world(layout.rooms[layout.start].center());
    center + Vec2::new((slot % 4) as f32 * 22.0 - 33.0, (slot / 4) as f32 * 24.0)
}

/// Centre d'une tuile en pixels; la carte est centrée sur l'origine, comme `Room::tile_center`.
fn tile_to_world(tile: UVec2) -> Vec2 {
    (tile.as_vec2() + Vec2::splat(0.5)) * TILE_SIZE - (GRID * CELL).as_vec2() * TILE_SIZE / 2.0
}

/// Un point au hasard sur le sol d'une salle, à `margin` tuiles des bords.
fn random_point(room: &Room, info: &RoomInfo, margin: u32, rng: &mut Rng) -> Vec2 {
    for _ in 0..12 {
        let x = info.min.x + margin + (rng.next_f32() * (info.size().x - 2 * margin) as f32) as u32;
        let y = info.min.y + margin + (rng.next_f32() * (info.size().y - 2 * margin) as f32) as u32;
        if room.tile(x as i32, y as i32) == Tile::Floor {
            return tile_to_world(UVec2::new(x, y));
        }
    }
    tile_to_world(info.center())
}

fn start_first_expedition(mut commands: Commands, mut rng: ResMut<Rng>) {
    let seed = rng.next_u64();
    let dungeon = Dungeon::generate(seed);
    commands.insert_resource(Level(1));
    populate(&mut commands, &dungeon, seed, 1, true, &mut rng);
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

/// Numéro du niveau en cours.
#[derive(Resource)]
struct Level(u32);

/// Temps pendant lequel un héros est resté près d'un prisonnier.
#[derive(Component)]
struct RescueHold(f32);

/// Secondes à passer près d'un prisonnier pour le délivrer, et distance de rencontre.
const RESCUE_TIME: f32 = 1.2;
const RESCUE_RANGE: f32 = 22.0;

/// Remplit un nouveau château: générateurs, énigme, pièges, trésor, portes, objectif de niveau et dialogues.
fn populate(commands: &mut Commands, dungeon: &Dungeon, seed: u64, level: u32, first: bool, rng: &mut Rng) {
    let (room, layout) = (&dungeon.room, &dungeon.layout);
    let region = Region::ALL[(rng.next_u64() % Region::ALL.len() as u64) as usize];
    info!("new expedition: level {level}, seed {seed}, region {region:?}");
    commands.spawn((Replicated, Expedition { seed, region, level }));

    for info in &layout.rooms {
        match info.kind {
            RoomKind::Combat => {
                let area = info.size().x * info.size().y;
                let count = if area > 130 && rng.next_f32() < 0.6 { 2 } else { 1 };
                for _ in 0..count {
                    commands.spawn((
                        Replicated,
                        Generator,
                        Health::full(GENERATOR_HEALTH),
                        Position(random_point(room, info, 2, rng)),
                        Hurtbox(GENERATOR_HALF_SIZE),
                        SpawnTimer(GENERATOR_INTERVAL),
                    ));
                }
                if rng.next_f32() < 0.4 {
                    for _ in 0..3 {
                        commands.spawn((
                            Replicated,
                            Trap { armed: true },
                            Position(random_point(room, info, 2, rng)),
                            Hold::default(),
                            TrapCooldown::default(),
                        ));
                    }
                }
            }
            RoomKind::Treasure => {
                let center = tile_to_world(info.center());
                for offset in [(-20.0, -12.0), (-20.0, 12.0), (0.0, -12.0), (0.0, 12.0), (20.0, 0.0)] {
                    spawn_pickup(commands, PickupKind::Coin, center + Vec2::from(offset), rng);
                }
                spawn_pickup(commands, PickupKind::Food, center, rng);
            }
            RoomKind::Start | RoomKind::Exit => {}
        }
    }

    // Les trois premières étapes de l'énigme, dans trois salles de combat.
    for (kind, room_index) in [InteractKind::Lever, InteractKind::Rune, InteractKind::Mechanism]
        .into_iter()
        .zip(layout.puzzle_rooms)
    {
        commands.spawn((
            Replicated,
            Interactable::new(kind),
            Position(random_point(room, &layout.rooms[room_index], 2, rng)),
            Hold::default(),
        ));
    }

    // Portes: la royale devant la sortie, une porte à clé devant le trésor.
    for conn in &layout.connections {
        let Some(kind) = conn.door else {
            continue;
        };
        let (a, b) = if conn.door_vertical {
            (conn.door_min, conn.door_min + UVec2::Y)
        } else {
            (conn.door_min, conn.door_min + UVec2::X)
        };
        let center = (tile_to_world(a) + tile_to_world(b)) / 2.0;
        let half = if conn.door_vertical {
            Vec2::new(TILE_SIZE / 2.0, TILE_SIZE)
        } else {
            Vec2::new(TILE_SIZE, TILE_SIZE / 2.0)
        };
        let door = commands
            .spawn((
                Replicated,
                Door { kind, vertical: conn.door_vertical },
                Position(center),
                Solid(half),
            ))
            .id();
        let node = match kind {
            DoorKind::Royal => {
                commands
                    .entity(door)
                    .insert((Interactable::new(InteractKind::Push), Hold::default()));
                "royal_door"
            }
            DoorKind::Locked => "alcove",
        };
        commands.spawn((
            Position(center),
            DialogueTrigger { node, radius: 70.0, delay: 0.3, fired: false },
        ));
    }

    // L'objectif du niveau. Le portail de sortie n'apparaît qu'une fois celui-ci rempli (voir `open_portal`).
    let (kind, goal) = QuestKind::for_level(level);
    commands.spawn((Replicated, Quest { kind, goal, progress: 0 }));
    let fights: Vec<&RoomInfo> = layout.rooms.iter().filter(|r| r.kind == RoomKind::Combat).collect();
    if !fights.is_empty() {
        for i in 0..goal {
            let info = fights[(rng.next_u64() % fights.len() as u64) as usize];
            let pos = random_point(room, info, 2, rng);
            match kind {
                QuestKind::Retrieve => {
                    commands.spawn((Replicated, Relic, Position(pos)));
                }
                QuestKind::Rescue => {
                    commands.spawn((Replicated, Captive, Position(pos), RescueHold(0.0)));
                }
                QuestKind::Hunt => {
                    let _ = i;
                }
            }
        }
    }

    if first {
        commands.spawn((
            Position(tile_to_world(layout.rooms[layout.start].center())),
            DialogueTrigger { node: "intro", radius: 200.0, delay: 3.0, fired: false },
        ));
    }
}

/// Un joueur demande de repartir de zéro depuis l'écran titre: nouveau château, héros ramenés au départ, or et clés
/// remis à zéro.
fn new_game(
    _request: On<FromClient<NewGame>>,
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    mut session: ResMut<DialogueSession>,
    mut level: ResMut<Level>,
    mut players: Query<(Entity, &mut Position, &mut Health, &mut Gold, &mut Keys), With<Player>>,
    world: Query<Entity, (With<Replicated>, Without<Player>)>,
    triggers: Query<Entity, With<DialogueTrigger>>,
) {
    for entity in world.iter().chain(triggers.iter()) {
        commands.entity(entity).despawn();
    }
    *session = DialogueSession::default();
    let seed = rng.next_u64();
    let dungeon = Dungeon::generate(seed);
    level.0 = 1;
    populate(&mut commands, &dungeon, seed, 1, true, &mut rng);
    for (slot, (entity, mut pos, mut health, mut gold, mut keys)) in players.iter_mut().enumerate() {
        pos.0 = spawn_point(slot, &dungeon.layout);
        health.current = health.max;
        gold.0 = 0;
        keys.0 = 0;
        commands.entity(entity).remove::<Downed>();
    }
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

/// Quand tous les héros debout sont dans le portail, un nouveau château est généré.
fn run_portal(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<DialogueSession>,
    mut level: ResMut<Level>,
    mut countdown: Local<f32>,
    portals: Query<&Position, (With<Portal>, Without<Player>)>,
    mut players: Query<(Entity, &mut Position, &mut Health, Has<Downed>, Has<Controlled>), With<Player>>,
    world: Query<Entity, (With<Replicated>, Without<Player>)>,
    triggers: Query<Entity, With<DialogueTrigger>>,
) {
    let Ok(portal) = portals.single() else {
        return;
    };
    // Seuls les héros dirigés comptent: les compagnons sont ramenés avec eux.
    let standing: Vec<_> = players
        .iter()
        .filter(|(_, _, _, downed, controlled)| !downed && *controlled)
        .collect();
    let all_in = !standing.is_empty()
        && standing.iter().all(|(_, p, ..)| p.0.distance(portal.0) <= PORTAL_RADIUS);
    if !all_in {
        *countdown = 0.0;
        return;
    }
    *countdown += time.delta_secs();
    if *countdown < PORTAL_DELAY {
        return;
    }
    *countdown = 0.0;

    // On efface l'ancien château (tout ce qui est répliqué sauf les héros) et on en génère un autre.
    for entity in world.iter().chain(triggers.iter()) {
        commands.entity(entity).despawn();
    }
    *session = DialogueSession::default();
    let seed = rng.next_u64();
    let dungeon = Dungeon::generate(seed);
    level.0 += 1;
    populate(&mut commands, &dungeon, seed, level.0, false, &mut rng);
    for (slot, (entity, mut pos, mut health, ..)) in players.iter_mut().enumerate() {
        pos.0 = spawn_point(slot, &dungeon.layout);
        health.current = health.max;
        commands.entity(entity).remove::<Downed>();
    }
    commands.insert_resource(dungeon.room);
    commands.insert_resource(dungeon.layout);
}

fn regen_mana(time: Res<Time>, mut players: Query<(&mut Mana, &HeroClass), (With<Player>, Without<Downed>)>) {
    for (mut mana, class) in &mut players {
        mana.current = (mana.current + class.stats().mana_regen * time.delta_secs()).min(mana.max);
    }
}

/// Déclenche le pouvoir spécial demandé, si le mana et la recharge le permettent.
fn player_specials(
    mut commands: Commands,
    time: Res<Time>,
    mut players: Query<
        (Entity, &Position, &Facing, &HeroClass, &mut Mana, &mut SpecialInput, &mut SpecialCooldown, &mut Strikes),
        (With<Player>, Without<Downed>),
    >,
    allies: Query<(Entity, &Position), (With<Player>, Without<Downed>)>,
    turrets: Query<(Entity, &TurretOwner)>,
) {
    for (entity, pos, facing, class, mut mana, mut input, mut cooldown, mut strikes) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if !std::mem::take(&mut input.queued) {
            continue;
        }
        let stats = class.stats();
        if cooldown.0 > 0.0 || mana.current < stats.special_cost {
            continue;
        }
        mana.current -= stats.special_cost;
        cooldown.0 = stats.special_cooldown;
        strikes.0 = strikes.0.wrapping_add(1);
        match class {
            HeroClass::Knight => {
                for (ally, ally_pos) in &allies {
                    if ally_pos.0.distance(pos.0) <= BULWARK_RADIUS {
                        commands.entity(ally).insert((Shielded, ShieldTimer(BULWARK_DURATION)));
                    }
                }
            }
            HeroClass::Sorcerer => {
                commands.spawn((
                    Replicated,
                    Zone { radius: ARCANE_RADIUS, class: *class },
                    Position(pos.0 + facing.0 * ARCANE_OFFSET),
                    ZoneLife(0.4),
                    // Un seul coup, au premier instant.
                    ZoneDamage { damage: ARCANE_DAMAGE, interval: 10.0, timer: 0.0 },
                ));
            }
            HeroClass::Ranger => {
                commands.spawn((
                    Replicated,
                    Zone { radius: RAIN_RADIUS, class: *class },
                    Position(pos.0 + facing.0 * RAIN_OFFSET),
                    ZoneLife(RAIN_DURATION),
                    ZoneDamage { damage: RAIN_DAMAGE, interval: RAIN_INTERVAL, timer: 0.0 },
                ));
            }
            HeroClass::Forgebraise => {
                // Une seule tourelle par Forgebraise.
                for (turret, owner) in &turrets {
                    if owner.0 == entity {
                        commands.entity(turret).despawn();
                    }
                }
                commands.spawn((
                    Replicated,
                    Turret,
                    TurretOwner(entity),
                    Position(pos.0 + facing.0 * 14.0),
                    TurretLife(TURRET_LIFETIME),
                    Cooldown::default(),
                ));
            }
        }
    }
}

fn tick_shields(
    mut commands: Commands,
    time: Res<Time>,
    mut shields: Query<(Entity, &mut ShieldTimer)>,
) {
    for (entity, mut timer) in &mut shields {
        timer.0 -= time.delta_secs();
        if timer.0 <= 0.0 {
            commands.entity(entity).remove::<(Shielded, ShieldTimer)>();
        }
    }
}

fn run_zones(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &Position, &Zone, &mut ZoneLife, &mut ZoneDamage)>,
    mut targets: Query<(&Position, &mut Health, &Hurtbox), (Without<Player>, Without<Zone>)>,
) {
    let dt = time.delta_secs();
    for (entity, pos, zone, mut life, mut damage) in &mut zones {
        damage.timer -= dt;
        if damage.timer <= 0.0 {
            damage.timer += damage.interval;
            for (target_pos, mut health, hurtbox) in &mut targets {
                if target_pos.0.distance(pos.0) <= zone.radius + hurtbox.0 {
                    health.current -= damage.damage;
                }
            }
        }
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Les tourelles tirent sur la cible la plus proche à portée.
fn run_turrets(
    mut commands: Commands,
    time: Res<Time>,
    mut turrets: Query<(Entity, &Position, &mut Cooldown, &mut TurretLife), With<Turret>>,
    targets: Query<&Position, (With<Hurtbox>, Without<Player>, Without<Turret>)>,
) {
    let dt = time.delta_secs();
    for (entity, pos, mut cooldown, mut life) in &mut turrets {
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        if cooldown.0 > 0.0 {
            continue;
        }
        let nearest = targets
            .iter()
            .map(|t| t.0)
            .filter(|t| t.distance(pos.0) <= TURRET_RANGE)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        if let Some(target) = nearest {
            let dir = (target - pos.0).normalize_or_zero();
            cooldown.0 = TURRET_COOLDOWN;
            commands.spawn((
                Replicated,
                Projectile { class: HeroClass::Forgebraise, dir },
                Position(pos.0 + dir * 8.0),
                Velocity(dir * TURRET_SHOT_SPEED),
                Damage(TURRET_DAMAGE),
                Lifetime(TURRET_RANGE / TURRET_SHOT_SPEED + 0.2),
            ));
        }
    }
}

/// Maintenir la touche d'action avec la bonne classe fait avancer l'objet visé.
/// La porte royale n'obéit au Chevalier qu'une fois les trois autres étapes terminées.
fn run_interactions(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<(&Position, &HeroClass, &Interacting), (With<Player>, Without<Downed>)>,
    mut things: Query<(Entity, &Position, &mut Interactable, &mut Hold), Without<Player>>,
) {
    let dt = time.delta_secs();
    let puzzle_ready = things
        .iter()
        .filter(|(_, _, i, _)| i.kind != InteractKind::Push)
        .all(|(_, _, i, _)| i.done);
    for (entity, pos, mut interactable, mut hold) in &mut things {
        if interactable.done {
            continue;
        }
        let kind = interactable.kind;
        let unlocked = kind != InteractKind::Push || puzzle_ready;
        let active = unlocked
            && players.iter().any(|(p, class, interacting)| {
                interacting.0 && *class == kind.class() && p.0.distance(pos.0) <= INTERACT_RANGE + 4.0
            });
        if active {
            hold.0 += dt;
        } else {
            hold.0 = (hold.0 - dt * 2.0).max(0.0);
        }
        let progress = (hold.0 / kind.hold_time()).min(1.0);
        if (interactable.progress - progress).abs() > 0.01 {
            interactable.progress = progress;
        }
        if hold.0 >= kind.hold_time() {
            interactable.done = true;
            interactable.progress = 1.0;
            info!("{kind:?} done");
            // La porte royale qui cède secoue plus fort qu'une étape intermédiaire.
            let strength = if kind == InteractKind::Push { 0.85 } else { 0.5 };
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(strength) });
            if kind == InteractKind::Push {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Les pièges blessent qui marche dessus; le Rôdeur peut les désamorcer en maintenant la touche d'action.
fn run_traps(
    time: Res<Time>,
    mut traps: Query<(&Position, &mut Trap, &mut Hold, &mut TrapCooldown), Without<Player>>,
    mut players: Query<(&Position, &HeroClass, &Interacting, &mut Health), (With<Player>, Without<Downed>)>,
) {
    let dt = time.delta_secs();
    for (pos, mut trap, mut hold, mut cooldown) in &mut traps {
        if !trap.armed {
            continue;
        }
        cooldown.0 = (cooldown.0 - dt).max(0.0);
        let disarming = players.iter().any(|(p, class, interacting, _)| {
            interacting.0 && *class == HeroClass::Ranger && p.0.distance(pos.0) <= INTERACT_RANGE
        });
        if disarming {
            hold.0 += dt;
            if hold.0 >= TRAP_DISARM_TIME {
                trap.armed = false;
                continue;
            }
        } else {
            hold.0 = (hold.0 - dt).max(0.0);
        }
        if cooldown.0 > 0.0 {
            continue;
        }
        for (p, _, _, mut health) in &mut players {
            if p.0.distance(pos.0) <= TRAP_TRIGGER_RADIUS {
                health.current -= TRAP_DAMAGE;
                cooldown.0 = TRAP_COOLDOWN;
            }
        }
    }
}

/// Une porte à clé s'ouvre quand un héros de l'équipe est devant et que l'équipe a une clé (une clé est consommée).
fn open_locked_doors(
    mut commands: Commands,
    doors: Query<(Entity, &Position, &Door), Without<Player>>,
    mut heroes: Query<(&Position, &PartyId, &mut Keys, Has<Downed>), With<Player>>,
) {
    for (entity, pos, door) in &doors {
        if door.kind != DoorKind::Locked {
            continue;
        }
        let near = heroes
            .iter()
            .find(|(p, _, _, downed)| !downed && p.0.distance(pos.0) <= DOOR_OPEN_RANGE + TILE_SIZE / 2.0)
            .map(|(_, party, ..)| *party);
        let Some(party) = near else {
            continue;
        };
        if let Some((_, _, mut keys, _)) = heroes
            .iter_mut()
            .find(|(_, id, keys, _)| **id == party && keys.0 > 0)
        {
            keys.0 -= 1;
            commands.entity(entity).despawn();
            commands.server_trigger(ToClients { targets: SendTargets::All, message: ScreenShake(0.3) });
        }
    }
}

/// Un héros qui reste dans une zone de dialogue lance la scène, si aucune autre n'est en cours.
fn run_dialogue_triggers(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<DialogueSession>,
    mut triggers: Query<(&Position, &mut DialogueTrigger)>,
    players: Query<&Position, (With<Player>, Without<Downed>, Without<DialogueTrigger>)>,
) {
    if session.active.is_some() {
        return;
    }
    for (pos, mut trigger) in &mut triggers {
        if trigger.fired {
            continue;
        }
        if !players.iter().any(|p| p.0.distance(pos.0) <= trigger.radius) {
            continue;
        }
        trigger.delay -= time.delta_secs();
        if trigger.delay > 0.0 {
            continue;
        }
        trigger.fired = true;
        *session = DialogueSession {
            active: Some(trigger.node.to_string()),
            ..default()
        };
        info!("dialogue: {}", trigger.node);
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: DialogueBegin(trigger.node.to_string()),
        });
        return;
    }
}

fn tick_dialogue(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<DialogueSession>,
    clients: Query<(), With<ActiveHero>>,
) {
    if session.active.is_none() {
        return;
    }
    let dt = time.delta_secs();
    session.advance_cooldown = (session.advance_cooldown - dt).max(0.0);
    session.idle += dt;
    if session.idle > DIALOGUE_IDLE_TIMEOUT {
        warn!("dialogue abandoned after a long silence");
        *session = DialogueSession::default();
        return;
    }
    if !session.votes.is_empty() {
        session.vote_timer += dt;
        if session.vote_timer >= DIALOGUE_VOTE_TIMEOUT || session.votes.len() >= clients.iter().count() {
            resolve_votes(&mut commands, &mut session);
        }
    }
}

/// Le choix le plus voté l'emporte; en cas d'égalité, le premier de la liste.
fn resolve_votes(commands: &mut Commands, session: &mut DialogueSession) {
    let mut tally: HashMap<u32, usize> = HashMap::new();
    for choice in session.votes.values() {
        *tally.entry(*choice).or_default() += 1;
    }
    let winner = tally
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(choice, _)| choice)
        .unwrap_or(0);
    session.votes.clear();
    session.vote_timer = 0.0;
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: DialogueSelect(winner),
    });
}

fn dialogue_advance(
    _request: On<FromClient<DialogueRequestAdvance>>,
    mut commands: Commands,
    mut session: ResMut<DialogueSession>,
) {
    if session.active.is_none() || session.advance_cooldown > 0.0 {
        return;
    }
    session.advance_cooldown = DIALOGUE_ADVANCE_COOLDOWN;
    session.idle = 0.0;
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: DialogueAdvance,
    });
}

fn dialogue_vote(vote: On<FromClient<DialogueVote>>, mut session: ResMut<DialogueSession>) {
    let Some(client) = vote.client_id.entity() else {
        return;
    };
    if session.active.is_some() {
        session.idle = 0.0;
        session.votes.insert(client, vote.0);
    }
}

fn dialogue_finished(_done: On<FromClient<DialogueFinished>>, mut session: ResMut<DialogueSession>) {
    *session = DialogueSession::default();
}

fn spawn_monster(commands: &mut Commands, kind: MonsterKind, pos: Vec2) {
    let stats = kind.stats();
    commands.spawn((
        Replicated,
        Monster(kind),
        Health::full(stats.max_hp),
        Position(pos),
        Hurtbox(stats.half_size),
        Cooldown::default(),
    ));
}

/// Direction à prendre pour aller de `pos` à `goal`: tout droit si la voie est libre, sinon en suivant
/// un itinéraire calculé à travers la carte (murs, couloirs, portes fermées).
fn steer(room: &Room, solids: &[(Vec2, Vec2)], route: &mut Route, pos: Vec2, goal: Vec2, dt: f32) -> Vec2 {
    if room.segment_clear(pos, goal, HERO_HALF_SIZE) {
        route.waypoints.clear();
        return (goal - pos).normalize_or_zero();
    }
    route.age += dt;
    if route.waypoints.is_empty() || route.goal.distance(goal) > ROUTE_GOAL_DRIFT || route.age > ROUTE_MAX_AGE {
        route.goal = goal;
        route.age = 0.0;
        route.waypoints = room.find_path(pos, goal, solids).unwrap_or_default();
    }
    // On saute les points déjà atteints, ou masqués par un raccourci direct.
    while let Some(first) = route.waypoints.first() {
        let reached = first.distance(pos) < 5.0;
        let shortcut = route.waypoints.len() >= 2 && room.segment_clear(pos, route.waypoints[1], HERO_HALF_SIZE);
        if reached || shortcut {
            route.waypoints.remove(0);
        } else {
            break;
        }
    }
    match route.waypoints.first() {
        Some(next) => (*next - pos).normalize_or_zero(),
        // Pas d'itinéraire (but inaccessible): on essaie quand même d'en approcher.
        None => (goal - pos).normalize_or_zero(),
    }
}

/// Les compagnons (héros non dirigés) suivent le héros dirigé, relèvent les alliés à terre et combattent seuls,
/// avec leur seule arme de base: les pouvoirs spéciaux restent réservés au héros que dirige le joueur.
fn hero_ai(
    time: Res<Time>,
    room: Res<Room>,
    doors: Query<(&Position, &Solid), Without<Player>>,
    mut heroes: Query<
        (
            &PartyId,
            &HeroClass,
            &Position,
            &mut MoveDir,
            &mut Facing,
            &mut Attacking,
            &mut Route,
        ),
        (With<Player>, Without<Controlled>, Without<Downed>),
    >,
    leaders: Query<(&PartyId, &Position), (With<Controlled>, Without<Downed>)>,
    downed: Query<(&PartyId, &Position), (With<Player>, With<Downed>)>,
    enemies: Query<&Position, (With<Hurtbox>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    let solids: Vec<_> = doors.iter().map(|(p, s)| (p.0, s.0)).collect();
    for (party, class, pos, mut dir, mut facing, mut attacking, mut route) in &mut heroes {
        dir.0 = Vec2::ZERO;
        attacking.0 = false;
        let leader = leaders.iter().find(|(id, _)| *id == party).map(|(_, p)| p.0);

        // Un ennemi n'est engagé que s'il est visible (pas derrière un mur) et proche du groupe.
        let nearest = enemies
            .iter()
            .map(|e| (e.0.distance(pos.0), e.0))
            .filter(|(d, e)| {
                *d <= AI_AGGRO
                    && leader.is_none_or(|l| l.distance(*e) <= AI_GROUP_RADIUS)
                    && room.segment_clear(pos.0, *e, 1.0)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));

        if let Some((distance, target)) = nearest {
            let to = (target - pos.0).normalize_or_zero();
            facing.0 = to;
            match class.stats().attack {
                Attack::Melee { range, .. } => {
                    if distance > range * 0.7 {
                        dir.0 = to;
                    }
                    attacking.0 = distance <= range + 6.0;
                }
                Attack::Shot { .. } => {
                    // Les tireurs gardent leurs distances.
                    attacking.0 = distance <= AI_SHOT_REACH;
                    if distance < 45.0 {
                        dir.0 = -to;
                    } else if distance > 100.0 {
                        dir.0 = to;
                    }
                }
            }
            continue;
        }

        // Pas d'ennemi en vue: aller relever un allié à terre, sinon suivre le chef.
        let ally = downed
            .iter()
            .filter(|(id, _)| *id == party)
            .map(|(_, p)| p.0)
            .min_by(|a, b| a.distance(pos.0).total_cmp(&b.distance(pos.0)));
        if let Some(ally) = ally {
            if ally.distance(pos.0) > REVIVE_RANGE * 0.5 {
                dir.0 = steer(&room, &solids, &mut route, pos.0, ally, dt);
            }
        } else if let Some(leader) = leader {
            // Dans un couloir étroit la place prévue en formation tombe dans un mur: on suit alors le chef de près.
            let slot = leader + formation_offset(*class);
            let free = !room.overlaps_wall(slot, HERO_HALF_SIZE) && room.segment_clear(leader, slot, HERO_HALF_SIZE);
            let (goal, stop) = if free { (slot, 10.0) } else { (leader, 16.0) };
            if goal.distance(pos.0) > stop {
                dir.0 = steer(&room, &solids, &mut route, pos.0, goal, dt);
            } else {
                route.waypoints.clear();
            }
        }
        if let Some(d) = dir.0.try_normalize() {
            facing.0 = d;
        }
    }
}

/// Place de chaque compagnon autour du héros dirigé.
fn formation_offset(class: HeroClass) -> Vec2 {
    match class {
        HeroClass::Knight => Vec2::new(-18.0, -14.0),
        HeroClass::Sorcerer => Vec2::new(-30.0, 10.0),
        HeroClass::Ranger => Vec2::new(-18.0, 22.0),
        HeroClass::Forgebraise => Vec2::new(-34.0, -10.0),
    }
}

/// Un compagnon qui s'est perdu (coincé derrière un angle, ou trop loin) est ramené auprès du chef.
fn leash_followers(
    mut followers: Query<(&PartyId, &mut Position), (With<Player>, Without<Controlled>, Without<Downed>)>,
    leaders: Query<(&PartyId, &Position), (With<Controlled>, Without<Downed>)>,
) {
    for (party, mut pos) in &mut followers {
        if let Some((_, leader)) = leaders.iter().find(|(id, _)| *id == party) {
            if leader.0.distance(pos.0) > AI_LEASH {
                pos.0 = leader.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un compagnon seul, avec les vraies collisions, doit atteindre chaque salle du château.
    #[test]
    fn follower_reaches_every_room() {
        let dt = 1.0 / 64.0;
        let speed = 75.0;
        for seed in 0..25 {
            let dungeon = Dungeon::generate(seed);
            let (room, layout) = (&dungeon.room, &dungeon.layout);
            for target in 0..layout.rooms.len() {
                let goal = tile_to_world(layout.rooms[target].center());
                let mut pos = tile_to_world(layout.rooms[layout.start].center());
                let mut route = Route::default();
                let mut arrived = false;
                for _ in 0..(60 * 64) {
                    if pos.distance(goal) < 12.0 {
                        arrived = true;
                        break;
                    }
                    let dir = steer(room, &[], &mut route, pos, goal, dt);
                    pos = room.move_and_collide(pos, dir * speed * dt, HERO_HALF_SIZE);
                }
                assert!(arrived, "seed {seed}: bloqué en route vers la salle {target}, à {pos}");
            }
        }
    }
}
