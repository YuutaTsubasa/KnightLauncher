use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use chrono::Utc;
use tauri::AppHandle;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    ensure_positions, handoff_focus_to_child, read_library_from_disk,
    read_settings_from_disk, write_library_to_disk, Game, GameVariant, Library,
};

pub(crate) struct EmuSystem {
    pub folder: &'static str,
    pub platform_id: &'static str,
    pub extensions: &'static [&'static str],
    pub launchers: &'static [&'static str],
    pub retroarch_core: Option<&'static str>,
}

pub(crate) const EMU_SYSTEMS: &[EmuSystem] = &[
    EmuSystem { folder: "snes", platform_id: "sfc", extensions: &["smc", "sfc", "fig"], launchers: &["retroarch"], retroarch_core: Some("snes9x_libretro.dll") },
    EmuSystem { folder: "nes", platform_id: "fc", extensions: &["nes"], launchers: &["retroarch"], retroarch_core: Some("nestopia_libretro.dll") },
    EmuSystem { folder: "n64", platform_id: "n64", extensions: &["n64", "z64", "v64"], launchers: &["retroarch"], retroarch_core: Some("mupen64plus_next_libretro.dll") },
    EmuSystem { folder: "gb", platform_id: "gb", extensions: &["gb"], launchers: &["retroarch"], retroarch_core: Some("gambatte_libretro.dll") },
    EmuSystem { folder: "gbc", platform_id: "gbc", extensions: &["gbc", "gb"], launchers: &["retroarch"], retroarch_core: Some("gambatte_libretro.dll") },
    EmuSystem { folder: "gba", platform_id: "gba", extensions: &["gba"], launchers: &["retroarch"], retroarch_core: Some("mgba_libretro.dll") },
    EmuSystem { folder: "nds", platform_id: "ds", extensions: &["nds"], launchers: &["melonDS", "retroarch"], retroarch_core: Some("melonds_libretro.dll") },
    EmuSystem { folder: "n3ds", platform_id: "3ds", extensions: &["3ds", "cci", "cia", "app", "cxi"], launchers: &["azahar"], retroarch_core: None },
    EmuSystem { folder: "gc", platform_id: "gc", extensions: &["iso", "gcm", "ciso", "rvz", "wbfs"], launchers: &["dolphin"], retroarch_core: None },
    EmuSystem { folder: "wii", platform_id: "wii", extensions: &["iso", "wbfs", "rvz", "wad"], launchers: &["dolphin", "primehack"], retroarch_core: None },
    EmuSystem { folder: "wiiu", platform_id: "wiiu", extensions: &["wud", "wux", "wua", "rpx"], launchers: &["Cemu"], retroarch_core: None },
    EmuSystem { folder: "switch", platform_id: "switch", extensions: &["nsp", "xci"], launchers: &["Ryujinx"], retroarch_core: None },
    EmuSystem { folder: "genesis", platform_id: "md", extensions: &["md", "smd", "bin", "gen"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "megadrive", platform_id: "md", extensions: &["md", "smd", "bin", "gen"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "sega32x", platform_id: "32x", extensions: &["32x", "md", "smd", "bin"], launchers: &["retroarch"], retroarch_core: Some("picodrive_libretro.dll") },
    EmuSystem { folder: "segacd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "megacd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "sega-cd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "mega-cd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "saturn", platform_id: "sat", extensions: &["cue", "iso", "chd", "mds"], launchers: &["retroarch"], retroarch_core: Some("mednafen_saturn_libretro.dll") },
    EmuSystem { folder: "dreamcast", platform_id: "dc", extensions: &["gdi", "cdi", "cue", "chd"], launchers: &["retroarch"], retroarch_core: Some("flycast_libretro.dll") },
    EmuSystem { folder: "gamegear", platform_id: "gg", extensions: &["gg"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "mastersystem", platform_id: "sms", extensions: &["sms"], launchers: &["retroarch"], retroarch_core: Some("genesis_plus_gx_libretro.dll") },
    EmuSystem { folder: "psx", platform_id: "ps1", extensions: &["chd", "cue", "iso", "pbp", "m3u", "ecm"], launchers: &["duckstation", "retroarch"], retroarch_core: Some("swanstation_libretro.dll") },
    EmuSystem { folder: "ps2", platform_id: "ps2", extensions: &["iso", "chd", "bin", "mdf"], launchers: &["pcsx2"], retroarch_core: None },
    EmuSystem { folder: "ps3", platform_id: "ps3", extensions: &["iso"], launchers: &["rpcs3"], retroarch_core: None },
    EmuSystem { folder: "ps4", platform_id: "ps4", extensions: &["pkg", "iso"], launchers: &["shadps4"], retroarch_core: None },
    EmuSystem { folder: "psp", platform_id: "psp", extensions: &["iso", "cso", "pbp"], launchers: &["PPSSPP"], retroarch_core: None },
    EmuSystem { folder: "psvita", platform_id: "vita", extensions: &["vpk"], launchers: &["Vita3K"], retroarch_core: None },
    EmuSystem { folder: "ngp", platform_id: "ngpc", extensions: &["ngp"], launchers: &["retroarch"], retroarch_core: Some("mednafen_ngp_libretro.dll") },
    EmuSystem { folder: "ngpc", platform_id: "ngpc", extensions: &["ngc"], launchers: &["retroarch"], retroarch_core: Some("mednafen_ngp_libretro.dll") },
    EmuSystem { folder: "xbox", platform_id: "xbox", extensions: &["iso", "xbe"], launchers: &["xemu"], retroarch_core: None },
    EmuSystem { folder: "xbox360", platform_id: "xbox360", extensions: &["iso", "xex"], launchers: &["xenia"], retroarch_core: None },
    EmuSystem { folder: "scummvm", platform_id: "scummvm", extensions: &["scummvm"], launchers: &["ScummVM"], retroarch_core: None },
];

fn find_emudeck_subdir(root: &Path, segments: &[&str]) -> Option<PathBuf> {
    let direct = segments.iter().fold(root.to_path_buf(), |acc, seg| acc.join(seg));
    if direct.is_dir() {
        return Some(direct);
    }
    let nested = segments
        .iter()
        .fold(root.join("Emulation"), |acc, seg| acc.join(seg));
    if nested.is_dir() {
        return Some(nested);
    }
    None
}

fn locate_launcher<'a>(
    emudeck_root: &Path,
    candidates: &'a [&'a str],
) -> Option<(PathBuf, &'a str)> {
    let dir = find_emudeck_subdir(emudeck_root, &["tools", "launchers"])?;
    for name in candidates {
        for ext in &["ps1", "bat", "cmd"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Some((candidate, *name));
            }
        }
    }
    None
}

fn lookup_emu_system(folder: &str) -> Option<&'static EmuSystem> {
    EMU_SYSTEMS.iter().find(|system| system.folder == folder)
}

fn spawn_launcher(
    launcher: &Path,
    rom_path: &Path,
    extra_args: &[&str],
) -> Result<Child, String> {
    let ext = launcher
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut command = match ext.as_str() {
        "bat" | "cmd" => {
            let mut c = Command::new("cmd");
            c.arg("/c").arg(launcher);
            for arg in extra_args {
                c.arg(arg);
            }
            c.arg(rom_path);
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell");
            c.arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(launcher);
            for arg in extra_args {
                c.arg(arg);
            }
            c.arg(rom_path);
            c
        }
        other => return Err(format!("Unsupported launcher type: {other}")),
    };
    command
        .spawn()
        .map_err(|error| format!("Unable to launch ROM: {error}"))
}

fn classify_single_region(tag: &str) -> Option<&'static str> {
    match tag.to_lowercase().as_str() {
        "japan" | "j" | "jp" | "jpn" => Some("Japan"),
        "usa" | "u" | "us" => Some("USA"),
        "europe" | "e" | "eu" | "eur" => Some("Europe"),
        "world" | "w" => Some("World"),
        "asia" => Some("Asia"),
        "korea" | "kr" => Some("Korea"),
        "china" | "cn" => Some("China"),
        "taiwan" | "tw" => Some("Taiwan"),
        "germany" | "de" | "ger" => Some("Germany"),
        "france" | "fr" | "fre" => Some("France"),
        "italy" | "it" | "ita" => Some("Italy"),
        "spain" | "es" | "spa" => Some("Spain"),
        "brazil" | "br" | "bra" => Some("Brazil"),
        "australia" | "au" | "aus" => Some("Australia"),
        "netherlands" | "nl" => Some("Netherlands"),
        "sweden" | "se" => Some("Sweden"),
        _ => None,
    }
}

fn classify_region_tag(tag: &str) -> Option<&'static str> {
    if let Some(region) = classify_single_region(tag) {
        return Some(region);
    }
    for part in tag.split(',') {
        if let Some(region) = classify_single_region(part.trim()) {
            return Some(region);
        }
    }
    None
}

fn parse_rom_filename(stem: &str) -> (String, Option<String>) {
    let mut clean = String::new();
    let mut current_paren = String::new();
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut paren_groups: Vec<String> = Vec::new();

    for ch in stem.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                if paren_depth == 1 {
                    current_paren.clear();
                    continue;
                }
            }
            ')' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    let trimmed = current_paren.trim().to_string();
                    if !trimmed.is_empty() {
                        paren_groups.push(trimmed);
                    }
                    continue;
                }
            }
            '[' => {
                bracket_depth += 1;
                continue;
            }
            ']' => {
                bracket_depth -= 1;
                continue;
            }
            _ => {}
        }

        if paren_depth == 0 && bracket_depth == 0 {
            clean.push(ch);
        } else if paren_depth >= 1 {
            current_paren.push(ch);
        }
    }

    let clean: String = clean.split_whitespace().collect::<Vec<_>>().join(" ");

    let label = paren_groups
        .iter()
        .find_map(|group| classify_region_tag(group).map(|region| region.to_string()))
        .or_else(|| paren_groups.first().cloned());

    (clean, label)
}

#[tauri::command]
pub(crate) fn scan_emudeck_roms(app: AppHandle, root: String) -> Result<Library, String> {
    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        return Err(format!(
            "EmuDeck root not found: {}",
            root_path.display()
        ));
    }
    let roms_dir = find_emudeck_subdir(&root_path, &["roms"])
        .ok_or_else(|| format!("Could not find roms folder under {}", root_path.display()))?;

    let mut settings = read_settings_from_disk(&app).unwrap_or_default();
    settings.emudeck_root = Some(root_path.to_string_lossy().to_string());
    crate::write_settings_to_disk(&app, &settings)?;

    let mut library = read_library_from_disk(&app)?;

    let already_tracked: HashSet<String> = library
        .games
        .iter()
        .flat_map(|game| {
            game.variants
                .iter()
                .map(|variant| variant.rom_path.to_lowercase())
        })
        .collect();

    for system in EMU_SYSTEMS {
        let system_dir = roms_dir.join(system.folder);
        if !system_dir.is_dir() {
            continue;
        }

        let mut groups: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

        for entry in WalkDir::new(&system_dir)
            .max_depth(6)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !system
                .extensions
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&extension))
            {
                continue;
            }
            let path_lower = path.to_string_lossy().to_lowercase();
            if already_tracked.contains(&path_lower) {
                continue;
            }

            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string();
            let (clean_title, parsed_label) = parse_rom_filename(&stem);
            let label = parsed_label.unwrap_or_else(|| "Default".to_string());
            let title_for_group = if clean_title.is_empty() {
                stem.clone()
            } else {
                clean_title.clone()
            };
            let key = title_for_group.to_lowercase();

            groups.entry(key).or_default().push((
                path.to_string_lossy().to_string(),
                label,
                title_for_group,
            ));
        }

        for (key, mut roms) in groups {
            roms.sort_by(|a, b| a.1.cmp(&b.1));

            let existing_idx = library.games.iter().position(|game| {
                game.rom_system.as_deref() == Some(system.folder)
                    && parse_rom_filename(&game.title).0.to_lowercase() == key
            });

            if let Some(idx) = existing_idx {
                let existing_paths: HashSet<String> = library.games[idx]
                    .variants
                    .iter()
                    .map(|variant| variant.rom_path.clone())
                    .collect();
                for (rom_path, label, _) in &roms {
                    if !existing_paths.contains(rom_path) {
                        library.games[idx].variants.push(GameVariant {
                            id: Uuid::new_v4().to_string(),
                            label: label.clone(),
                            rom_path: rom_path.clone(),
                            last_played_at: None,
                            play_count: 0,
                            retro_achievements: None,
                        });
                    }
                }
            } else {
                let title = roms
                    .iter()
                    .map(|(_, _, t)| t.clone())
                    .find(|t| !t.is_empty())
                    .unwrap_or_else(|| "Untitled".to_string());

                let variants: Vec<GameVariant> = roms
                    .iter()
                    .map(|(rom_path, label, _)| GameVariant {
                        id: Uuid::new_v4().to_string(),
                        label: label.clone(),
                        rom_path: rom_path.clone(),
                        last_played_at: None,
                        play_count: 0,
                        retro_achievements: None,
                    })
                    .collect();

                library.games.push(Game {
                    id: Uuid::new_v4().to_string(),
                    title,
                    executable_path: String::new(),
                    launch_args: String::new(),
                    working_directory: String::new(),
                    cover_image: None,
                    hero_image: None,
                    logo_image: None,
                    favorite: false,
                    last_played_at: None,
                    play_count: 0,
                    description: None,
                    platform: Some(system.platform_id.to_string()),
                    tags: Vec::new(),
                    rom_system: Some(system.folder.to_string()),
                    variants,
                    retro_achievements: None,
                    position: 0,
                    hidden: false,
                    preferred_achievement_variant_id: None,
                    steam_app_id: None,
                    steam_achievements: None,
                    ps3_trophies: None,
                });
            }
        }
    }

    ensure_positions(&mut library);
    library
        .games
        .sort_by_key(|game| game.title.to_lowercase());
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn launch_rom_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
) -> Result<Library, String> {
    let settings = read_settings_from_disk(&app)?;
    let emudeck_root = settings
        .emudeck_root
        .ok_or_else(|| "EmuDeck root not configured. Run Scan ROMs first.".to_string())?;
    let emudeck_path = PathBuf::from(&emudeck_root);

    let mut library = read_library_from_disk(&app)?;
    let game_idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let system = library.games[game_idx]
        .rom_system
        .clone()
        .ok_or_else(|| "Game is not ROM-based.".to_string())?;
    let variant_idx = library.games[game_idx]
        .variants
        .iter()
        .position(|variant| variant.id == variant_id)
        .ok_or_else(|| "Variant not found.".to_string())?;
    let rom_path = PathBuf::from(&library.games[game_idx].variants[variant_idx].rom_path);
    if !rom_path.exists() {
        return Err(format!(
            "ROM file no longer exists: {}",
            rom_path.display()
        ));
    }

    let definition = lookup_emu_system(&system)
        .ok_or_else(|| format!("Unknown ROM system '{system}'."))?;
    let (launcher, launcher_name) = locate_launcher(&emudeck_path, definition.launchers)
        .ok_or_else(|| {
            format!(
                "Launcher not found for system '{system}'. Tried: {} under {}",
                definition.launchers.join(", "),
                emudeck_path.display()
            )
        })?;
    let mut extra_args: Vec<&str> = Vec::new();
    if launcher_name == "retroarch" {
        if let Some(core) = definition.retroarch_core {
            extra_args.push("-L");
            extra_args.push(core);
        }
    }
    let child = spawn_launcher(&launcher, &rom_path, &extra_args)?;
    handoff_focus_to_child(&app, child, game_id.clone(), Some(variant_id.clone()));

    let now = Utc::now().to_rfc3339();
    {
        let variant = &mut library.games[game_idx].variants[variant_idx];
        variant.last_played_at = Some(now.clone());
        variant.play_count = variant.play_count.saturating_add(1);
    }
    let game = &mut library.games[game_idx];
    game.last_played_at = Some(now);
    game.play_count = game.play_count.saturating_add(1);

    write_library_to_disk(&app, &library)?;
    Ok(library)
}
