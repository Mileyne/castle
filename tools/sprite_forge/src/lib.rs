//! Génère `heroes.png`: 4 classes x 8 directions x (25 images de repos + 16 de marche + 10 d'attaque), en 96x96.
//!
//! Usage: cargo run --release -p sprite_forge -- <dossier de sortie> [dossier d'aperçu]
//!
//! Le même code sert l'éditeur (`sprite_editor`): la bibliothèque expose la génération, les réglages des modèles
//! (`tuning`) et les retouches de tuiles (`overrides`).

mod decals;
mod decor;
pub mod glb;
mod editor_api;
mod effects;
mod heroes;
mod heroes_medieval;
mod landmarks;
mod monsters;
mod photo;
mod monsters_boss;
mod monsters_more;
mod outdoor;
pub mod overrides;
mod props;
mod render;
mod shapes;
mod tiles;
pub mod tuning;

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::path::{Path, PathBuf};

use heroes::{CLASSES, Class, Pose, build};
pub use editor_api::*;
pub use shapes::Theme;
use render::{Camera, render};

/// Niveau de détail des sprites (héros, ennemis, objets, sorties): 1 = cases de 96 pixels, 3 = cases de 288. Tout ce qui est
/// dessiné en pixels en dépend (`FORGE_DETAIL=2` à la compilation produit le jeu de planches « léger » pour les appareils à peu de mémoire); le jeu affiche les sprites à la même taille quel que soit ce niveau.
const DETAIL: usize = match option_env!("FORGE_DETAIL") {
    Some(digit) => (digit.as_bytes()[0] - b'0') as usize,
    None => 3,
};
/// Côté d'une case, en pixels.
const CELL: usize = 96 * DETAIL;
/// Côté d'une tuile du décor, en pixels.
const TILE_CELL: usize = 48;
/// Case d'une sortie de niveau, et nombre d'images de son animation.
const EXIT_CELL: usize = 192 * DETAIL;
const EXIT_FRAMES: usize = 10;
/// Les cycles sont rendus pour une lecture à 25 images par seconde: repos d'une seconde, marche dont le cycle dure
/// 16 images (25 images/s à 1,6 cycle/s), attaque de 0,4 s.
const IDLE_FRAMES: usize = 25;
const WALK_FRAMES: usize = 16;
/// Images d'attaque en plus des images de repos et de marche.
const ATTACK_FRAMES: usize = 10;
/// Images de repos et de marche, puis images par direction pour les ennemis comme pour les héros (en plus, l'attaque).
const MOVE_FRAMES: usize = IDLE_FRAMES + WALK_FRAMES;
const FRAMES: usize = MOVE_FRAMES + ATTACK_FRAMES;
/// Pixels par mètre du modèle, et inclinaison de la vue.
const SCALE: f32 = 40.0 * DETAIL as f32;
/// Les tuiles du décor ne dépendent pas du niveau de détail des sprites.
const TILE_SCALE: f32 = 40.0;
const ELEVATION: f32 = 0.42;
const FEET_Y: f32 = 88.0 * DETAIL as f32;
/// Colonnes des planches d'un personnage ou d'un monstre: une direction fait trois lignes (51 images), pour que les planches
/// restent de taille raisonnable (un fichier par personnage, loin des limites de GitHub et des cartes graphiques).
const SHEET_COLUMNS: usize = 17;
const SUPERSAMPLE: usize = 3;

/// Ordre des lignes: angle croissant depuis l'est, dans le sens trigonométrique (le haut de l'écran est le nord).
const DIRECTIONS: usize = 8;

fn camera(direction: usize) -> Camera {
    // Direction de regard à l'écran: (cos, sin) avec y vers le haut. Le nord s'éloigne du spectateur (-Z).
    let angle = direction as f32 * FRAC_PI_4;
    let facing = (angle.cos(), angle.sin());
    let yaw = f32::atan2(facing.0, -facing.1);
    Camera { yaw, elevation: ELEVATION, scale: SCALE, size: CELL, feet_y: FEET_Y, outline: true, shadow: true }
}

fn frame_pose(frame: usize) -> Pose {
    if frame < IDLE_FRAMES {
        Pose::Idle(frame as f32 / IDLE_FRAMES as f32)
    } else if frame < MOVE_FRAMES {
        Pose::Walk((frame - IDLE_FRAMES) as f32 / WALK_FRAMES as f32)
    } else {
        // Les images d'attaque prennent le milieu de tranches égales du mouvement.
        Pose::Attack((frame - MOVE_FRAMES) as f32 / ATTACK_FRAMES as f32 + 0.5 / ATTACK_FRAMES as f32)
    }
}

fn to_rgba8(pixels: &[[f32; 4]]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|p| {
            let a = p[3].clamp(0.0, 1.0);
            [p[0], p[1], p[2]].map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8).into_iter().chain([(a * 255.0 + 0.5) as u8])
        })
        .collect()
}

fn write_png(path: &Path, width: usize, height: usize, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// Flou séparable (boîte, répété) d'une image RGBA de `w` x `h` pixels. Les bords bouclent sur le côté opposé: les
/// joints qui tombent sur les bords d'un grand motif se mélangent à de la pierre comme les autres, ce qui évite
/// une ligne sombre tous les quatre tuiles.
fn blur(img: &[[f32; 4]], w: usize, h: usize, radius: usize) -> Vec<[f32; 4]> {
    if radius == 0 {
        return img.to_vec();
    }
    let pass = |src: &[[f32; 4]], horizontal: bool| -> Vec<[f32; 4]> {
        let mut out = vec![[0.0f32; 4]; w * h];
        let span = (2 * radius + 1) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 4];
                for k in -(radius as i32)..=(radius as i32) {
                    let (sx, sy) = if horizontal {
                        ((x as i32 + k).rem_euclid(w as i32) as usize, y)
                    } else {
                        (x, (y as i32 + k).rem_euclid(h as i32) as usize)
                    };
                    for c in 0..4 {
                        sum[c] += src[sy * w + sx][c];
                    }
                }
                out[y * w + x] = sum.map(|v| v / span);
            }
        }
        out
    };
    let once = pass(&pass(img, true), false);
    pass(&pass(&once, true), false)
}

/// Assemble une planche de `rows` x `cols` cases carrées de `cell` pixels, rendues par `draw(ligne, colonne)`.
fn sheet_of_cells(rows: usize, cols: usize, cell: usize, draw: impl Fn(usize, usize) -> Vec<[f32; 4]> + Sync) -> Vec<u8> {
    let w = cols * cell;
    let mut sheet = vec![0u8; w * rows * cell * 4];
    // Une ligne par fil d'exécution (en lots) pour garder des temps courts.
    let rendered: Vec<Vec<Vec<u8>>> = std::thread::scope(|scope| {
        let draw = &draw;
        let handles: Vec<_> = (0..rows)
            .map(|row| scope.spawn(move || (0..cols).map(|col| to_rgba8(&draw(row, col))).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (row, cells) in rendered.iter().enumerate() {
        for (col, image) in cells.iter().enumerate() {
            for y in 0..cell {
                let dst = ((row * cell + y) * w + col * cell) * 4;
                sheet[dst..dst + cell * 4].copy_from_slice(&image[y * cell * 4..(y + 1) * cell * 4]);
            }
        }
    }
    sheet
}

/// Les cases (RGBA, `CELL` x `CELL`) d'un personnage ou d'un monstre: les huit directions à la suite (direction * FRAMES +
/// image). `build` donne les pièces d'une pose.
fn character_cells(build: impl Fn(Pose) -> Vec<render::Part> + Sync) -> Vec<Vec<u8>> {
    let total = DIRECTIONS * FRAMES;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = total.div_ceil(threads);
    let build = &build;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    (t * per..((t + 1) * per).min(total))
                        .map(|index| {
                            let (direction, frame) = (index / FRAMES, index % FRAMES);
                            to_rgba8(&render(&build(frame_pose(frame)), &camera(direction), SUPERSAMPLE))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// Zone de la case qui contient quelque chose dans au moins une image: le reste est transparent partout et n'est pas stocké.
#[derive(Clone, Copy, Debug)]
struct Crop {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

/// La plus petite zone qui contient tous les pixels non transparents de `cells`, arrondie au multiple de 4 le plus proche
/// (les textures compressées sont faites de blocs de 4 x 4 pixels).
fn union_crop(cells: &[Vec<u8>], cell: usize) -> Crop {
    let (mut x0, mut y0, mut x1, mut y1) = (cell, cell, 0, 0);
    for image in cells {
        for y in 0..cell {
            for x in 0..cell {
                if image[(y * cell + x) * 4 + 3] != 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return Crop { x: 0, y: 0, w: 4, h: 4 };
    }
    // Une petite marge, puis des dimensions multiples de 4 (en gardant la zone dans la case).
    let (x0, y0, x1, y1) = (x0.saturating_sub(2), y0.saturating_sub(2), (x1 + 2).min(cell), (y1 + 2).min(cell));
    let round = |lo: usize, hi: usize| {
        let size = (hi - lo).div_ceil(4) * 4;
        let start = lo.min(cell.saturating_sub(size));
        (start, size.min(cell))
    };
    let ((x, w), (y, h)) = (round(x0, x1), round(y0, y1));
    Crop { x, y, w, h }
}

/// Assemble des cases rognées en une planche de `cols` colonnes, et la renvoie avec ses dimensions.
fn pack_cells(cells: &[Vec<u8>], cell: usize, crop: Crop, cols: usize) -> (Vec<u8>, usize, usize) {
    let rows = cells.len().div_ceil(cols);
    let (w, h) = (cols * crop.w, rows * crop.h);
    let mut sheet = vec![0u8; w * h * 4];
    for (i, image) in cells.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        for y in 0..crop.h {
            let src = ((crop.y + y) * cell + crop.x) * 4;
            let dst = ((row * crop.h + y) * w + col * crop.w) * 4;
            sheet[dst..dst + crop.w * 4].copy_from_slice(&image[src..src + crop.w * 4]);
        }
    }
    (sheet, w, h)
}

/// Écrit une image en texture compressée (Basis Universal, UASTC): le jeu la décompresse vers le format de la carte graphique
/// (BC7, ASTC ou ETC2), où elle occupe 8 bits par pixel au lieu de 32.
fn write_basis(path: &Path, width: usize, height: usize, rgba: &[u8]) {
    use basis_universal::{BasisTextureFormat, ColorSpace, Compressor, CompressorParams, UASTC_QUALITY_DEFAULT};
    let mut params = CompressorParams::new();
    params.set_basis_format(BasisTextureFormat::UASTC4x4);
    params.set_generate_mipmaps(false);
    params.set_color_space(ColorSpace::Srgb);
    params.set_uastc_quality_level(UASTC_QUALITY_DEFAULT);
    params.source_image_mut(0).init(rgba, width as u32, height as u32, 4);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get() as u32);
    let mut compressor = Compressor::new(threads);
    // SAFETY: les paramètres sont valides (une seule image RGBA, format UASTC); c'est le contrat des liaisons de basis-universal.
    unsafe {
        compressor.init(&params);
        compressor.process().unwrap_or_else(|e| panic!("{}: compression impossible ({e:?})", path.display()));
    }
    std::fs::write(path, compressor.basis_file()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// Ce que le jeu doit savoir d'une planche rognée: la taille d'une case, et où cette zone se trouvait dans la case d'origine.
struct SheetMeta {
    dir: &'static str,
    name: String,
    crop: Crop,
}

/// Nom du fichier d'un personnage, d'un monstre ou d'un objet (le jeu applique la même règle).
fn file_name(name: impl std::fmt::Debug) -> String {
    format!("{name:?}").to_lowercase()
}

/// Caméra d'un grand motif de tuiles (`MACRO` x `MACRO` tuiles), vue de dessus (`FRAC_PI_2`) ou de face (0).
fn macro_camera(elevation: f32) -> Camera {
    let size = tiles::MACRO * TILE_CELL;
    Camera { yaw: 0.0, elevation, scale: TILE_SCALE, size, feet_y: size as f32 / 2.0, outline: false, shadow: false }
}

/// Grands motifs d'une région, tels que la génération les rend (graines comprises): l'éditeur les montre à l'identique.
fn render_floor(theme: Theme, region: usize, pattern: usize) -> Vec<[f32; 4]> {
    let seed = (region * 31 + pattern) as u64 * 7919 + 17;
    render(&tiles::floor_macro(region, pattern, seed, theme), &macro_camera(FRAC_PI_2), SUPERSAMPLE)
}

fn render_roof(theme: Theme, region: usize, pattern: usize) -> Vec<[f32; 4]> {
    let seed = (region * 41 + pattern) as u64 * 5303 + 11;
    render(&tiles::roof_macro(region, pattern, seed, theme), &macro_camera(FRAC_PI_2), SUPERSAMPLE)
}

fn render_face(theme: Theme, region: usize, pattern: usize) -> Vec<[f32; 4]> {
    let seed = (region * 37 + pattern) as u64 * 6151 + 29;
    render(&tiles::wall_face_macro(region, pattern, seed, theme), &macro_camera(0.0), SUPERSAMPLE)
}

/// Face de mur décorée (une seule tuile, non floutée), à la colonne `col` de la planche.
fn render_decor(theme: Theme, region: usize, col: usize) -> Vec<[f32; 4]> {
    let seed = (region * 100 + col) as u64 * 7919 + 17;
    let cam = Camera { size: TILE_CELL, feet_y: TILE_CELL as f32 / 2.0, ..macro_camera(0.0) };
    render(&tiles::wall_face_decor(region, 1, seed, theme), &cam, SUPERSAMPLE)
}

/// Cadrage d'un objet: posé au sol et vu de trois quarts, les plus grands (escalier, rune) un peu plus petits pour tenir
/// dans la case; les panneaux de mur sont vus de face et centrés dans leur case.
fn prop_camera(prop: props::Prop) -> Camera {
    if prop.is_wall_mounted() {
        return Camera { yaw: 0.0, elevation: 0.0, scale: SCALE, size: CELL, feet_y: CELL as f32 / 2.0, outline: true, shadow: false };
    }
    let scale = if matches!(prop, props::Prop::Stairs | props::Prop::Rune) { 31.0 * DETAIL as f32 } else { SCALE };
    Camera { yaw: 0.0, elevation: ELEVATION, scale, size: CELL, feet_y: FEET_Y, outline: true, shadow: true }
}

/// Génère toutes les planches d'un thème dans `out_dir`, et des aperçus agrandis dans `preview` si demandé.
pub fn generate(theme: Theme, out_dir: &Path, preview: Option<&Path>) {
    std::fs::create_dir_all(out_dir).unwrap();
    // Réglages des modèles et retouches de tuiles faits dans l'éditeur, par-dessus ce que le code produit.
    tuning::set_active(tuning::load(theme));
    let retouched = overrides::load_all(theme);

    // Héros, ennemis et objets: un fichier par personnage, monstre ou objet, rogné à ce qu'il dessine, et compressé. Les
    // dimensions rognées sont écrites dans `sprite_meta.rs`, que le jeu compile.
    for dir in ["heroes", "monsters", "objects"] {
        std::fs::create_dir_all(out_dir.join(dir)).unwrap();
    }
    let mut metas: Vec<SheetMeta> = Vec::new();
    let mut save = |dir: &'static str, name: String, cells: Vec<Vec<u8>>, cols: usize| {
        let crop = union_crop(&cells, CELL);
        let (sheet, w, h) = pack_cells(&cells, CELL, crop, cols);
        write_basis(&out_dir.join(dir).join(format!("{name}.basis")), w, h, &sheet);
        println!("{dir}/{name}: cases de {}x{} px ({}x{} dans la case d'origine)", crop.w, crop.h, crop.x, crop.y);
        metas.push(SheetMeta { dir, name, crop });
    };
    for &class in CLASSES.iter() {
        save("heroes", file_name(class), character_cells(|pose| build(class, pose, theme)), SHEET_COLUMNS);
    }
    for &monster in monsters::MONSTERS.iter() {
        save("monsters", file_name(monster), character_cells(|pose| monsters::build(monster, pose, theme)), SHEET_COLUMNS);
    }
    // Objets: une ligne de vingt images d'animation, vus de face comme les héros.
    for &prop in props::PROPS.iter() {
        let cells: Vec<Vec<u8>> = (0..props::FRAMES)
            .map(|col| to_rgba8(&render(&props::build(prop, col as f32 / props::FRAMES as f32, theme), &prop_camera(prop), SUPERSAMPLE)))
            .collect();
        save("objects", file_name(prop), cells, props::FRAMES);
    }
    let mut source = String::from("//! Généré par `tools/sprite_forge` (ne pas modifier à la main): taille des cases rognées de chaque planche, et position de cette\n//! zone dans la case d'origine de CELL pixels.\n\n");
    source += &format!("/// Côté de la case d'origine, en pixels.\npub const CELL: u32 = {CELL};\n\n");
    source += "/// (dossier, nom, x, y, largeur, hauteur) de la zone conservée dans chaque planche.\npub const SHEETS: &[(&str, &str, u32, u32, u32, u32)] = &[\n";
    for m in &metas {
        source += &format!("    (\"{}\", \"{}\", {}, {}, {}, {}),\n", m.dir, m.name, m.crop.x, m.crop.y, m.crop.w, m.crop.h);
    }
    source += "];\n";
    std::fs::write(out_dir.join("sprite_meta.rs"), source).unwrap();

    // Grands motifs du décor: une planche par motif (une seule image, case des sorties de niveau).
    std::fs::create_dir_all(out_dir.join("landmarks")).unwrap();
    let landmark_camera = Camera { yaw: 0.0, elevation: ELEVATION, scale: SCALE, size: EXIT_CELL, feet_y: FEET_Y * 2.0, outline: true, shadow: true };
    for (index, name) in landmarks::LANDMARKS.iter().enumerate() {
        let image = to_rgba8(&render(&landmarks::build(index), &landmark_camera, SUPERSAMPLE));
        write_basis(&out_dir.join("landmarks").join(format!("{name}.basis")), EXIT_CELL, EXIT_CELL, &image);
    }

    // Sorties de niveau: dessinées en grand (cases de 192 px, éléments agrandis), avec un cycle de vacillement plus court.
    let exit_cell = EXIT_CELL;
    let exit_camera = Camera { yaw: 0.0, elevation: ELEVATION, scale: SCALE, size: exit_cell, feet_y: FEET_Y * 2.0, outline: true, shadow: true };
    let exit_sheet = sheet_of_cells(props::EXITS.len(), EXIT_FRAMES, exit_cell, |row, col| {
        let t = col as f32 / EXIT_FRAMES as f32;
        render(&props::build_exit(props::EXITS[row], t, theme), &exit_camera, SUPERSAMPLE)
    });
    write_basis(&out_dir.join("exits.basis"), EXIT_FRAMES * exit_cell, props::EXITS.len() * exit_cell, &exit_sheet);

    // Tuiles: le sol et les faces de mur sont rendus en grands motifs (4x4 tuiles, 4x1 tuiles) puis découpés,
    // pour que les dalles et les blocs traversent les bordures de tuiles. Une ligne par région.
    let macro_px = tiles::MACRO * TILE_CELL;
    // Rendu des grands motifs de chaque région, en parallèle.
    let macros: Vec<(Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..tiles::REGIONS)
            .map(|region| {
                scope.spawn(move || {
                    let floors = (0..tiles::FLOOR_PATTERNS).map(|v| render_floor(theme, region, v)).collect();
                    let roofs = (0..tiles::TOP_PATTERNS).map(|v| render_roof(theme, region, v)).collect();
                    let faces = (0..tiles::FACE_PATTERNS).map(|v| render_face(theme, region, v)).collect();
                    (floors, roofs, faces)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let slice = |img: &[[f32; 4]], x0: usize, y0: usize| -> Vec<[f32; 4]> {
        let mut out = Vec::with_capacity(TILE_CELL * TILE_CELL);
        for y in 0..TILE_CELL {
            for x in 0..TILE_CELL {
                out.push(img[(y0 + y) * macro_px + x0 + x]);
            }
        }
        out
    };
    // Trois versions de la planche: nette, puis légèrement floue, puis plus floue (pour les zones éloignées du héros).
    // Le flou est appliqué aux grands motifs avant le découpage, pour ne pas mélanger des tuiles sans rapport.
    let build_sheet = |radius: usize| -> Vec<u8> {
        let blurred: Vec<(Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>)> = macros
            .iter()
            .map(|(floors, roofs, faces)| {
                let b = |set: &Vec<Vec<[f32; 4]>>| set.iter().map(|img| blur(img, macro_px, macro_px, radius)).collect::<Vec<_>>();
                (b(floors), b(roofs), b(faces))
            })
            .collect();
        sheet_of_cells(tiles::REGIONS, tiles::COLUMNS, TILE_CELL, |region, col| {
            // Une tuile retouchée à la main remplace la tuile générée (puis se floute comme elle).
            if let Some(tile) = retouched.get(&(region, col)) {
                return blur(tile, TILE_CELL, TILE_CELL, radius);
            }
            let (floors, roofs, faces) = &blurred[region];
            if col < tiles::FLOOR_COLUMNS {
                let (pattern, within) = (col / (tiles::MACRO * tiles::MACRO), col % (tiles::MACRO * tiles::MACRO));
                slice(&floors[pattern], (within % tiles::MACRO) * TILE_CELL, (within / tiles::MACRO) * TILE_CELL)
            } else if col < tiles::FACE_START {
                let within_all = col - tiles::TOP_START;
                let (pattern, within) = (within_all / (tiles::MACRO * tiles::MACRO), within_all % (tiles::MACRO * tiles::MACRO));
                slice(&roofs[pattern], (within % tiles::MACRO) * TILE_CELL, (within / tiles::MACRO) * TILE_CELL)
            } else if col < tiles::DECOR_START {
                let (pattern, within) = ((col - tiles::FACE_START) / tiles::MACRO, (col - tiles::FACE_START) % tiles::MACRO);
                // La bande de la face occupe le centre du grand carré rendu.
                slice(&faces[pattern], within * TILE_CELL, (macro_px - TILE_CELL) / 2)
            } else {
                blur(&render_decor(theme, region, col), TILE_CELL, TILE_CELL, radius)
            }
        })
    };
    let tile_sheet = build_sheet(0);
    write_png(&out_dir.join("tiles.png"), tiles::COLUMNS * TILE_CELL, tiles::REGIONS * TILE_CELL, &tile_sheet);
    // Projectiles (orientés vers la droite) et effets de coups, dans le style du thème.
    let projectile_sheet = sheet_of_cells(1, effects::PROJECTILES, effects::PROJECTILE_CELL, |_, col| effects::projectile(theme, col));
    write_png(&out_dir.join("projectiles.png"), effects::PROJECTILES * effects::PROJECTILE_CELL, effects::PROJECTILE_CELL, &projectile_sheet);
    let fx_sheet = sheet_of_cells(1, effects::FX, effects::FX_CELL, |_, col| if col == 0 { effects::slash(theme) } else { effects::smash(theme) });
    write_png(&out_dir.join("fx.png"), effects::FX * effects::FX_CELL, effects::FX_CELL, &fx_sheet);

    // Vignettes des héros: les visages des photos de référence (dossier `photos/`), identiques dans tous les styles.
    // Carrés (x, y, côté) repérés sur chaque photo, dans l'ordre de `CLASSES`.
    const PORTRAIT: usize = 128;
    // Une colonne de plus que les classes: la princesse Samélia (photo Emma.jpg).
    let columns = CLASSES.len() + 1;
    let mut portraits = vec![0u8; columns * PORTRAIT * PORTRAIT * 4];
    for col in 0..columns {
        let (file, x, y, side) = match CLASSES.get(col) {
            Some(Class::Knight) => ("Chevalier", 340, 60, 280),
            Some(Class::Sorcerer) => ("Ensorceleur", 360, 200, 270),
            Some(Class::Ranger) => ("Rodeur", 340, 50, 270),
            Some(Class::Forgebraise) => ("Forgebraise", 330, 10, 270),
            None => ("Emma", 480, 100, 280),
        };
        let mut tile = photo::Photo::load(Path::new(&format!("photos/{file}.png"))).square(x, y, side, PORTRAIT);
        photo::cutout(&mut tile, PORTRAIT, 12);
        for row in 0..PORTRAIT {
            let dst = (row * columns * PORTRAIT + col * PORTRAIT) * 4;
            portraits[dst..dst + PORTRAIT * 4].copy_from_slice(&tile[row * PORTRAIT * 4..(row + 1) * PORTRAIT * 4]);
        }
    }
    write_png(&out_dir.join("portraits.png"), columns * PORTRAIT, PORTRAIT, &portraits);

    // Marques d'usure du sol: masques blancs que le jeu teinte et étire.
    let decal_sheet = sheet_of_cells(1, decals::COLUMNS, decals::CELL, |_, col| decals::cells()[col].clone());
    write_png(&out_dir.join("decals.png"), decals::COLUMNS * decals::CELL, decals::CELL, &decal_sheet);

    // Décor posé sur le sol: les sortes en colonnes, les variantes en lignes, à l'échelle des tuiles (le jeu les mêle au sol).
    let decor_camera = Camera { yaw: 0.0, elevation: 1.15, scale: TILE_SCALE, size: decor::CELL, feet_y: decor::CELL as f32 / 2.0, outline: false, shadow: false };
    let decor_sheet = sheet_of_cells(decor::VARIANTS, decor::KINDS, decor::CELL, |row, col| render(&decor::build(col, row), &decor_camera, SUPERSAMPLE));
    write_png(&out_dir.join("decor.png"), decor::KINDS * decor::CELL, decor::VARIANTS * decor::CELL, &decor_sheet);

    if let Some(dir) = preview {
        std::fs::create_dir_all(dir).unwrap();
        // Aperçus agrandis (x3, fond gris) des tuiles et des objets, et de quelques lignes d'ennemis.
        let zoomed = |name: &str, w: usize, h: usize, rgba: &[u8], zoom: usize| {
            let (zw, zh) = (w * zoom, h * zoom);
            let mut img = vec![0u8; zw * zh * 4];
            for y in 0..zh {
                for x in 0..zw {
                    let src = ((y / zoom) * w + x / zoom) * 4;
                    let dst = (y * zw + x) * 4;
                    let a = rgba[src + 3] as f32 / 255.0;
                    for c in 0..3 {
                        img[dst + c] = (rgba[src + c] as f32 * a + 70.0 * (1.0 - a)) as u8;
                    }
                    img[dst + 3] = 255;
                }
            }
            write_png(&dir.join(name), zw, zh, &img);
        };
        zoomed("portraits_x2.png", columns * PORTRAIT, PORTRAIT, &portraits, 2);
        zoomed("projectiles_x2.png", effects::PROJECTILES * effects::PROJECTILE_CELL, effects::PROJECTILE_CELL, &projectile_sheet, 2);
        zoomed("fx_x2.png", effects::FX * effects::FX_CELL, effects::FX_CELL, &fx_sheet, 2);
        // Aperçu d'une zone de 12x4 tuiles par région: trois motifs de sol, puis des faces de mur, des dessus et des faces décorées.
        let sheet_w = tiles::COLUMNS * TILE_CELL;
        let tile_at = |sheet: &[u8], region: usize, col: usize, tx: usize, ty: usize, out: &mut Vec<u8>, out_w: usize| {
            for y in 0..TILE_CELL {
                let src = ((region * TILE_CELL + y) * sheet_w + col * TILE_CELL) * 4;
                let dst = ((ty * TILE_CELL + y) * out_w + tx * TILE_CELL) * 4;
                out[dst..dst + TILE_CELL * 4].copy_from_slice(&sheet[src..src + TILE_CELL * 4]);
            }
        };
        for (name, sheet) in [("tiles_region", &tile_sheet)] {
    for region in 0..tiles::REGIONS {
                let (cols, rows) = (12, 6);
                let out_w = cols * TILE_CELL;
                let mut out = vec![0u8; out_w * rows * TILE_CELL * 4];
                for pattern in 0..tiles::FLOOR_PATTERNS {
                    for within in 0..tiles::MACRO * tiles::MACRO {
                        tile_at(sheet, region, pattern * 16 + within, pattern * 4 + within % 4, within / 4, &mut out, out_w);
                    }
                }
                for k in 0..tiles::FACE_PATTERNS * tiles::MACRO {
                    tile_at(sheet, region, tiles::FACE_START + k, k, 4, &mut out, out_w);
                }
                for (k, col) in [tiles::TOP_START, tiles::TOP_START + 16, tiles::DECOR_START, tiles::DECOR_START + 1].into_iter().enumerate() {
                    tile_at(sheet, region, col, 8 + k, 4, &mut out, out_w);
                }
                // Un mur de 12 tuiles vu comme dans le jeu: face sur une rangée, sol dessous.
                for k in 0..12 {
                    tile_at(sheet, region, tiles::FACE_START + (k % 8), k, 5, &mut out, out_w);
                }
                zoomed(&format!("{name}_{region}.png"), out_w, rows * TILE_CELL, &out, 2);
            }
        }
        // Séquence d'attaque de chaque classe, vue de face (sud) puis de profil droit (est).
        for (i, &class) in CLASSES.iter().enumerate() {
            let zoom = 2;
            let (cw, ch) = (ATTACK_FRAMES * CELL * zoom, 2 * CELL * zoom);
            let mut strip = vec![0u8; cw * ch * 4];
            for (row, direction) in [6usize, 0].into_iter().enumerate() {
                for k in 0..ATTACK_FRAMES {
                    let image = to_rgba8(&render(&build(class, frame_pose(MOVE_FRAMES + k), theme), &camera(direction), SUPERSAMPLE));
                    for y in 0..CELL * zoom {
                        for x in 0..CELL * zoom {
                            let src = ((y / zoom) * CELL + x / zoom) * 4;
                            let dst = ((row * CELL * zoom + y) * cw + k * CELL * zoom + x) * 4;
                            let a = image[src + 3] as f32 / 255.0;
                            for c in 0..3 {
                                strip[dst + c] = (image[src + c] as f32 * a + 70.0 * (1.0 - a)) as u8;
                            }
                            strip[dst + 3] = 255;
                        }
                    }
                }
            }
            write_png(&dir.join(format!("attack_{i}.png")), cw, ch, &strip);
        }
        // Gros plans (x3) de quatre directions au repos, pour juger les proportions.
        for (i, &class) in CLASSES.iter().enumerate() {
            let zoom = 3;
            let shown = [6usize, 7, 0, 2]; // face, diagonale, profil droit, dos
            let (cw, ch) = (shown.len() * CELL * zoom, CELL * zoom);
            let mut closeup = vec![0u8; cw * ch * 4];
            for (slot, &direction) in shown.iter().enumerate() {
                let image = to_rgba8(&render(&build(class, Pose::Idle(0.0), theme), &camera(direction), SUPERSAMPLE));
                for y in 0..CELL * zoom {
                    for x in 0..CELL * zoom {
                        let src = ((y / zoom) * CELL + x / zoom) * 4;
                        let dst = (y * cw + slot * CELL * zoom + x) * 4;
                        let px = &image[src..src + 4];
                        // Fond sombre pour voir les contours.
                        let a = px[3] as f32 / 255.0;
                        for c in 0..3 {
                            closeup[dst + c] = (px[c] as f32 * a + 70.0 * (1.0 - a)) as u8;
                        }
                        closeup[dst + 3] = 255;
                    }
                }
            }
            write_png(&dir.join(format!("closeup_{i}.png")), cw, ch, &closeup);
        }
    }
    let _ = (PI, TAU);
    println!("{}: cases de {CELL} px, {FRAMES} images par direction", theme.folder());
}

pub fn cli() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Export des modèles 3D vers glTF, pour Blender: `sprite_forge export-glb <dossier> [clé...]` (voir `glb.rs`).
    if args.first().map(String::as_str) == Some("export-glb") {
        let dir = PathBuf::from(args.get(1).expect("usage: sprite_forge export-glb <dossier de sortie> [clé...]"));
        let written = glb::export_all(&dir, &args[2..]).expect("écriture");
        println!("{written} fichier(s) dans {}", dir.display());
        return;
    }
    // Outils d'étude des photos de référence (voir `photo.rs`).
    if matches!(args.first().map(String::as_str), Some("crop" | "avg")) {
        let n = |i: usize| args[i].parse::<usize>().expect("nombre attendu");
        let photo = photo::Photo::load(Path::new(&args[1]));
        if args[0] == "avg" {
            let c = photo.average(n(2), n(3), n(4), n(5));
            println!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        } else {
            let (w, h, rgba) = photo.crop(n(2), n(3), n(4), n(5), n(6));
            write_png(Path::new(&args[7]), w, h, &rgba);
        }
        return;
    }
    let root = PathBuf::from(args.first().expect("usage: sprite_forge <dossier de sortie> [dossier d'aperçu]"));
    // Un sous-dossier par thème (il n'y a plus que medieval/).
    // Le jeu n'utilise que le style médiéval; `--all` régénère aussi les anciens styles.
    let themes: &[Theme] = if args.iter().any(|a| a == "--all") { &Theme::ALL } else { &[Theme::Medieval] };
    for &theme in themes {
        let preview = args.get(1).filter(|p| !p.starts_with("--")).map(|p| PathBuf::from(p).join(theme.folder()));
        generate(theme, &root.join(theme.folder()), preview.as_deref());
    }
}
