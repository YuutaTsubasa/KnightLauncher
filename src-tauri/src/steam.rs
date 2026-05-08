use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    download_to, ensure_positions, extract_xml_text, http_client, next_position,
    read_library_from_disk, read_settings_from_disk, write_library_to_disk,
    write_settings_to_disk, Achievement, ArtworkKind, Game, Library,
    RetroAchievementsLink,
};

pub(crate) fn find_default_steam_root() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
    ];
    for candidate in candidates.iter() {
        let path = PathBuf::from(candidate);
        if path.is_dir() && path.join("steamapps").is_dir() {
            return Some(path);
        }
    }
    None
}

fn extract_vdf_value(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let start = content.find(&needle)?;
    let after = &content[start + needle.len()..];
    let q1 = after.find('"')?;
    let after = &after[q1 + 1..];
    let q2 = after.find('"')?;
    Some(after[..q2].to_string())
}

pub(crate) fn detect_steam_user_id(steam_root: &Path) -> Option<String> {
    let path = steam_root.join("config").join("loginusers.vdf");
    let content = fs::read_to_string(&path).ok()?;

    let mut current_id: Option<String> = None;
    let mut first_id: Option<String> = None;
    let mut most_recent_id: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(stripped) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            if !stripped.contains('"')
                && stripped.len() == 17
                && stripped.starts_with("7656")
                && stripped.chars().all(|c| c.is_ascii_digit())
            {
                current_id = Some(stripped.to_string());
                if first_id.is_none() {
                    first_id = current_id.clone();
                }
                continue;
            }
        }
        if trimmed.starts_with("\"MostRecent\"") && trimmed.contains("\"1\"") {
            if let Some(ref id) = current_id {
                most_recent_id = Some(id.clone());
            }
        }
    }

    most_recent_id.or(first_id)
}

fn parse_library_folders(path: &Path) -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    let mut cursor = content.as_str();
    while let Some(idx) = cursor.find("\"path\"") {
        let after = &cursor[idx + 6..];
        let Some(q1) = after.find('"') else { break; };
        let after = &after[q1 + 1..];
        let Some(q2) = after.find('"') else { break; };
        let raw = &after[..q2];
        let normalized = raw.replace("\\\\", "\\");
        let candidate = PathBuf::from(normalized);
        if candidate.join("steamapps").is_dir() {
            paths.push(candidate);
        }
        cursor = &after[q2 + 1..];
    }
    paths
}

#[tauri::command]
pub(crate) fn scan_steam_library(
    app: AppHandle,
    steam_root: Option<String>,
) -> Result<Library, String> {
    let provided = steam_root
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let saved = read_settings_from_disk(&app)
        .ok()
        .and_then(|settings| settings.steam_root)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let root = if let Some(value) = provided.clone() {
        PathBuf::from(value)
    } else if let Some(value) = saved {
        PathBuf::from(value)
    } else if let Some(value) = find_default_steam_root() {
        value
    } else {
        return Err(
            "Steam install not found. Please pick the Steam folder manually.".to_string(),
        );
    };

    if !root.is_dir() || !root.join("steamapps").is_dir() {
        return Err(format!(
            "Steam install not found under {}.",
            root.display()
        ));
    }

    let mut settings = read_settings_from_disk(&app).unwrap_or_default();
    settings.steam_root = Some(root.to_string_lossy().to_string());
    if settings
        .steam_user_id
        .as_deref()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        if let Some(detected) = detect_steam_user_id(&root) {
            settings.steam_user_id = Some(detected);
        }
    }
    write_settings_to_disk(&app, &settings)?;

    let mut libraries: Vec<PathBuf> = Vec::new();
    libraries.push(root.clone());
    let library_folders_vdf = root.join("steamapps").join("libraryfolders.vdf");
    for extra in parse_library_folders(&library_folders_vdf) {
        if !libraries.iter().any(|existing| existing == &extra) {
            libraries.push(extra);
        }
    }

    let mut library = read_library_from_disk(&app)?;

    {
        let mut seen: HashSet<String> = HashSet::new();
        library.games.retain(|game| {
            let Some(app_id) = game.steam_app_id.as_ref() else {
                return true;
            };
            if seen.contains(app_id) {
                return false;
            }
            seen.insert(app_id.clone());
            true
        });
    }

    let already_app_ids: HashSet<String> = library
        .games
        .iter()
        .filter_map(|game| game.steam_app_id.clone())
        .collect();

    for steam_lib in libraries {
        let steamapps = steam_lib.join("steamapps");
        let entries = match fs::read_dir(&steamapps) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("appmanifest_") || !name_str.ends_with(".acf") {
                continue;
            }
            let manifest_path = entry.path();
            let Ok(content) = fs::read_to_string(&manifest_path) else {
                continue;
            };

            let Some(app_id) = extract_vdf_value(&content, "appid") else {
                continue;
            };
            if already_app_ids.contains(&app_id) {
                continue;
            }

            let state_flags = extract_vdf_value(&content, "StateFlags")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            if state_flags & 4 == 0 {
                continue;
            }

            let title = extract_vdf_value(&content, "name")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("Steam app {app_id}"));

            let position = next_position(&library);
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
                platform: Some("Steam".to_string()),
                tags: Vec::new(),
                rom_system: None,
                variants: Vec::new(),
                retro_achievements: None,
                position,
                hidden: false,
                preferred_achievement_variant_id: None,
                steam_app_id: Some(app_id),
                steam_achievements: None,
                ps3_trophies: None,
            });
        }
    }

    ensure_positions(&mut library);
    library
        .games
        .sort_by_key(|game| game.title.to_lowercase());
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

pub(crate) struct RawSteamAchievement {
    pub apiname: String,
    pub name: String,
    pub description: String,
    pub icon_closed: String,
    pub icon_open: String,
    pub closed: bool,
    pub unlock_ts: Option<i64>,
}

pub(crate) fn parse_steam_achievements_xml(
    xml: &str,
) -> Result<(String, Vec<RawSteamAchievement>), String> {
    if xml.contains("<error>") || xml.contains("This profile is private") {
        return Err(
            "Steam profile or game stats are private. Please set your profile to Public."
                .to_string(),
        );
    }
    let game_name = extract_xml_text(xml, "gameName").unwrap_or_default();

    let mut achievements = Vec::new();
    let mut cursor = 0usize;
    let needle = "<achievement closed=";

    while let Some(found) = xml[cursor..].find(needle) {
        let abs = cursor + found;
        let after_open = &xml[abs + needle.len()..];
        let closed = after_open.starts_with("\"1\"");
        let block_end = match xml[abs..].find("</achievement>") {
            Some(p) => abs + p,
            None => break,
        };
        let block = &xml[abs..block_end];

        let apiname = extract_xml_text(block, "apiname").unwrap_or_default();
        let name = extract_xml_text(block, "name").unwrap_or_default();
        let description = extract_xml_text(block, "description").unwrap_or_default();
        let icon_closed = extract_xml_text(block, "iconClosed").unwrap_or_default();
        let icon_open = extract_xml_text(block, "iconOpen").unwrap_or_default();
        let unlock_ts = extract_xml_text(block, "unlockTimestamp")
            .and_then(|s| s.parse::<i64>().ok());

        achievements.push(RawSteamAchievement {
            apiname,
            name,
            description,
            icon_closed,
            icon_open,
            closed,
            unlock_ts,
        });

        cursor = block_end + "</achievement>".len();
    }

    Ok((game_name, achievements))
}

fn steam_cache_dir(app: &AppHandle, app_id: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?
        .join("steam_achievements")
        .join(app_id);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create Steam cache dir: {error}"))?;
    Ok(dir)
}

fn fetch_steam_achievements_link(
    app: &AppHandle,
    steamid: &str,
    app_id: &str,
) -> Result<RetroAchievementsLink, String> {
    let url = format!(
        "https://steamcommunity.com/profiles/{steamid}/stats/{app_id}/achievements/?xml=1"
    );
    let response = http_client()?
        .get(&url)
        .header("User-Agent", "KnightLauncher")
        .send()
        .map_err(|error| format!("Unable to reach Steam community: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Steam community returned HTTP {status}."));
    }
    let body = response
        .text()
        .map_err(|error| format!("Unable to read Steam response: {error}"))?;

    let (game_name, raw) = parse_steam_achievements_xml(&body)?;

    let cache_dir = steam_cache_dir(app, app_id)?;
    let mut achievements: Vec<Achievement> = Vec::with_capacity(raw.len());
    let mut earned: u32 = 0;

    for (idx, item) in raw.iter().enumerate() {
        let safe_name: String = item
            .apiname
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let stem = if safe_name.is_empty() {
            format!("ach-{idx}")
        } else {
            safe_name
        };

        let badge_path = if !item.icon_closed.is_empty() {
            download_to(
                &item.icon_closed,
                &cache_dir.join(format!("{stem}-earned.png")),
                ArtworkKind::Badge,
            )
            .ok()
        } else {
            None
        };
        let badge_locked_path = if !item.icon_open.is_empty() {
            download_to(
                &item.icon_open,
                &cache_dir.join(format!("{stem}-locked.png")),
                ArtworkKind::Badge,
            )
            .ok()
        } else {
            None
        };

        let earned_date = if item.closed {
            earned += 1;
            item.unlock_ts
                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
                .map(|dt| dt.to_rfc3339())
                .or_else(|| Some(Utc::now().to_rfc3339()))
        } else {
            None
        };

        achievements.push(Achievement {
            id: idx as u32,
            title: item.name.clone(),
            description: item.description.clone(),
            points: 0,
            badge_url: item.icon_closed.clone(),
            badge_locked_url: item.icon_open.clone(),
            badge_path,
            badge_locked_path,
            earned_date,
            display_order: idx as u32,
        });
    }

    let total = achievements.len() as u32;
    let app_id_num = app_id.parse::<u32>().unwrap_or(0);

    Ok(RetroAchievementsLink {
        game_id: app_id_num,
        title: game_name,
        console_id: 0,
        console_name: "Steam".to_string(),
        icon_path: None,
        icon_url: None,
        box_art_url: None,
        title_url: None,
        ingame_url: None,
        achievements_total: total,
        achievements_earned: earned,
        points_total: 0,
        points_earned: 0,
        achievements,
        last_synced_at: Some(Utc::now().to_rfc3339()),
    })
}

#[tauri::command]
pub(crate) fn steam_achievements_link_game(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let settings = read_settings_from_disk(&app)?;
    let steamid = settings
        .steam_user_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Steam user not detected. Run Scan Steam library first while signed in.".to_string()
        })?;

    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|g| g.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let app_id = library.games[idx]
        .steam_app_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Game is not a Steam game.".to_string())?;

    let link = fetch_steam_achievements_link(&app, &steamid, &app_id)?;
    library.games[idx].steam_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn steam_achievements_refresh(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let settings = read_settings_from_disk(&app)?;
    let steamid = settings
        .steam_user_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Steam user not detected.".to_string())?;

    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|g| g.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let app_id = library.games[idx]
        .steam_app_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Game is not a Steam game.".to_string())?;

    let link = fetch_steam_achievements_link(&app, &steamid, &app_id)?;
    library.games[idx].steam_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn steam_achievements_unlink(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|g| g.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    library.games[idx].steam_achievements = None;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_steam_achievements_xml_decodes_cdata_and_status() {
        let xml = r#"<?xml version="1.0"?>
<playerstats>
  <game>
    <gameName><![CDATA[Sample Game]]></gameName>
    <achievements>
      <achievement closed="1">
        <apiname>FIRST</apiname>
        <name><![CDATA[First Win]]></name>
        <description><![CDATA[Win a match]]></description>
        <iconClosed><![CDATA[https://example.test/closed.jpg]]></iconClosed>
        <iconOpen><![CDATA[https://example.test/open.jpg]]></iconOpen>
        <unlockTimestamp>1700000000</unlockTimestamp>
      </achievement>
      <achievement closed="0">
        <apiname>SECOND</apiname>
        <name>Second</name>
        <description>Lose a match</description>
        <iconClosed>https://example.test/2c.jpg</iconClosed>
        <iconOpen>https://example.test/2o.jpg</iconOpen>
      </achievement>
    </achievements>
  </game>
</playerstats>"#;

        let (game, achievements) =
            parse_steam_achievements_xml(xml).expect("parse");
        assert_eq!(game, "Sample Game");
        assert_eq!(achievements.len(), 2);

        let first = &achievements[0];
        assert_eq!(first.apiname, "FIRST");
        assert_eq!(first.name, "First Win");
        assert_eq!(first.icon_closed, "https://example.test/closed.jpg");
        assert!(first.closed);
        assert_eq!(first.unlock_ts, Some(1700000000));

        let second = &achievements[1];
        assert!(!second.closed);
        assert_eq!(second.unlock_ts, None);
    }

    #[test]
    fn parse_steam_achievements_xml_rejects_private_profile() {
        let xml = "<error>This profile is private</error>";
        assert!(parse_steam_achievements_xml(xml).is_err());
    }
}
