//! Programme `castle_client` (ordinateur).

fn main() {
    // Outils de niveaux (sans fenêtre):
    //   `castle_client --export-levels <dossier> [--force] [--only 1,2,3]`: écrit les niveaux de départ de la campagne;
    //   `castle_client --decorate <dossier> [--force]`: sème le décor au sol des niveaux.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let force = args.iter().any(|a| a == "--force");
    if let Some(i) = args.iter().position(|a| a == "--export-levels") {
        let dir = args.get(i + 1).expect("usage: castle_client --export-levels <dossier> [--force] [--only 1,2,3]");
        // `--only 1,2,3` limite l'export à ces niveaux.
        let only: Option<Vec<u32>> = args
            .iter()
            .position(|a| a == "--only")
            .and_then(|i| args.get(i + 1))
            .map(|list| list.split(',').filter_map(|n| n.trim().parse().ok()).collect());
        castle_client::sim::export_levels(std::path::Path::new(dir), force, only.as_deref()).expect("export des niveaux");
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--decorate") {
        let dir = args.get(i + 1).expect("usage: castle_client --decorate <dossier> [--force]");
        castle_client::sim::decorate_levels(std::path::Path::new(dir), force).expect("décor des niveaux");
        return;
    }
    // `castle_client --niveau=N` (ou `--niveau N`): commence directement au niveau N (1 à 20), sans écran titre ni prologue, dans une
    // sauvegarde jetable (les vraies parties ne sont pas touchées).
    let level = args.iter().enumerate().find_map(|(i, a)| match a.strip_prefix("--niveau") {
        Some(rest) if rest.starts_with('=') => rest[1..].parse::<u32>().ok(),
        Some("") => args.get(i + 1)?.parse::<u32>().ok(),
        _ => None,
    });
    if args.iter().any(|a| a.starts_with("--niveau")) && level.is_none() {
        eprintln!("usage: castle_client --niveau=N (N de 1 à 20)");
        std::process::exit(2);
    }
    if let Some(level) = level {
        let saves = std::env::temp_dir().join(format!("castle_niveau_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&saves);
        // SAFETY: rien d'autre ne tourne encore, l'application n'est pas construite.
        unsafe {
            std::env::set_var("CASTLE_SAVE_DIR", &saves);
            std::env::set_var("CASTLE_AUTOSTART", "niveau");
            std::env::set_var("CASTLE_START_LEVEL", level.to_string());
        }
    }
    castle_client::run();
}
