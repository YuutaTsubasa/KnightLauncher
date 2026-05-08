use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration as ChronoDuration, NaiveDate, TimeZone, Utc};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    ensure_positions, extract_xml_text, read_library_from_disk, read_settings_from_disk,
    save_bytes_as_webp, write_library_to_disk, write_settings_to_disk, Achievement,
    AppSettings, ArtworkKind, Game, GameVariant, Library, RetroAchievementsLink,
};

pub(crate) struct Ps3TrophyDef {
    pub id: u32,
    pub hidden: bool,
    pub ttype: String,
    pub name: String,
    pub detail: String,
}

pub(crate) struct Ps3TrophySet {
    pub np_comm_id: String,
    pub title: String,
    pub trophies: Vec<Ps3TrophyDef>,
}

pub(crate) struct TropUsrState {
    pub earned: bool,
    pub earned_at: Option<String>,
}

pub(crate) fn is_ps3_platform(platform: Option<&str>) -> bool {
    let Some(p) = platform else {
        return false;
    };
    let normalized: String = p
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    matches!(normalized.as_str(), "ps3" | "playstation3")
}

pub(crate) fn read_param_sfo_title(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    parse_param_sfo_title(&bytes)
}

pub(crate) fn parse_param_sfo_title(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 20 || &bytes[0..4] != b"\0PSF" {
        return None;
    }
    let key_table_start =
        u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let data_table_start =
        u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let entries =
        u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;

    for i in 0..entries {
        let idx = 20 + i * 16;
        if idx + 16 > bytes.len() {
            return None;
        }
        let key_offset = u16::from_le_bytes([bytes[idx], bytes[idx + 1]]) as usize;
        let data_len = u32::from_le_bytes([
            bytes[idx + 4],
            bytes[idx + 5],
            bytes[idx + 6],
            bytes[idx + 7],
        ]) as usize;
        let data_offset = u32::from_le_bytes([
            bytes[idx + 12],
            bytes[idx + 13],
            bytes[idx + 14],
            bytes[idx + 15],
        ]) as usize;

        let key_start = key_table_start + key_offset;
        if key_start >= bytes.len() {
            continue;
        }
        let key_end = bytes[key_start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| key_start + p)
            .unwrap_or(bytes.len());
        let key = std::str::from_utf8(&bytes[key_start..key_end]).ok()?;

        if key == "TITLE" {
            let data_start = data_table_start + data_offset;
            let data_end = (data_start + data_len).min(bytes.len());
            if data_start >= data_end {
                return None;
            }
            let data = &bytes[data_start..data_end];
            let trimmed_end = data
                .iter()
                .rposition(|&b| b != 0)
                .map(|p| p + 1)
                .unwrap_or(0);
            return std::str::from_utf8(&data[..trimmed_end])
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }
    None
}

fn parse_sfm_attr(header: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = header.find(&needle)? + needle.len();
    let end = header[start..].find('"')?;
    Some(header[start..start + end].to_string())
}

pub(crate) fn parse_tropconf(path: &Path) -> Option<Ps3TrophySet> {
    let text = fs::read_to_string(path).ok()?;
    parse_tropconf_str(&text)
}

pub(crate) fn parse_tropconf_str(text: &str) -> Option<Ps3TrophySet> {
    let title = extract_xml_text(text, "title-name").unwrap_or_default();
    let np_comm_id = extract_xml_text(text, "npcommid").unwrap_or_default();

    let mut trophies = Vec::new();
    let mut cursor = 0;
    while let Some(rel_open) = text[cursor..].find("<trophy ") {
        let abs_open = cursor + rel_open;
        let Some(rel_open_end) = text[abs_open..].find('>') else {
            break;
        };
        let abs_open_end = abs_open + rel_open_end;
        let header_str = &text[abs_open..=abs_open_end];
        let body_start = abs_open_end + 1;
        let Some(rel_close) = text[body_start..].find("</trophy>") else {
            break;
        };
        let abs_close = body_start + rel_close;
        let body = &text[body_start..abs_close];

        let id = parse_sfm_attr(header_str, "id")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let hidden = parse_sfm_attr(header_str, "hidden")
            .map(|s| s == "yes")
            .unwrap_or(false);
        let ttype = parse_sfm_attr(header_str, "ttype").unwrap_or_else(|| "B".to_string());
        let name = extract_xml_text(body, "name").unwrap_or_default();
        let detail = extract_xml_text(body, "detail").unwrap_or_default();

        trophies.push(Ps3TrophyDef {
            id,
            hidden,
            ttype,
            name,
            detail,
        });
        cursor = abs_close + "</trophy>".len();
    }

    Some(Ps3TrophySet {
        np_comm_id,
        title,
        trophies,
    })
}

pub(crate) fn sce_rtc_tick_to_iso(ticks: u64) -> Option<String> {
    if ticks == 0 {
        return None;
    }
    let secs = (ticks / 1_000_000) as i64;
    let micros = (ticks % 1_000_000) as i64;
    let epoch = NaiveDate::from_ymd_opt(1, 1, 1)?.and_hms_opt(0, 0, 0)?;
    let dt = epoch
        .checked_add_signed(ChronoDuration::seconds(secs))?
        .checked_add_signed(ChronoDuration::microseconds(micros))?;
    Some(Utc.from_utc_datetime(&dt).to_rfc3339())
}

pub(crate) fn parse_tropusr_state(path: &Path) -> HashMap<u32, TropUsrState> {
    let Ok(bytes) = fs::read(path) else {
        return HashMap::new();
    };
    parse_tropusr_state_bytes(&bytes)
}

pub(crate) fn parse_tropusr_state_bytes(bytes: &[u8]) -> HashMap<u32, TropUsrState> {
    let mut map = HashMap::new();
    if bytes.len() < 48 || bytes[0..4] != [0x81, 0x8F, 0x54, 0xAD] {
        return map;
    }
    let tables_count =
        u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let header_size = 48usize;
    let table_header_size = 32usize;

    for t in 0..tables_count {
        let off = header_size + t * table_header_size;
        if off + table_header_size > bytes.len() {
            break;
        }
        let tag =
            u32::from_be_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
        if tag != 6 {
            continue;
        }
        let ent_size = u32::from_be_bytes([
            bytes[off + 4],
            bytes[off + 5],
            bytes[off + 6],
            bytes[off + 7],
        ]) as usize;
        let entry_count = u32::from_be_bytes([
            bytes[off + 12],
            bytes[off + 13],
            bytes[off + 14],
            bytes[off + 15],
        ]) as usize;
        let data_off = u64::from_be_bytes([
            bytes[off + 16],
            bytes[off + 17],
            bytes[off + 18],
            bytes[off + 19],
            bytes[off + 20],
            bytes[off + 21],
            bytes[off + 22],
            bytes[off + 23],
        ]) as usize;

        let stride = ent_size + 16;
        for i in 0..entry_count {
            let e = data_off + i * stride;
            if e + 0x28 > bytes.len() {
                break;
            }
            let trophy_id = u32::from_be_bytes([
                bytes[e + 8],
                bytes[e + 9],
                bytes[e + 10],
                bytes[e + 11],
            ]);
            let earned_flag = u32::from_be_bytes([
                bytes[e + 0x14],
                bytes[e + 0x15],
                bytes[e + 0x16],
                bytes[e + 0x17],
            ]);
            let earned = earned_flag != 0;
            let earned_at = if earned && e + 0x28 <= bytes.len() {
                let tick = u64::from_be_bytes([
                    bytes[e + 0x20],
                    bytes[e + 0x21],
                    bytes[e + 0x22],
                    bytes[e + 0x23],
                    bytes[e + 0x24],
                    bytes[e + 0x25],
                    bytes[e + 0x26],
                    bytes[e + 0x27],
                ]);
                sce_rtc_tick_to_iso(tick)
            } else {
                None
            };
            map.insert(trophy_id, TropUsrState { earned, earned_at });
        }
    }
    map
}

fn ps3_trophy_points(ttype: &str) -> u32 {
    match ttype {
        "P" => 180,
        "G" => 90,
        "S" => 30,
        _ => 15,
    }
}

fn ps3_trophy_cache_dir(app: &AppHandle, np_comm_id: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Unable to resolve cache dir: {error}"))?;
    let dir = base.join("cache").join("ps3_trophies").join(np_comm_id);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create cache dir: {error}"))?;
    Ok(dir)
}

fn copy_trophy_icon(src_dir: &Path, cache_dir: &Path, trophy_id: u32) -> Option<String> {
    let src = src_dir.join(format!("TROP{:03}.PNG", trophy_id));
    if !src.is_file() {
        return None;
    }
    let dest = cache_dir.join(format!("TROP{:03}.PNG", trophy_id));
    if !dest.is_file() {
        if let Err(error) = fs::copy(&src, &dest) {
            eprintln!(
                "copy trophy icon {} -> {}: {error}",
                src.display(),
                dest.display()
            );
            return None;
        }
    }
    let kind = ArtworkKind::Badge;
    let bytes = fs::read(&dest).ok()?;
    let _ = save_bytes_as_webp(&bytes, &dest, kind);
    let webp = dest.with_extension("webp");
    if webp.is_file() {
        Some(webp.to_string_lossy().to_string())
    } else {
        Some(dest.to_string_lossy().to_string())
    }
}

fn build_ps3_trophy_link(
    app: &AppHandle,
    folder: &Path,
    set: &Ps3TrophySet,
    states: &HashMap<u32, TropUsrState>,
) -> Result<RetroAchievementsLink, String> {
    let cache_dir = ps3_trophy_cache_dir(app, &set.np_comm_id)?;
    let now = Utc::now().to_rfc3339();

    let mut achievements: Vec<Achievement> = Vec::with_capacity(set.trophies.len());
    let mut earned_count: u32 = 0;
    let mut points_total: u32 = 0;
    let mut points_earned: u32 = 0;

    for trophy in &set.trophies {
        let badge_path = copy_trophy_icon(folder, &cache_dir, trophy.id);
        let points = ps3_trophy_points(&trophy.ttype);
        points_total += points;
        let state = states.get(&trophy.id);
        let is_earned = state.map(|s| s.earned).unwrap_or(false);
        let earned_date = if is_earned {
            earned_count += 1;
            points_earned += points;
            state.and_then(|s| s.earned_at.clone())
        } else {
            None
        };
        let title = if trophy.hidden && !is_earned {
            "Hidden Trophy".to_string()
        } else {
            trophy.name.clone()
        };
        let description = if trophy.hidden && !is_earned {
            String::new()
        } else {
            trophy.detail.clone()
        };
        achievements.push(Achievement {
            id: trophy.id,
            title,
            description,
            points,
            badge_url: format!("[{}]", trophy.ttype),
            badge_locked_url: format!("[{}]", trophy.ttype),
            badge_path: badge_path.clone(),
            badge_locked_path: badge_path,
            earned_date,
            display_order: trophy.id,
        });
    }

    Ok(RetroAchievementsLink {
        game_id: 0,
        title: set.title.clone(),
        console_id: 0,
        console_name: "PS3".to_string(),
        icon_path: None,
        icon_url: None,
        box_art_url: None,
        title_url: None,
        ingame_url: None,
        achievements_total: set.trophies.len() as u32,
        achievements_earned: earned_count,
        points_total,
        points_earned,
        achievements,
        last_synced_at: Some(now),
    })
}

fn normalize_match_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn detect_rpcs3_trophy_root(settings: &AppSettings) -> Option<PathBuf> {
    if let Some(stored) = settings
        .rpcs3_trophy_root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let path = PathBuf::from(stored);
        if path.is_dir() {
            return Some(path);
        }
    }
    let games_root = settings
        .rpcs3_games_root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let dev_hdd0 = PathBuf::from(games_root).parent()?.to_path_buf();
    let home = dev_hdd0.join("home");
    if !home.is_dir() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = fs::read_dir(&home)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("trophy"))
        .filter(|path| path.is_dir())
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

fn find_trophy_folder_for_game(
    trophy_root: &Path,
    game_title: &str,
) -> Option<(PathBuf, Ps3TrophySet)> {
    let target = normalize_match_title(game_title);
    if target.is_empty() {
        return None;
    }
    let entries = fs::read_dir(trophy_root).ok()?;
    let mut best: Option<(PathBuf, Ps3TrophySet)> = None;
    for entry in entries.filter_map(Result::ok) {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let sfm = folder.join("TROPCONF.SFM");
        if !sfm.is_file() {
            continue;
        }
        let Some(set) = parse_tropconf(&sfm) else {
            continue;
        };
        let candidate = normalize_match_title(&set.title);
        if candidate.is_empty() {
            continue;
        }
        let exact = candidate == target;
        let contains =
            candidate.contains(&target) || target.contains(&candidate);
        if exact {
            return Some((folder, set));
        }
        if contains && best.is_none() {
            best = Some((folder, set));
        }
    }
    best
}

fn list_trophy_set_titles(trophy_root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(trophy_root) else {
        return Vec::new();
    };
    let mut titles = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let folder = entry.path();
        let sfm = folder.join("TROPCONF.SFM");
        if !sfm.is_file() {
            continue;
        }
        if let Some(set) = parse_tropconf(&sfm) {
            if !set.title.is_empty() {
                titles.push(set.title);
            }
        }
    }
    titles.sort();
    titles.dedup();
    titles
}

fn ps3_link_for_game(
    app: &AppHandle,
    game_title: &str,
) -> Result<RetroAchievementsLink, String> {
    let settings = read_settings_from_disk(app)?;
    let trophy_root = detect_rpcs3_trophy_root(&settings).ok_or_else(|| {
        "RPCS3 trophy folder not found. Run Scan RPCS3 Games first so the launcher knows where dev_hdd0 is.".to_string()
    })?;
    let (folder, set) = find_trophy_folder_for_game(&trophy_root, game_title)
        .ok_or_else(|| {
            let titles = list_trophy_set_titles(&trophy_root);
            if titles.is_empty() {
                format!(
                    "No trophy sets found under {}.",
                    trophy_root.display()
                )
            } else {
                format!(
                    "No trophy set matched \"{}\". Available: {}",
                    game_title,
                    titles.join(", ")
                )
            }
        })?;
    let earned = {
        let usr = folder.join("TROPUSR.DAT");
        if usr.is_file() {
            parse_tropusr_state(&usr)
        } else {
            HashMap::new()
        }
    };
    build_ps3_trophy_link(app, &folder, &set, &earned)
}

#[tauri::command]
pub(crate) fn scan_rpcs3_games(app: AppHandle, root: String) -> Result<Library, String> {
    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        return Err(format!(
            "RPCS3 game folder not found: {}",
            root_path.display()
        ));
    }

    let mut settings = read_settings_from_disk(&app).unwrap_or_default();
    settings.rpcs3_games_root = Some(root_path.to_string_lossy().to_string());
    write_settings_to_disk(&app, &settings)?;

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

    let entries = fs::read_dir(&root_path)
        .map_err(|error| format!("Unable to read {}: {error}", root_path.display()))?;

    for entry in entries.filter_map(Result::ok) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let title_id = entry.file_name().to_string_lossy().to_string();
        let eboot = dir.join("USRDIR").join("EBOOT.BIN");
        if !eboot.is_file() {
            continue;
        }
        let eboot_str = eboot.to_string_lossy().to_string();
        if already_tracked.contains(&eboot_str.to_lowercase()) {
            continue;
        }

        let sfo = dir.join("PARAM.SFO");
        let title = read_param_sfo_title(&sfo).unwrap_or_else(|| title_id.clone());

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
            platform: Some("ps3".to_string()),
            tags: Vec::new(),
            rom_system: Some("ps3".to_string()),
            variants: vec![GameVariant {
                id: Uuid::new_v4().to_string(),
                label: title_id.clone(),
                rom_path: eboot_str,
                last_played_at: None,
                play_count: 0,
                retro_achievements: None,
            }],
            retro_achievements: None,
            position: 0,
            hidden: false,
            preferred_achievement_variant_id: None,
            steam_app_id: None,
            steam_achievements: None,
            ps3_trophies: None,
        });
    }

    ensure_positions(&mut library);
    library
        .games
        .sort_by_key(|game| game.title.to_lowercase());
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn ps3_trophies_link_game(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    if !is_ps3_platform(library.games[idx].platform.as_deref()) {
        return Err("Only PS3 games can link trophies.".to_string());
    }
    let title = library.games[idx].title.clone();
    let link = ps3_link_for_game(&app, &title)?;
    library.games[idx].ps3_trophies = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn ps3_trophies_refresh(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    ps3_trophies_link_game(app, game_id)
}

#[tauri::command]
pub(crate) fn ps3_trophies_unlink(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    library.games[idx].ps3_trophies = None;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sce_rtc_tick_matches_sample_unix_time() {
        let tick: u64 = 0x00e311393dd8e6d4;
        let iso = sce_rtc_tick_to_iso(tick).expect("tick should decode");
        let dt = chrono::DateTime::parse_from_rfc3339(&iso).expect("rfc3339");
        assert_eq!(dt.timestamp(), 1778160461);
        assert_eq!(sce_rtc_tick_to_iso(0), None);
    }

    #[test]
    fn parse_tropconf_handles_sonic_fighters_sample() {
        let text = include_str!("../../references/TROPUSR/TROPCONF.SFM");
        let set = parse_tropconf_str(text).expect("parse SFM");
        assert_eq!(set.np_comm_id, "NPWR03869_00");
        assert_eq!(set.title, "Sonic the Fighters");
        assert_eq!(set.trophies.len(), 12);

        let trophy_7 = set.trophies.iter().find(|t| t.id == 7).expect("id=7");
        assert_eq!(trophy_7.ttype, "G");
        assert_eq!(trophy_7.name, "Perfect");
        assert!(!trophy_7.hidden);

        let bronze_count = set.trophies.iter().filter(|t| t.ttype == "B").count();
        let silver_count = set.trophies.iter().filter(|t| t.ttype == "S").count();
        let gold_count = set.trophies.iter().filter(|t| t.ttype == "G").count();
        assert_eq!(bronze_count, 7);
        assert_eq!(silver_count, 4);
        assert_eq!(gold_count, 1);
    }

    #[test]
    fn parse_tropusr_state_marks_only_earned_trophies() {
        let bytes = include_bytes!("../../references/TROPUSR/TROPUSR.DAT");
        let states = parse_tropusr_state_bytes(bytes);
        assert_eq!(states.len(), 12);

        let earned: Vec<u32> = (0..12)
            .filter(|id| states.get(id).map(|s| s.earned).unwrap_or(false))
            .collect();
        assert_eq!(earned, vec![7]);

        let trophy_7 = &states[&7];
        assert!(trophy_7.earned);
        let iso = trophy_7.earned_at.as_deref().expect("earned_at present");
        let dt = chrono::DateTime::parse_from_rfc3339(iso).expect("rfc3339");
        assert_eq!(dt.timestamp(), 1778160461);

        assert!(!states[&0].earned);
        assert!(states[&0].earned_at.is_none());
    }

    #[test]
    fn parse_tropusr_state_rejects_bad_magic() {
        let bytes = vec![0u8; 64];
        assert!(parse_tropusr_state_bytes(&bytes).is_empty());
    }

    #[test]
    fn parse_param_sfo_title_reads_title_field() {
        let key = b"TITLE\0";
        let value = b"My Test Game\0";
        let key_table_start: u32 = 20 + 16;
        let data_table_start: u32 = key_table_start + key.len() as u32;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\0PSF");
        bytes.extend_from_slice(&[1u8, 1, 0, 0]);
        bytes.extend_from_slice(&key_table_start.to_le_bytes());
        bytes.extend_from_slice(&data_table_start.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());

        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0x0204u16.to_le_bytes());
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());

        bytes.extend_from_slice(key);
        bytes.extend_from_slice(value);

        assert_eq!(parse_param_sfo_title(&bytes).as_deref(), Some("My Test Game"));
    }

    #[test]
    fn is_ps3_platform_normalizes_aliases() {
        assert!(is_ps3_platform(Some("ps3")));
        assert!(is_ps3_platform(Some("PS3")));
        assert!(is_ps3_platform(Some(" PlayStation 3 ")));
        assert!(is_ps3_platform(Some("playstation3")));
        assert!(!is_ps3_platform(Some("ps2")));
        assert!(!is_ps3_platform(None));
    }
}
