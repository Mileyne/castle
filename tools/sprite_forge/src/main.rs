//! Génère `heroes.png`: 4 classes x 8 directions x (4 images de repos + 8 images de marche), en 96x96.
//!
//! Usage: cargo run --release -p sprite_forge -- <dossier de sortie> [dossier d'aperçu]

mod decals;
mod effects;
mod heroes;
mod heroes_gore;
mod heroes_medieval;
mod monsters;
mod photo;
mod monsters_gore;
mod props;
mod props_gore;
mod render;
mod shapes;
mod tiles;

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::path::{Path, PathBuf};

use heroes::{CLASSES, Class, Pose, build};
use shapes::Theme;
use render::{Camera, render};

/// Côté d'une case, en pixels.
const CELL: usize = 96;
/// Côté d'une tuile du décor, en pixels.
const TILE_CELL: usize = 48;
const IDLE_FRAMES: usize = 4;
const WALK_FRAMES: usize = 8;
/// Images d'attaque en plus des images de repos et de marche (les héros seulement).
const ATTACK_FRAMES: usize = 6;
/// Images par direction: ennemis (repos + marche) et héros (en plus, l'attaque).
const FRAMES: usize = IDLE_FRAMES + WALK_FRAMES;
const HERO_FRAMES: usize = FRAMES + ATTACK_FRAMES;
/// Pixels par mètre du modèle, et inclinaison de la vue.
const SCALE: f32 = 40.0;
const ELEVATION: f32 = 0.42;
const FEET_Y: f32 = 88.0;
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
    } else if frame < FRAMES {
        Pose::Walk((frame - IDLE_FRAMES) as f32 / WALK_FRAMES as f32)
    } else {
        // Les images d'attaque prennent le milieu de six tranches du mouvement.
        Pose::Attack((frame - FRAMES) as f32 / ATTACK_FRAMES as f32 + 0.5 / ATTACK_FRAMES as f32)
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

fn top_macro_clone(c: &Camera) -> Camera {
    top_cam_clone(c)
}

fn top_cam_clone(c: &Camera) -> Camera {
    Camera { yaw: c.yaw, elevation: c.elevation, scale: c.scale, size: c.size, feet_y: c.feet_y, outline: c.outline, shadow: c.shadow }
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

/// Rend toutes les images d'une classe: DIRECTIONS lignes de FRAMES colonnes.
fn class_sheet(class: Class, theme: Theme) -> Vec<u8> {
    let (w, h) = (HERO_FRAMES * CELL, DIRECTIONS * CELL);
    let mut sheet = vec![0u8; w * h * 4];
    for direction in 0..DIRECTIONS {
        let cam = camera(direction);
        for frame in 0..HERO_FRAMES {
            let parts = build(class, frame_pose(frame), theme);
            let image = to_rgba8(&render(&parts, &cam, SUPERSAMPLE));
            for y in 0..CELL {
                let dst = ((direction * CELL + y) * w + frame * CELL) * 4;
                sheet[dst..dst + CELL * 4].copy_from_slice(&image[y * CELL * 4..(y + 1) * CELL * 4]);
            }
        }
    }
    sheet
}

/// Génère toutes les planches d'un thème dans `out_dir`, et des aperçus agrandis dans `preview` si demandé.
fn generate(theme: Theme, out_dir: &Path, preview: Option<&Path>) {
    std::fs::create_dir_all(out_dir).unwrap();

    // Une classe par fil d'exécution.
    let sheets: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let handles: Vec<_> = CLASSES.iter().map(|&class| scope.spawn(move || class_sheet(class, theme))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // Planche finale: les classes empilées (ligne = classe * 8 + direction).
    let (w, class_h) = (HERO_FRAMES * CELL, DIRECTIONS * CELL);
    let mut all = Vec::with_capacity(w * class_h * 4 * CLASSES.len());
    for sheet in &sheets {
        all.extend_from_slice(sheet);
    }
    write_png(&out_dir.join("heroes.png"), w, class_h * CLASSES.len(), &all);

    // Ennemis: 8 directions par monstre, même découpage que les héros.
    let monster_sheet = sheet_of_cells(
        monsters::MONSTERS.len() * DIRECTIONS,
        FRAMES,
        CELL,
        |row, col| {
            let monster = monsters::MONSTERS[row / DIRECTIONS];
            render(&monsters::build(monster, frame_pose(col), theme), &camera(row % DIRECTIONS), SUPERSAMPLE)
        },
    );
    write_png(&out_dir.join("monsters.png"), FRAMES * CELL, monsters::MONSTERS.len() * DIRECTIONS * CELL, &monster_sheet);

    // Objets: une ligne par objet, quatre images d'animation, vus de face comme les héros.
    let object_sheet = sheet_of_cells(props::PROPS.len(), props::FRAMES, CELL, |row, col| {
        let t = col as f32 / props::FRAMES as f32;
        let prop = props::PROPS[row];
        // Les plus grands objets (portail, rune) sont rendus plus petits pour tenir dans la case.
        let scale = match prop {
            props::Prop::Portal | props::Prop::Rune => 31.0,
            _ => SCALE,
        };
        let cam = Camera { yaw: 0.0, elevation: ELEVATION, scale, size: CELL, feet_y: FEET_Y, outline: true, shadow: true };
        render(&props::build(prop, t, theme), &cam, SUPERSAMPLE)
    });
    write_png(&out_dir.join("objects.png"), props::FRAMES * CELL, props::PROPS.len() * CELL, &object_sheet);

    // Tuiles: le sol et les faces de mur sont rendus en grands motifs (4x4 tuiles, 4x1 tuiles) puis découpés,
    // pour que les dalles et les blocs traversent les bordures de tuiles. Une ligne par région.
    let macro_px = tiles::MACRO * TILE_CELL;
    let make_cam = |elevation: f32| Camera { yaw: 0.0, elevation, scale: SCALE, size: macro_px, feet_y: macro_px as f32 / 2.0, outline: false, shadow: false };
    let (top_macro, front_macro) = (make_cam(FRAC_PI_2), make_cam(0.0));
    let tile_cam = Camera { size: TILE_CELL, feet_y: TILE_CELL as f32 / 2.0, ..make_cam(FRAC_PI_2) };
    let tile_front = Camera { elevation: 0.0, ..top_cam_clone(&tile_cam) };
    // Rendu des grands motifs de chaque région, en parallèle.
    let macros: Vec<(Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>, Vec<Vec<[f32; 4]>>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..tiles::REGIONS)
            .map(|region| {
                let (top_macro, front_macro) = (top_macro_clone(&top_macro), top_macro_clone(&front_macro));
                scope.spawn(move || {
                    let floors = (0..tiles::FLOOR_PATTERNS)
                        .map(|v| render(&tiles::floor_macro(region, v, (region * 31 + v) as u64 * 7919 + 17, theme), &top_macro, SUPERSAMPLE))
                        .collect();
                    let roofs = (0..tiles::TOP_PATTERNS)
                        .map(|v| render(&tiles::roof_macro(region, v, (region * 41 + v) as u64 * 5303 + 11, theme), &top_macro, SUPERSAMPLE))
                        .collect();
                    let faces = (0..tiles::FACE_PATTERNS)
                        .map(|v| render(&tiles::wall_face_macro(region, v, (region * 37 + v) as u64 * 6151 + 29, theme), &front_macro, SUPERSAMPLE))
                        .collect();
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
            let seed = (region * 100 + col) as u64 * 7919 + 17;
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
                let tile = render(&tiles::wall_face_decor(region, 1, seed, theme), &tile_front, SUPERSAMPLE);
                blur(&tile, TILE_CELL, TILE_CELL, radius)
            }
        })
    };
    let tile_sheet = build_sheet(0);
    write_png(&out_dir.join("tiles.png"), tiles::COLUMNS * TILE_CELL, tiles::REGIONS * TILE_CELL, &tile_sheet);
    // Trois niveaux de flou croissants, pour des transitions progressives d'une zone à l'autre.
    let mut blur_sheets = Vec::new();
    for (level, radius) in [(1, 1usize), (2, 2), (3, 4)] {
        let sheet = build_sheet(radius);
        write_png(&out_dir.join(format!("tiles_blur{level}.png")), tiles::COLUMNS * TILE_CELL, tiles::REGIONS * TILE_CELL, &sheet);
        blur_sheets.push(sheet);
    }

    // Projectiles (orientés vers la droite) et effets de coups, dans le style du thème.
    let projectile_sheet = sheet_of_cells(1, effects::PROJECTILES, effects::PROJECTILE_CELL, |_, col| effects::projectile(theme, col));
    write_png(&out_dir.join("projectiles.png"), effects::PROJECTILES * effects::PROJECTILE_CELL, effects::PROJECTILE_CELL, &projectile_sheet);
    let fx_sheet = sheet_of_cells(1, effects::FX, effects::FX_CELL, |_, col| if col == 0 { effects::slash(theme) } else { effects::smash(theme) });
    write_png(&out_dir.join("fx.png"), effects::FX * effects::FX_CELL, effects::FX_CELL, &fx_sheet);

    // Vignettes des héros: les visages des photos de référence (dossier `photos/`), identiques dans tous les styles.
    // Carrés (x, y, côté) repérés sur chaque photo, dans l'ordre de `CLASSES`.
    const PORTRAIT: usize = 128;
    let mut portraits = vec![0u8; CLASSES.len() * PORTRAIT * PORTRAIT * 4];
    for (col, class) in CLASSES.iter().enumerate() {
        let (file, x, y, side) = match class {
            Class::Knight => ("Chevalier", 340, 60, 280),
            Class::Sorcerer => ("Ensorceleur", 360, 200, 270),
            Class::Ranger => ("Rodeur", 340, 50, 270),
            Class::Forgebraise => ("Forgebraise", 330, 10, 270),
        };
        let tile = photo::Photo::load(Path::new(&format!("photos/{file}.png"))).square(x, y, side, PORTRAIT);
        let mut tile = tile;
        photo::cutout(&mut tile, PORTRAIT, 12);
        for row in 0..PORTRAIT {
            let dst = (row * CLASSES.len() * PORTRAIT + col * PORTRAIT) * 4;
            portraits[dst..dst + PORTRAIT * 4].copy_from_slice(&tile[row * PORTRAIT * 4..(row + 1) * PORTRAIT * 4]);
        }
    }
    write_png(&out_dir.join("portraits.png"), CLASSES.len() * PORTRAIT, PORTRAIT, &portraits);

    // Marques d'usure du sol: masques blancs que le jeu teinte et étire.
    let decal_sheet = sheet_of_cells(1, decals::COLUMNS, decals::CELL, |_, col| decals::cells()[col].clone());
    write_png(&out_dir.join("decals.png"), decals::COLUMNS * decals::CELL, decals::CELL, &decal_sheet);

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
        zoomed("portraits_x2.png", CLASSES.len() * PORTRAIT, PORTRAIT, &portraits, 2);
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
        for (name, sheet) in [("tiles_region", &tile_sheet), ("tiles_blur3_region", &blur_sheets[2])] {
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
        zoomed("objects_x1.png", props::FRAMES * CELL, props::PROPS.len() * CELL, &object_sheet, 1);
        // Squelette (lignes 0 à 7) et chauve-souris (8 à 15): première image de repos de quatre directions.
        let mut picks = vec![0u8; 8 * CELL * CELL * 4];
        for (slot, row) in [6usize, 7, 0, 2, 14, 15, 8, 10].into_iter().enumerate() {
            for y in 0..CELL {
                let src = ((row * CELL + y) * FRAMES * CELL) * 4;
                let dst = (y * 8 * CELL + slot * CELL) * 4;
                picks[dst..dst + CELL * 4].copy_from_slice(&monster_sheet[src..src + CELL * 4]);
            }
        }
        zoomed("monsters_x2.png", 8 * CELL, CELL, &picks, 2);
        // Séquence d'attaque (6 images) de chaque classe, vue de face (sud) puis de profil droit (est).
        for (i, &class) in CLASSES.iter().enumerate() {
            let zoom = 2;
            let (cw, ch) = (ATTACK_FRAMES * CELL * zoom, 2 * CELL * zoom);
            let mut strip = vec![0u8; cw * ch * 4];
            for (row, direction) in [6usize, 0].into_iter().enumerate() {
                for k in 0..ATTACK_FRAMES {
                    let image = to_rgba8(&render(&build(class, frame_pose(FRAMES + k), theme), &camera(direction), SUPERSAMPLE));
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
        for (i, sheet) in sheets.iter().enumerate() {
            write_png(&dir.join(format!("hero_{i}.png")), w, class_h, sheet);
        }
    }
    let _ = (PI, TAU);
    println!("{}: heroes.png {}x{} px, {} images par direction", theme.folder(), w, class_h * CLASSES.len(), HERO_FRAMES);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    // Un sous-dossier par thème: manga/ et gore/.
    for theme in Theme::ALL {
        let preview = args.get(1).map(|p| PathBuf::from(p).join(theme.folder()));
        generate(theme, &root.join(theme.folder()), preview.as_deref());
    }
}
