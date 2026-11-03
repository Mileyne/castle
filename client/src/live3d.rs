//! Les héros, les monstres, les objets, les grands motifs du décor et les sorties de niveau sont de vrais modèles 3D, construits à
//! partir des modèles de `sprite_forge` (voir `merged`): des entités de la scène du décor (`decor3d`), posées sur le sol à leur position
//! réelle. Ils sont donc vus sous l'angle et l'inclinaison choisis, nets à tout cadrage, et masqués par les murs par la profondeur.
//!
//! Chaque modèle est une seule entité qui porte le maillage fusionné de sa pose du moment. Les poses de tous les modèles d'un niveau
//! sont construites à l'avance (voir `prebake`); une pose manquante se construit à la demande.

use bevy::{
    camera::visibility::RenderLayers,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use castle_shared::{ExitKind, HeroClass, Landmark, LandmarkKind, LevelExit, Monster, MonsterKind};
use sprite_forge::{Anim, models};

use crate::decor3d::look::{ActiveLight, LevelLight, Response};
use crate::art::{PoseKind, Prop, SPRITE_SIZE, WalkCycle, exit_row, monster_scale, object};
use crate::merged::{ModelSource, PoseCache, SURFACES, Surface, face_path, load_face, surface_material, weave_image};
use crate::settings::ModelShader;

/// Les modèles de `sprite_forge` montrent 2,4 mètres dans une case de sprite: un mètre vaut `taille de la case / 2,4` unités du monde.
const METERS_PER_SPRITE: f32 = 2.4;
/// Les planches des grands motifs et des sorties montrent 4,8 mètres sur 64 unités du monde.
const LARGE_PLATE_METERS: f32 = 4.8;
const LARGE_PLATE_UNITS: f32 = 64.0;

/// L'extension du matériau standard qui donne aux modèles leur émission propre (voir `assets/shaders/model_glow.wesl`).
#[derive(Asset, AsBindGroup, TypePath, Clone, Debug)]
pub struct GlowExtension {
    /// (part de la couleur du modèle qu'il émet, -, -, -).
    #[uniform(100)]
    glow: Vec4,
}

impl MaterialExtension for GlowExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/model_glow.wesl".into()
    }
}

/// Le matériau des modèles 3D: un matériau standard, avec la capacité d'émettre sa propre lumière.
pub type ModelMaterial = ExtendedMaterial<StandardMaterial, GlowExtension>;

pub struct Live3dPlugin;

impl Plugin for Live3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Caches>()
            .add_plugins(MaterialPlugin::<ModelMaterial>::default())
            .add_observer(release)
            .add_systems(Update, (attach_characters, attach_props, attach_landmarks, attach_exits, animate).chain().after(crate::art::AnimateCharacters))
            .add_systems(Update, retune);
    }
}

/// La teinte d'un modèle (celle que le sprite portait): un héros à terre est translucide, un héros sous régénération pulse en vert,
/// un escalier fermé est grisé... Absente, le modèle garde ses couleurs.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct ModelTint(pub Color);

/// La teinte quantifiée (un octet par canal, les valeurs au-dessus de 1 sont gardées jusqu'à 2): la clé d'un matériau.
type TintKey = [u8; 4];

const WHITE: TintKey = [127, 127, 127, 255];

/// La catégorie d'un modèle pour la lumière: chacune a ses réglages (`LightTuning`): capacité à capter la lumière et à rayonner.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    /// Les héros: ce sont les seuls modèles qui ont un regard répliqué.
    Hero,
    /// Les monstres: ils marchent, mais n'ont pas de regard répliqué.
    Enemy,
    /// Les objets, grands motifs et sorties.
    Object,
}

impl Kind {
    fn response(self, tuning: &LevelLight) -> Response {
        match self {
            Self::Hero => tuning.hero,
            Self::Enemy => tuning.enemy,
            Self::Object => tuning.object,
        }
    }
}

/// Applique à un matériau la réponse de sa catégorie: les couleurs sont multipliées par le gain (ce qui garde les teintes), et le modèle
/// émet une part `glow` de ses propres couleurs. Les matières lumineuses (yeux des loups) et les modèles sans éclairage n'y touchent pas.
fn respond(material: &mut ModelMaterial, surface: Surface, tint: Color, response: Response, lit: bool) {
    if !lit || surface == Surface::Glow {
        return;
    }
    let linear = tint.to_linear();
    material.base.base_color = Color::linear_rgba(linear.red * response.gain, linear.green * response.gain, linear.blue * response.gain, linear.alpha);
    material.extension.glow.x = response.glow;
}

/// La couleur d'une teinte quantifiée.
fn tint_color(tint: TintKey) -> Color {
    let channel = |byte: u8| f32::from(byte) / 127.0;
    Color::srgba(channel(tint[0]), channel(tint[1]), channel(tint[2]), f32::from(tint[3]) / 255.0)
}

fn tint_key(tint: Option<&ModelTint>) -> TintKey {
    let Some(ModelTint(color)) = tint else {
        return WHITE;
    };
    let color = color.to_srgba();
    let byte = |value: f32| (value.clamp(0.0, 2.0) * 127.0).round() as u8;
    [byte(color.red), byte(color.green), byte(color.blue), (color.alpha.clamp(0.0, 1.0) * 255.0).round() as u8]
}

/// Les maillages fusionnés de toutes les poses construites, et les matériaux (un par matière, éclairage et teinte, partagés).
#[derive(Resource, Default)]
pub struct Caches {
    pub poses: PoseCache,
    materials: std::collections::HashMap<(Surface, ModelShader, TintKey, Option<&'static str>, Kind), Handle<ModelMaterial>>,
    /// Les peaux des crânes (une par héros), chargées à la première utilisation.
    faces: std::collections::HashMap<&'static str, Handle<Image>>,
    /// La texture de tissage des vêtements, créée à la première utilisation.
    cloth: Option<Handle<Image>>,
}

/// Un modèle 3D: ce qu'il montre, sa place dans la scène, et le maillage de sa pose du moment.
#[derive(Component)]
pub struct Live3d {
    source: ModelSource,
    root: Entity,
    /// Les entités qui portent les maillages fusionnés (une par matière), et la pose qu'ils montrent.
    bodies: [Option<Entity>; SURFACES],
    shown: Option<(ModelSource, u8, u16)>,
    /// La teinte et le shader du matériau du corps.
    tint: TintKey,
    shader: Option<ModelShader>,
    /// Unités du monde par mètre du modèle.
    scale: f32,
}

fn spawn_root(commands: &mut Commands) -> Entity {
    commands.spawn((Transform::default(), Visibility::default(), RenderLayers::layer(crate::decor3d::LAYER))).id()
}

fn attach(commands: &mut Commands, entity: Entity, source: ModelSource, scale: f32) {
    let root = spawn_root(commands);
    commands.entity(entity).insert(Live3d { source, root, bodies: [None; SURFACES], shown: None, tint: WHITE, shader: None, scale });
}

/// Le modèle d'une classe de héros.
pub fn hero_model(class: HeroClass) -> ModelSource {
    ModelSource::Key(format!("hero:{class:?}"))
}

/// Le modèle d'un monstre.
pub fn monster_model(kind: MonsterKind) -> ModelSource {
    ModelSource::Key(format!("monster:{kind:?}"))
}

/// Le modèle d'un grand motif du décor.
pub fn landmark_model(kind: LandmarkKind) -> Option<ModelSource> {
    LandmarkKind::ALL.iter().position(|candidate| *candidate == kind).map(ModelSource::Landmark)
}

/// Les modèles d'une sortie de niveau, fermée puis ouverte (aucun si le jeu ne la dessine pas).
pub fn exit_models(kind: ExitKind) -> Vec<ModelSource> {
    if kind == ExitKind::Hidden {
        return Vec::new();
    }
    let mut rows = vec![exit_row(kind, false), exit_row(kind, true)];
    rows.dedup();
    rows.into_iter().map(ModelSource::Exit).collect()
}

/// Un héros ou un monstre qui vient d'apparaître reçoit son modèle.
fn attach_characters(mut commands: Commands, fresh: Query<(Entity, Option<&HeroClass>, Option<&Monster>), Added<WalkCycle>>) {
    for (entity, class, monster) in &fresh {
        let (source, size) = match (class, monster) {
            (Some(class), _) => (hero_model(*class), SPRITE_SIZE),
            (None, Some(monster)) => (monster_model(monster.0), SPRITE_SIZE * monster_scale(monster.0)),
            _ => continue,
        };
        attach(&mut commands, entity, source, size / METERS_PER_SPRITE);
    }
}

/// Les clés des modèles d'objets, dans l'ordre des planches (`object::FILES`).
fn prop_keys() -> &'static [String] {
    static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| models().into_iter().filter(|model| model.group == "Objets").map(|model| model.key).collect())
}

/// Les lignes d'objets qui se posent au sol (pas les panneaux de mur, ni les portes et herses, dont l'orientation dépend de leur place).
const GROUND_PROPS: [usize; 14] = [
    object::GENERATOR,
    object::TURRET,
    object::COIN,
    object::FOOD,
    object::KEY,
    object::TRAP_ARMED,
    object::TRAP_OFF,
    object::POTION,
    object::INGREDIENT,
    object::GEAR_PART,
    object::TARGET_DISC,
    object::CAPTIVE,
    object::SAMELIA,
    object::SAMELIA_CAGED,
];

/// Les modèles de tous les objets qui se posent au sol.
pub fn prop_models() -> Vec<ModelSource> {
    GROUND_PROPS.iter().filter_map(|row| prop_model(*row)).collect()
}

/// Le modèle d'une ligne d'objet, quand elle en a un.
pub fn prop_model(row: usize) -> Option<ModelSource> {
    GROUND_PROPS.contains(&row).then(|| prop_keys().get(row).cloned().map(ModelSource::Key)).flatten()
}

/// Un objet qui vient d'apparaître reçoit son modèle, quand il en a un.
fn attach_props(mut commands: Commands, fresh: Query<(Entity, &Prop), (Added<Prop>, Without<Live3d>)>) {
    for (entity, prop) in &fresh {
        if let Some(source) = prop_model(prop.0) {
            attach(&mut commands, entity, source, SPRITE_SIZE / METERS_PER_SPRITE);
        }
    }
}

/// Un grand motif du décor (rocher, colonne, statue, fontaine) qui vient d'apparaître reçoit son modèle.
fn attach_landmarks(mut commands: Commands, fresh: Query<(Entity, &Landmark), (Added<Landmark>, Without<Live3d>)>) {
    for (entity, landmark) in &fresh {
        if let Some(source) = landmark_model(landmark.0) {
            // La fontaine est dessinée plus petite dans sa case.
            let scale = LARGE_PLATE_UNITS * landmark.0.display_scale() / LARGE_PLATE_METERS;
            attach(&mut commands, entity, source, scale);
        }
    }
}

/// Une sortie de niveau qui vient d'apparaître reçoit son modèle (pas celles que le jeu ne dessine pas).
fn attach_exits(mut commands: Commands, fresh: Query<(Entity, &LevelExit), (Added<LevelExit>, Without<Live3d>)>) {
    for (entity, exit) in &fresh {
        if exit.kind != ExitKind::Hidden {
            attach(&mut commands, entity, ModelSource::Exit(exit_row(exit.kind, exit.open)), LARGE_PLATE_UNITS / LARGE_PLATE_METERS);
        }
    }
}

/// Quand les réglages de lumière changent (dans le programme `lumiere`), les matériaux déjà créés les reprennent.
fn retune(active: Res<ActiveLight>, caches: Res<Caches>, mut materials: ResMut<Assets<ModelMaterial>>) {
    if !active.is_changed() {
        return;
    }
    let tuning = &active.0;
    for ((surface, shader, tint, _, kind), handle) in &caches.materials {
        if let Some(mut material) = materials.get_mut(handle) {
            respond(&mut material, *surface, tint_color(*tint), kind.response(tuning), *shader == ModelShader::Lit);
        }
    }
}

/// Un modèle disparu retire sa scène.
fn release(removed: On<Remove<Live3d>>, models: Query<&Live3d>, mut commands: Commands) {
    if let Ok(live) = models.get(removed.entity) {
        commands.entity(live.root).despawn();
    }
}

/// Met chaque modèle dans la pose du sprite (repos, marche, coup) et tourne son regard dans le sens où le personnage va.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut models: Query<(Entity, &mut Live3d, Option<&WalkCycle>, Option<&crate::Facing>, Option<&Prop>, Option<&LevelExit>, Option<&ModelTint>, &InheritedVisibility, Option<&crate::sim::ExitAxis>)>,
    mut transforms: Query<&mut Transform>,
    mut shown: Query<&mut Visibility>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    active: Res<ActiveLight>,
) {
    let tuning = &active.0;
    let caches = &mut *caches;
    let shader = crate::settings::model_shader();
    for (entity, mut live, cycle, facing, prop, exit, tint, visible, axis) in &mut models {
        let anim = match cycle {
            Some(cycle) => {
                let (kind, t) = cycle.pose;
                match kind {
                    PoseKind::Idle => Anim::Idle(t),
                    PoseKind::Walk => Anim::Walk(t),
                    PoseKind::Attack => Anim::Attack(t),
                }
            }
            // Une sortie: sa boucle de dix images (torches et bannières qui vacillent).
            None if exit.is_some() => Anim::Idle((time.elapsed_secs() * 1.2).fract()),
            // Un objet: vingt images en boucle.
            None => Anim::Idle((time.elapsed_secs() * 1.25).fract()),
        };
        // Un objet qui change d'état (un piège qui se désarme...) ou une sortie qui s'ouvre (pont réparé, barrière ouverte...) change de
        // modèle.
        let changed = match (prop, exit) {
            (Some(prop), _) => prop_model(prop.0),
            (_, Some(exit)) => Some(ModelSource::Exit(exit_row(exit.kind, exit.open))),
            _ => None,
        };
        if let Some(source) = changed.filter(|source| *source != live.source) {
            live.source = source;
        }
        // Le regard: les héros ont le leur (répliqué); les monstres suivent leur dernier déplacement (une des huit directions).
        let look = match (facing, cycle) {
            (Some(facing), _) if facing.0.length_squared() > 0.01 => facing.0.normalize(),
            (_, Some(cycle)) => Vec2::from_angle(cycle.direction as f32 * std::f32::consts::FRAC_PI_4),
            _ => Vec2::NEG_Y,
        };
        let yaw = look.x.atan2(-look.y);
        let at = transforms.get(entity).map(|at| at.translation.truncate()).unwrap_or_default();
        if let Ok(mut root) = transforms.get_mut(live.root) {
            root.rotation = if cycle.is_some() {
                Quat::from_rotation_y(yaw)
            } else if axis.is_some_and(|axis| axis.vertical) {
                // Un pont du nord au sud: son modèle (ouest-est) est tourné d'un quart de tour.
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            } else {
                Quat::IDENTITY
            };
            // Sur le sol à la position du personnage (le nord est -z), à l'échelle du monde.
            root.translation = Vec3::new(at.x, 0.0, -at.y);
            root.scale = Vec3::splat(live.scale);
        }
        if let Ok(mut visibility) = shown.get_mut(live.root) {
            let wanted = if visible.get() { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
        // La teinte et l'éclairage: un matériau par matière, partagés. Le changer remplace les matériaux des corps déjà posés.
        let tint = tint_key(tint);
        let cloth = caches.cloth.get_or_insert_with(|| images.add(weave_image())).clone();
        let restyle = live.tint != tint || live.shader != Some(shader);
        let kind = if facing.is_some() {
            Kind::Hero
        } else if cycle.is_some() {
            Kind::Enemy
        } else {
            Kind::Object
        };
        let face = face_path(&live.source);
        // Les maillages de la pose: construits à l'avance (`prebake`), sinon à la demande.
        let (family, index) = live.source.pose(anim);
        let wanted = (live.source.clone(), family, index);
        let new_pose = live.shown.as_ref() != Some(&wanted);
        if !restyle && !new_pose {
            continue;
        }
        let pose = if new_pose { Some(caches.poses.get_or_build(&wanted, &mut meshes)) } else { None };
        for surface in Surface::ALL {
            let slot = surface.index();
            let material = restyle.then(|| material_for(caches, &mut materials, &cloth, &assets, face, surface, shader, tint, kind, tuning));
            match (live.bodies[slot], pose.as_ref().map(|meshes| meshes[slot].clone())) {
                // La pose change: le corps montre le maillage de sa matière, ou se cache s'il n'y en a pas.
                (Some(body), Some(Some(handle))) => {
                    commands.entity(body).insert((Mesh3d(handle), Visibility::Inherited));
                }
                (Some(body), Some(None)) => {
                    commands.entity(body).insert(Visibility::Hidden);
                }
                (None, Some(Some(handle))) => {
                    let material = material.clone().unwrap_or_else(|| material_for(caches, &mut materials, &cloth, &assets, face, surface, shader, tint, kind, tuning));
                    live.bodies[slot] = Some(
                        commands
                            .spawn((Mesh3d(handle), MeshMaterial3d(material), ChildOf(live.root), RenderLayers::layer(crate::decor3d::LAYER)))
                            .id(),
                    );
                }
                _ => {}
            }
            if let (Some(body), Some(material)) = (live.bodies[slot], material) {
                commands.entity(body).insert(MeshMaterial3d(material));
            }
        }
        live.tint = tint;
        live.shader = Some(shader);
        live.shown = Some(wanted);
    }
}

/// Le matériau d'une matière, d'un éclairage et d'une teinte (et, pour le crâne, de la photo du héros), créé la première fois.
#[allow(clippy::too_many_arguments)]
fn material_for(
    caches: &mut Caches,
    standard: &mut Assets<ModelMaterial>,
    cloth: &Handle<Image>,
    assets: &AssetServer,
    face: Option<&'static str>,
    surface: Surface,
    shader: ModelShader,
    tint: TintKey,
    kind: Kind,
    tuning: &LevelLight,
) -> Handle<ModelMaterial> {
    // Seul le crâne dépend de la photo: les autres matières sont communes à tous les modèles.
    let face = if surface == Surface::Face { face } else { None };
    let face_image = face.map(|path| caches.faces.entry(path).or_insert_with(|| load_face(assets, path)).clone());
    caches
        .materials
        .entry((surface, shader, tint, face, kind))
        .or_insert_with(|| {
            let lit = shader == ModelShader::Lit;
            let color = tint_color(tint);
            let mut material = ModelMaterial {
                base: surface_material(surface, lit, color, Some(cloth), face_image.as_ref()),
                extension: GlowExtension { glow: Vec4::ZERO },
            };
            respond(&mut material, surface, color, kind.response(tuning), lit);
            standard.add(material)
        })
        .clone()
}
