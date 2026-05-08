use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    download_to, http_client, read_library_from_disk, read_settings_from_disk,
    write_library_to_disk, Achievement, ArtworkKind, Library, RetroAchievementsLink,
};

fn ra_console_id(platform_id: &str) -> Option<u32> {
    match platform_id {
        "md" => Some(1),
        "n64" => Some(2),
        "sfc" => Some(3),
        "gb" => Some(4),
        "gba" => Some(5),
        "gbc" => Some(6),
        "fc" => Some(7),
        "segacd" => Some(9),
        "32x" => Some(10),
        "sms" => Some(11),
        "ps1" => Some(12),
        "ngpc" => Some(14),
        "gg" => Some(15),
        "gc" => Some(16),
        "ds" => Some(18),
        "ps2" => Some(21),
        "sat" => Some(39),
        "dc" => Some(40),
        "psp" => Some(41),
        _ => None,
    }
}

fn ra_credentials(app: &AppHandle) -> Result<(String, String), String> {
    let settings = read_settings_from_disk(app)?;
    let user = settings
        .retro_achievements_user
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RetroAchievements username not configured.".to_string())?;
    let api_key = settings
        .retro_achievements_api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "RetroAchievements API key not configured.".to_string())?;
    Ok((user, api_key))
}

fn ra_get<T: for<'de> Deserialize<'de>>(
    app: &AppHandle,
    endpoint: &str,
    extra_params: &[(&str, &str)],
) -> Result<T, String> {
    let (user, api_key) = ra_credentials(app)?;
    let url = format!("https://retroachievements.org/API/{endpoint}");
    let mut params: Vec<(&str, &str)> = vec![
        ("z", user.as_str()),
        ("u", user.as_str()),
        ("y", api_key.as_str()),
    ];
    params.extend_from_slice(extra_params);

    let response = http_client()?
        .get(&url)
        .query(&params)
        .send()
        .map_err(|error| format!("RetroAchievements request failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("RetroAchievements API returned HTTP {status}."));
    }

    response
        .json::<T>()
        .map_err(|error| format!("Unable to parse RetroAchievements response: {error}"))
}

fn ra_cache_dir(app: &AppHandle, ra_game_id: u32) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?
        .join("retroachievements")
        .join(ra_game_id.to_string());
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create RA cache dir: {error}"))?;
    Ok(dir)
}

#[derive(Debug, Deserialize)]
struct RaGameListEntry {
    #[serde(rename = "ID")]
    id: u32,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "ConsoleID")]
    console_id: u32,
    #[serde(rename = "ConsoleName")]
    console_name: String,
    #[serde(rename = "ImageIcon", default)]
    image_icon: Option<String>,
    #[serde(rename = "NumAchievements", default)]
    num_achievements: u32,
    #[serde(rename = "Points", default)]
    points: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaGameSearchResult {
    pub id: u32,
    pub title: String,
    pub console_id: u32,
    pub console_name: String,
    pub icon_url: Option<String>,
    pub num_achievements: u32,
    pub points: u32,
}

#[derive(Debug, Deserialize)]
struct RaAchievementResponse {
    #[serde(rename = "ID")]
    id: u32,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "Points", default)]
    points: u32,
    #[serde(rename = "BadgeName")]
    badge_name: String,
    #[serde(rename = "DisplayOrder", default)]
    display_order: u32,
    #[serde(rename = "DateEarned", default)]
    date_earned: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RaGameInfoResponse {
    #[serde(rename = "ID")]
    id: u32,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "ConsoleID")]
    console_id: u32,
    #[serde(rename = "ConsoleName")]
    console_name: String,
    #[serde(rename = "ImageIcon", default)]
    image_icon: Option<String>,
    #[serde(rename = "ImageTitle", default)]
    image_title: Option<String>,
    #[serde(rename = "ImageIngame", default, alias = "ImageInGame")]
    image_ingame: Option<String>,
    #[serde(rename = "ImageBoxArt", default)]
    image_box_art: Option<String>,
    #[serde(rename = "NumAchievements", default)]
    num_achievements: u32,
    #[serde(rename = "NumAwardedToUser", default)]
    num_awarded_to_user: u32,
    #[serde(rename = "Achievements", default)]
    achievements: HashMap<String, RaAchievementResponse>,
}

fn ra_fetch_link(app: &AppHandle, ra_game_id: u32) -> Result<RetroAchievementsLink, String> {
    let game_id_str = ra_game_id.to_string();
    let response = ra_get::<RaGameInfoResponse>(
        app,
        "API_GetGameInfoAndUserProgress.php",
        &[("g", game_id_str.as_str())],
    )?;

    let cache_dir = ra_cache_dir(app, ra_game_id)?;

    let mut achievements: Vec<RaAchievementResponse> =
        response.achievements.into_values().collect();
    achievements.sort_by_key(|achievement| achievement.display_order);

    let mut points_total: u32 = 0;
    let mut points_earned: u32 = 0;
    let mut achievement_records: Vec<Achievement> = Vec::with_capacity(achievements.len());

    for ach in achievements {
        let badge_url = format!(
            "https://media.retroachievements.org/Badge/{}.png",
            ach.badge_name
        );
        let badge_locked_url = format!(
            "https://media.retroachievements.org/Badge/{}_lock.png",
            ach.badge_name
        );
        let badge_path = download_to(
            &badge_url,
            &cache_dir.join(format!("{}.png", ach.badge_name)),
            ArtworkKind::Badge,
        )
        .ok();
        let badge_locked_path = download_to(
            &badge_locked_url,
            &cache_dir.join(format!("{}_lock.png", ach.badge_name)),
            ArtworkKind::Badge,
        )
        .ok();

        points_total = points_total.saturating_add(ach.points);
        if ach.date_earned.is_some() {
            points_earned = points_earned.saturating_add(ach.points);
        }

        achievement_records.push(Achievement {
            id: ach.id,
            title: ach.title,
            description: ach.description,
            points: ach.points,
            badge_url,
            badge_locked_url,
            badge_path,
            badge_locked_path,
            earned_date: ach.date_earned,
            display_order: ach.display_order,
        });
    }

    let icon_url = response
        .image_icon
        .as_ref()
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("https://retroachievements.org{relative}"));
    let box_art_url = response
        .image_box_art
        .as_ref()
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("https://retroachievements.org{relative}"));
    let title_url = response
        .image_title
        .as_ref()
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("https://retroachievements.org{relative}"));
    let ingame_url = response
        .image_ingame
        .as_ref()
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("https://retroachievements.org{relative}"));

    let icon_path = icon_url
        .as_ref()
        .and_then(|url| download_to(url, &cache_dir.join("icon.png"), ArtworkKind::Logo).ok());

    Ok(RetroAchievementsLink {
        game_id: response.id,
        title: response.title,
        console_id: response.console_id,
        console_name: response.console_name,
        icon_path,
        icon_url,
        box_art_url,
        title_url,
        ingame_url,
        achievements_total: response.num_achievements,
        achievements_earned: response.num_awarded_to_user,
        points_total,
        points_earned,
        achievements: achievement_records,
        last_synced_at: Some(Utc::now().to_rfc3339()),
    })
}

#[tauri::command]
pub(crate) fn retroachievements_search_games(
    app: AppHandle,
    query: String,
    platform_id: String,
) -> Result<Vec<RaGameSearchResult>, String> {
    let normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let console_id = ra_console_id(&platform_id).ok_or_else(|| {
        format!("Platform '{platform_id}' is not supported by RetroAchievements.")
    })?;
    let console_id_str = console_id.to_string();
    let games = ra_get::<Vec<RaGameListEntry>>(
        &app,
        "API_GetGameList.php",
        &[("i", console_id_str.as_str()), ("f", "1")],
    )?;

    let results: Vec<RaGameSearchResult> = games
        .into_iter()
        .filter(|entry| entry.num_achievements > 0)
        .filter(|entry| entry.title.to_lowercase().contains(&normalized))
        .take(40)
        .map(|entry| RaGameSearchResult {
            icon_url: entry
                .image_icon
                .map(|relative| format!("https://retroachievements.org{relative}")),
            id: entry.id,
            title: entry.title,
            console_id: entry.console_id,
            console_name: entry.console_name,
            num_achievements: entry.num_achievements,
            points: entry.points,
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub(crate) fn retroachievements_link_game(
    app: AppHandle,
    game_id: String,
    ra_game_id: u32,
) -> Result<Library, String> {
    let link = ra_fetch_link(&app, ra_game_id)?;
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    library.games[idx].retro_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn retroachievements_refresh(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let ra_game_id = library.games[idx]
        .retro_achievements
        .as_ref()
        .map(|link| link.game_id)
        .ok_or_else(|| "Game is not linked to RetroAchievements.".to_string())?;
    let link = ra_fetch_link(&app, ra_game_id)?;
    library.games[idx].retro_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn retroachievements_unlink(
    app: AppHandle,
    game_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    library.games[idx].retro_achievements = None;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn retroachievements_link_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
    ra_game_id: u32,
) -> Result<Library, String> {
    let link = ra_fetch_link(&app, ra_game_id)?;
    let mut library = read_library_from_disk(&app)?;
    let game_idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let variant_idx = library.games[game_idx]
        .variants
        .iter()
        .position(|variant| variant.id == variant_id)
        .ok_or_else(|| "Variant not found.".to_string())?;
    library.games[game_idx].variants[variant_idx].retro_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn retroachievements_refresh_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let game_idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let variant_idx = library.games[game_idx]
        .variants
        .iter()
        .position(|variant| variant.id == variant_id)
        .ok_or_else(|| "Variant not found.".to_string())?;
    let ra_game_id = library.games[game_idx].variants[variant_idx]
        .retro_achievements
        .as_ref()
        .map(|link| link.game_id)
        .ok_or_else(|| "Variant is not linked to RetroAchievements.".to_string())?;
    let link = ra_fetch_link(&app, ra_game_id)?;
    library.games[game_idx].variants[variant_idx].retro_achievements = Some(link);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
pub(crate) fn retroachievements_unlink_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let game_idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    let variant_idx = library.games[game_idx]
        .variants
        .iter()
        .position(|variant| variant.id == variant_id)
        .ok_or_else(|| "Variant not found.".to_string())?;
    library.games[game_idx].variants[variant_idx].retro_achievements = None;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}
