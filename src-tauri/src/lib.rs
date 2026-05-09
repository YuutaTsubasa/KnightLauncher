use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use uuid::Uuid;
use walkdir::WalkDir;

mod artwork;
mod emudeck;
mod ps3;
mod ra;
mod steam;

pub(crate) use artwork::{
    artwork_kind_from_label, convert_optional_to_webp, download_to, save_bytes_as_webp,
    ArtworkKind,
};

const DETAIL_WINDOW: &str = "detail";
const LIBRARY_WINDOW: &str = "main";
const STEAMGRIDDB_API_BASE: &str = "https://www.steamgriddb.com/api/v2";

#[derive(Default)]
struct DisplayAssignment {
    swapped: Mutex<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVariant {
    pub id: String,
    pub label: String,
    pub rom_path: String,
    pub last_played_at: Option<String>,
    pub play_count: u32,
    #[serde(default)]
    pub retro_achievements: Option<RetroAchievementsLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub badge_url: String,
    pub badge_locked_url: String,
    pub badge_path: Option<String>,
    pub badge_locked_path: Option<String>,
    pub earned_date: Option<String>,
    pub display_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetroAchievementsLink {
    pub game_id: u32,
    pub title: String,
    pub console_id: u32,
    pub console_name: String,
    pub icon_path: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub box_art_url: Option<String>,
    #[serde(default)]
    pub title_url: Option<String>,
    #[serde(default)]
    pub ingame_url: Option<String>,
    pub achievements_total: u32,
    pub achievements_earned: u32,
    pub points_total: u32,
    pub points_earned: u32,
    pub achievements: Vec<Achievement>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub title: String,
    pub executable_path: String,
    pub launch_args: String,
    pub working_directory: String,
    pub cover_image: Option<String>,
    pub hero_image: Option<String>,
    pub logo_image: Option<String>,
    pub favorite: bool,
    pub last_played_at: Option<String>,
    pub play_count: u32,
    pub description: Option<String>,
    pub platform: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub rom_system: Option<String>,
    #[serde(default)]
    pub variants: Vec<GameVariant>,
    #[serde(default)]
    pub retro_achievements: Option<RetroAchievementsLink>,
    #[serde(default)]
    pub position: u32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub preferred_achievement_variant_id: Option<String>,
    #[serde(default)]
    pub steam_app_id: Option<String>,
    #[serde(default)]
    pub steam_achievements: Option<RetroAchievementsLink>,
    #[serde(default)]
    pub ps3_trophies: Option<RetroAchievementsLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub games: Vec<Game>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub steamgriddb_api_key: Option<String>,
    #[serde(default)]
    pub emudeck_root: Option<String>,
    #[serde(default)]
    pub retro_achievements_user: Option<String>,
    #[serde(default)]
    pub retro_achievements_api_key: Option<String>,
    #[serde(default)]
    pub steam_root: Option<String>,
    #[serde(default)]
    pub steam_user_id: Option<String>,
    #[serde(default)]
    pub rpcs3_games_root: Option<String>,
    #[serde(default)]
    pub rpcs3_trophy_root: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub id: u32,
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGridDbGame {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGridDbAsset {
    pub id: u32,
    pub kind: String,
    pub url: String,
    pub thumb: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGridDbArtwork {
    pub covers: Vec<SteamGridDbAsset>,
    pub heroes: Vec<SteamGridDbAsset>,
    pub logos: Vec<SteamGridDbAsset>,
    pub icons: Vec<SteamGridDbAsset>,
}

#[derive(Debug, Deserialize)]
struct SteamGridDbResponse<T> {
    success: bool,
    data: T,
}

#[derive(Debug, Deserialize)]
struct SteamGridDbImage {
    id: u32,
    url: String,
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    style: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleImageResult {
    pub title: String,
    pub link: String,
    pub thumbnail: String,
    pub context_link: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mime: Option<String>,
}


#[derive(Debug, Clone)]
struct MonitorLayout {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl MonitorLayout {
    fn from_monitor(monitor: &tauri::Monitor) -> Self {
        let position = monitor.position();
        let size = monitor.size();

        Self {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        }
    }
}

fn library_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create app data directory: {error}"))?;
    Ok(dir.join("library.json"))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create app data directory: {error}"))?;
    Ok(dir.join("settings.json"))
}

pub(crate) fn read_library_from_disk(app: &AppHandle) -> Result<Library, String> {
    let path = library_path(app)?;
    if !path.exists() {
        return Ok(Library::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Unable to parse {}: {error}", path.display()))
}

pub(crate) fn read_settings_from_disk(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Unable to parse {}: {error}", path.display()))
}

fn rotate_backup(path: &Path) {
    if !path.exists() {
        return;
    }
    let mut backup = path.to_path_buf();
    let new_ext = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{ext}.bak"),
        None => "bak".to_string(),
    };
    backup.set_extension(new_ext);
    let _ = fs::copy(path, &backup);
}

pub(crate) fn write_library_to_disk(app: &AppHandle, library: &Library) -> Result<(), String> {
    let path = library_path(app)?;
    let contents = serde_json::to_string_pretty(library)
        .map_err(|error| format!("Unable to serialize library: {error}"))?;
    rotate_backup(&path);
    fs::write(&path, contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))
}

pub(crate) fn write_settings_to_disk(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Unable to serialize settings: {error}"))?;
    rotate_backup(&path);
    fs::write(&path, contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))
}

fn steamgriddb_key(app: &AppHandle) -> Result<String, String> {
    let key = read_settings_from_disk(app)?
        .steamgriddb_api_key
        .unwrap_or_default()
        .trim()
        .to_string();

    if key.is_empty() {
        return Err("SteamGridDB API key is not configured.".to_string());
    }

    Ok(key)
}

pub(crate) fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(18))
        .user_agent(concat!("KnightLauncher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Unable to create HTTP client: {error}"))
}

fn steamgriddb_get<T: for<'de> Deserialize<'de>>(app: &AppHandle, path: &str) -> Result<T, String> {
    let key = steamgriddb_key(app)?;
    let url = format!("{STEAMGRIDDB_API_BASE}{path}");
    let response = http_client()?
        .get(url)
        .bearer_auth(key)
        .send()
        .map_err(|error| format!("Unable to reach SteamGridDB: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("SteamGridDB returned HTTP {status}."));
    }

    let payload = response
        .json::<SteamGridDbResponse<T>>()
        .map_err(|error| format!("Unable to read SteamGridDB response: {error}"))?;

    if !payload.success {
        return Err("SteamGridDB request failed.".to_string());
    }

    Ok(payload.data)
}

fn sgdb_assets(images: Vec<SteamGridDbImage>, kind: &str) -> Vec<SteamGridDbAsset> {
    images
        .into_iter()
        .map(|image| SteamGridDbAsset {
            id: image.id,
            kind: kind.to_string(),
            thumb: image.thumb.unwrap_or_else(|| image.url.clone()),
            url: image.url,
            width: image.width,
            height: image.height,
            style: image.style,
        })
        .collect()
}

fn artwork_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?
        .join("artwork");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create artwork directory: {error}"))?;
    Ok(dir)
}

fn validate_download_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("Invalid artwork URL: {error}"))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err("Artwork URL must use HTTP or HTTPS.".to_string());
    }

    let Some(host) = parsed.host_str() else {
        return Err("Artwork URL is missing a host.".to_string());
    };

    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") {
        return Err("Artwork URL cannot point to a local host.".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(ip) => {
                ip.is_private()
                    || ip.is_loopback()
                    || ip.is_link_local()
                    || ip.is_broadcast()
                    || ip.is_documentation()
                    || ip.is_unspecified()
            }
            IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified(),
        };

        if blocked {
            return Err("Artwork URL cannot point to a private or local address.".to_string());
        }
    }

    Ok(())
}

fn validate_steamgriddb_asset_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|error| format!("Invalid artwork URL: {error}"))?;
    if parsed.scheme() != "https" {
        return Err("SteamGridDB artwork URL must use HTTPS.".to_string());
    }

    match parsed.host_str() {
        Some("cdn2.steamgriddb.com") | Some("steamgriddb.com") | Some("www.steamgriddb.com") => {
            Ok(())
        }
        _ => Err("Artwork URL is not from SteamGridDB.".to_string()),
    }
}

fn safe_artwork_kind(kind: &str) -> Result<&str, String> {
    match kind {
        "cover" | "hero" | "logo" | "icon" => Ok(kind),
        _ => Err("Unsupported artwork type.".to_string()),
    }
}

fn normalize_optional_secret(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled Game")
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn game_from_executable(path: PathBuf) -> Game {
    let working_directory = path
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    Game {
        id: Uuid::new_v4().to_string(),
        title: title_from_path(&path),
        executable_path: path.to_string_lossy().to_string(),
        launch_args: String::new(),
        working_directory,
        cover_image: None,
        hero_image: None,
        logo_image: None,
        favorite: false,
        last_played_at: None,
        play_count: 0,
        description: None,
        platform: Some("Windows".to_string()),
        tags: Vec::new(),
        rom_system: None,
        variants: Vec::new(),
        retro_achievements: None,
        position: 0,
        hidden: false,
        preferred_achievement_variant_id: None,
        steam_app_id: None,
        steam_achievements: None,
        ps3_trophies: None,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameFinishedPayload {
    game_id: String,
    variant_id: Option<String>,
}

pub(crate) fn handoff_focus_to_child(
    app: &AppHandle,
    mut child: Child,
    game_id: String,
    variant_id: Option<String>,
) {
    if let Some(window) = app.get_webview_window(LIBRARY_WINDOW) {
        let _ = window.minimize();
    }
    if let Some(window) = app.get_webview_window(DETAIL_WINDOW) {
        let _ = window.minimize();
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        if let Some(window) = handle.get_webview_window(LIBRARY_WINDOW) {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        if let Some(window) = handle.get_webview_window(DETAIL_WINDOW) {
            let _ = window.unminimize();
        }
        let _ = handle.emit(
            "game-finished",
            GameFinishedPayload {
                game_id,
                variant_id,
            },
        );
    });
}


#[tauri::command]
fn set_preferred_achievement_variant(
    app: AppHandle,
    game_id: String,
    variant_id: Option<String>,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    if let Some(ref variant_id) = variant_id {
        let exists = library.games[idx]
            .variants
            .iter()
            .any(|variant| &variant.id == variant_id);
        if !exists {
            return Err("Variant not found.".to_string());
        }
    }
    library.games[idx].preferred_achievement_variant_id = variant_id;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

fn push_path_if_set(slot: &Option<String>, set: &mut HashSet<PathBuf>) {
    if let Some(path) = slot {
        if !path.is_empty() {
            set.insert(PathBuf::from(path));
        }
    }
}

fn collect_link_paths(link: &RetroAchievementsLink, set: &mut HashSet<PathBuf>) {
    push_path_if_set(&link.icon_path, set);
    for achievement in &link.achievements {
        push_path_if_set(&achievement.badge_path, set);
        push_path_if_set(&achievement.badge_locked_path, set);
    }
}

fn convert_link_to_webp(link: &mut RetroAchievementsLink) {
    convert_optional_to_webp(&mut link.icon_path, ArtworkKind::Logo);
    for achievement in link.achievements.iter_mut() {
        convert_optional_to_webp(&mut achievement.badge_path, ArtworkKind::Badge);
        convert_optional_to_webp(&mut achievement.badge_locked_path, ArtworkKind::Badge);
    }
}

fn collect_referenced_artwork_paths(library: &Library) -> HashSet<PathBuf> {
    let mut keep: HashSet<PathBuf> = HashSet::new();
    for game in &library.games {
        push_path_if_set(&game.cover_image, &mut keep);
        push_path_if_set(&game.hero_image, &mut keep);
        push_path_if_set(&game.logo_image, &mut keep);
        if let Some(link) = &game.retro_achievements {
            collect_link_paths(link, &mut keep);
        }
        if let Some(link) = &game.steam_achievements {
            collect_link_paths(link, &mut keep);
        }
        if let Some(link) = &game.ps3_trophies {
            collect_link_paths(link, &mut keep);
        }
        for variant in &game.variants {
            if let Some(link) = &variant.retro_achievements {
                collect_link_paths(link, &mut keep);
            }
        }
    }
    keep
}

fn achievement_cache_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(app_data) = app.path().app_data_dir() {
        roots.push(app_data.join("retroachievements"));
        roots.push(app_data.join("steam_achievements"));
    }
    if let Ok(local) = app.path().app_local_data_dir() {
        roots.push(local.join("cache").join("ps3_trophies"));
    }
    roots
}

fn cleanup_orphans_under(dir: &Path, keep: &HashSet<PathBuf>) -> u32 {
    if !dir.is_dir() {
        return 0;
    }
    let mut removed = 0u32;
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if !keep.contains(path) && fs::remove_file(path).is_ok() {
            removed = removed.saturating_add(1);
        }
    }
    removed
}

#[tauri::command]
fn cleanup_orphan_artwork(app: AppHandle) -> Result<u32, String> {
    let library = read_library_from_disk(&app)?;
    let keep = collect_referenced_artwork_paths(&library);

    let mut removed = 0u32;
    if let Ok(art_dir) = artwork_dir(&app) {
        removed = removed.saturating_add(cleanup_orphans_under(&art_dir, &keep));
    }
    for root in achievement_cache_roots(&app) {
        removed = removed.saturating_add(cleanup_orphans_under(&root, &keep));
    }
    Ok(removed)
}

#[tauri::command]
fn convert_library_artwork_to_webp(app: AppHandle) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;

    for game in library.games.iter_mut() {
        convert_optional_to_webp(&mut game.cover_image, ArtworkKind::Cover);
        convert_optional_to_webp(&mut game.hero_image, ArtworkKind::Hero);
        convert_optional_to_webp(&mut game.logo_image, ArtworkKind::Logo);

        if let Some(link) = game.retro_achievements.as_mut() {
            convert_link_to_webp(link);
        }
        if let Some(link) = game.steam_achievements.as_mut() {
            convert_link_to_webp(link);
        }
        if let Some(link) = game.ps3_trophies.as_mut() {
            convert_link_to_webp(link);
        }

        for variant in game.variants.iter_mut() {
            if let Some(link) = variant.retro_achievements.as_mut() {
                convert_link_to_webp(link);
            }
        }
    }

    write_library_to_disk(&app, &library)?;

    let keep = collect_referenced_artwork_paths(&library);
    if let Ok(art_dir) = artwork_dir(&app) {
        cleanup_orphans_under(&art_dir, &keep);
    }
    for root in achievement_cache_roots(&app) {
        cleanup_orphans_under(&root, &keep);
    }

    Ok(library)
}

#[tauri::command]
fn rename_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
    label: String,
) -> Result<Library, String> {
    let trimmed = label.trim().to_string();
    if trimmed.is_empty() {
        return Err("Variant label cannot be empty.".to_string());
    }
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
    library.games[game_idx].variants[variant_idx].label = trimmed;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}


fn sorted_monitor_layouts(app: &AppHandle) -> Result<Vec<MonitorLayout>, String> {
    let mut monitors = app
        .available_monitors()
        .map_err(|error| format!("Unable to read display layout: {error}"))?
        .iter()
        .map(MonitorLayout::from_monitor)
        .collect::<Vec<_>>();

    monitors.sort_by_key(|monitor| (monitor.y, monitor.x));
    Ok(monitors)
}

fn role_url(role: &str) -> WebviewUrl {
    WebviewUrl::App(format!("index.html?window={role}").into())
}

fn set_window_on_monitor(window: &WebviewWindow, monitor: &MonitorLayout) -> Result<(), String> {
    window
        .set_fullscreen(false)
        .map_err(|error| format!("Unable to leave fullscreen: {error}"))?;
    window
        .set_decorations(false)
        .map_err(|error| format!("Unable to disable decorations: {error}"))?;
    window
        .set_position(PhysicalPosition::new(monitor.x, monitor.y))
        .map_err(|error| format!("Unable to position window: {error}"))?;
    window
        .set_size(PhysicalSize::new(monitor.width, monitor.height))
        .map_err(|error| format!("Unable to size window: {error}"))?;
    window
        .set_fullscreen(true)
        .map_err(|error| format!("Unable to enter fullscreen: {error}"))?;
    window
        .show()
        .map_err(|error| format!("Unable to show window: {error}"))?;
    Ok(())
}

fn window_for_role(app: &AppHandle, label: &str, role: &str) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(app, label, role_url(role))
        .title(if role == "detail" {
            "KnightLauncher Detail"
        } else {
            "KnightLauncher Library"
        })
        .decorations(false)
        .resizable(false)
        .visible(false)
        .build()
        .map_err(|error| format!("Unable to create {role} window: {error}"))
}

fn arrange_dual_windows(app: &AppHandle, swapped: bool) -> Result<(), String> {
    let monitors = sorted_monitor_layouts(app)?;

    let library_window = window_for_role(app, LIBRARY_WINDOW, "library")?;
    let detail_window = window_for_role(app, DETAIL_WINDOW, "detail")?;

    if monitors.len() < 2 {
        library_window
            .show()
            .map_err(|error| format!("Unable to show single-window layout: {error}"))?;
        detail_window
            .hide()
            .map_err(|error| format!("Unable to hide detail window: {error}"))?;
        return Ok(());
    }

    let top_monitor = &monitors[0];
    let bottom_monitor = &monitors[monitors.len() - 1];
    let (detail_monitor, library_monitor) = if swapped {
        (bottom_monitor, top_monitor)
    } else {
        (top_monitor, bottom_monitor)
    };

    set_window_on_monitor(&detail_window, detail_monitor)?;
    set_window_on_monitor(&library_window, library_monitor)?;
    library_window
        .set_focus()
        .map_err(|error| format!("Unable to focus library window: {error}"))?;

    Ok(())
}

#[tauri::command]
fn load_library(app: AppHandle) -> Result<Library, String> {
    read_library_from_disk(&app)
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
    read_settings_from_disk(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let settings = AppSettings {
        steamgriddb_api_key: normalize_optional_secret(settings.steamgriddb_api_key),
        emudeck_root: normalize_optional_secret(settings.emudeck_root),
        retro_achievements_user: normalize_optional_secret(settings.retro_achievements_user),
        retro_achievements_api_key: normalize_optional_secret(settings.retro_achievements_api_key),
        steam_root: normalize_optional_secret(settings.steam_root),
        steam_user_id: normalize_optional_secret(settings.steam_user_id),
        rpcs3_games_root: normalize_optional_secret(settings.rpcs3_games_root),
        rpcs3_trophy_root: normalize_optional_secret(settings.rpcs3_trophy_root),
    };
    write_settings_to_disk(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
fn save_library(app: AppHandle, library: Library) -> Result<Library, String> {
    let mut library = library;
    ensure_positions(&mut library);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn upsert_game(app: AppHandle, game: Game) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;

    match library.games.iter().position(|item| item.id == game.id) {
        Some(index) => library.games[index] = game,
        None => library.games.push(game),
    }

    ensure_positions(&mut library);
    library.games.sort_by_key(|game| game.title.to_lowercase());
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn remove_game(app: AppHandle, id: String) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    library.games.retain(|game| game.id != id);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

pub(crate) fn ensure_positions(library: &mut Library) {
    if library.games.is_empty() {
        return;
    }
    if library.games.iter().all(|game| game.position == 0) {
        let mut order: Vec<usize> = (0..library.games.len()).collect();
        order.sort_by(|&a, &b| {
            library.games[a]
                .title
                .to_lowercase()
                .cmp(&library.games[b].title.to_lowercase())
        });
        for (index, &original) in order.iter().enumerate() {
            library.games[original].position = (index as u32) + 1;
        }
        return;
    }
    let mut next = library
        .games
        .iter()
        .map(|game| game.position)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for game in library.games.iter_mut() {
        if game.position == 0 {
            game.position = next;
            next = next.saturating_add(1);
        }
    }
}

pub(crate) fn next_position(library: &Library) -> u32 {
    library
        .games
        .iter()
        .map(|game| game.position)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

#[tauri::command]
fn swap_game_positions(
    app: AppHandle,
    game_id_a: String,
    game_id_b: String,
) -> Result<Library, String> {
    if game_id_a == game_id_b {
        return read_library_from_disk(&app);
    }
    let mut library = read_library_from_disk(&app)?;
    ensure_positions(&mut library);

    let pos_a = library
        .games
        .iter()
        .find(|game| game.id == game_id_a)
        .map(|game| game.position)
        .ok_or_else(|| "First game not found.".to_string())?;
    let pos_b = library
        .games
        .iter()
        .find(|game| game.id == game_id_b)
        .map(|game| game.position)
        .ok_or_else(|| "Second game not found.".to_string())?;

    for game in library.games.iter_mut() {
        if game.id == game_id_a {
            game.position = pos_b;
        } else if game.id == game_id_b {
            game.position = pos_a;
        }
    }

    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn set_game_hidden(app: AppHandle, game_id: String, hidden: bool) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    library.games[idx].hidden = hidden;
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn merge_games(
    app: AppHandle,
    source_id: String,
    target_id: String,
) -> Result<Library, String> {
    if source_id == target_id {
        return Err("Cannot merge a game into itself.".to_string());
    }
    let mut library = read_library_from_disk(&app)?;

    let source_idx = library
        .games
        .iter()
        .position(|game| game.id == source_id)
        .ok_or_else(|| "Source game not found.".to_string())?;
    let source = library.games.remove(source_idx);

    if source.variants.is_empty() {
        // Restore source so caller sees consistent state
        library.games.push(source);
        return Err("Source game has no ROM variants to merge.".to_string());
    }

    let target_idx = library
        .games
        .iter()
        .position(|game| game.id == target_id)
        .ok_or_else(|| "Target game not found.".to_string())?;

    let target = &mut library.games[target_idx];
    let existing_paths: HashSet<String> = target
        .variants
        .iter()
        .map(|variant| variant.rom_path.clone())
        .collect();
    for variant in source.variants {
        if !existing_paths.contains(&variant.rom_path) {
            target.variants.push(variant);
        }
    }
    target.play_count = target.play_count.saturating_add(source.play_count);
    if let Some(source_played) = source.last_played_at.as_ref() {
        match &target.last_played_at {
            None => target.last_played_at = Some(source_played.clone()),
            Some(current) if source_played > current => {
                target.last_played_at = Some(source_played.clone())
            }
            _ => {}
        }
    }

    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn split_variant(
    app: AppHandle,
    game_id: String,
    variant_id: String,
) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;

    let idx = library
        .games
        .iter()
        .position(|game| game.id == game_id)
        .ok_or_else(|| "Game not found.".to_string())?;
    if library.games[idx].variants.len() <= 1 {
        return Err("Game has only one variant; nothing to split.".to_string());
    }
    let variant_pos = library.games[idx]
        .variants
        .iter()
        .position(|variant| variant.id == variant_id)
        .ok_or_else(|| "Variant not found.".to_string())?;

    let variant = library.games[idx].variants.remove(variant_pos);
    let position = next_position(&library);
    let source = &library.games[idx];

    let new_game = Game {
        id: Uuid::new_v4().to_string(),
        title: format!("{} ({})", source.title, variant.label),
        executable_path: String::new(),
        launch_args: String::new(),
        working_directory: String::new(),
        cover_image: source.cover_image.clone(),
        hero_image: source.hero_image.clone(),
        logo_image: source.logo_image.clone(),
        favorite: false,
        last_played_at: variant.last_played_at.clone(),
        play_count: variant.play_count,
        description: source.description.clone(),
        platform: source.platform.clone(),
        tags: source.tags.clone(),
        rom_system: source.rom_system.clone(),
        variants: vec![variant],
        retro_achievements: None,
        position,
        hidden: false,
        preferred_achievement_variant_id: None,
        steam_app_id: None,
        steam_achievements: None,
        ps3_trophies: None,
    };
    library.games.push(new_game);

    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn select_executable() -> Result<Option<Game>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Executable", &["exe"])
        .pick_file()
    else {
        return Ok(None);
    };

    Ok(Some(game_from_executable(path)))
}

#[tauri::command]
fn select_executable_path() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .add_filter("Executable", &["exe"])
        .pick_file()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn select_image_path() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "webp", "bmp"])
        .pick_file()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn select_folder() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn scan_folder(path: String) -> Result<Vec<Game>, String> {
    let root = PathBuf::from(path);
    if !root.exists() {
        return Err("Folder does not exist.".to_string());
    }

    let mut seen = HashSet::new();
    let mut games = Vec::new();

    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let is_executable = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("exe"))
            .unwrap_or(false);

        if is_executable && seen.insert(path.to_path_buf()) {
            games.push(game_from_executable(path.to_path_buf()));
        }
    }

    games.sort_by_key(|game| game.title.to_lowercase());
    Ok(games)
}

pub(crate) fn extract_xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = xml.find(&open)?;
    let after = s + open.len();
    let e = xml[after..].find(&close)? + after;
    let raw = xml[after..e].trim();
    let unwrapped = raw
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(raw)
        .trim();
    Some(unwrapped.to_string())
}

#[tauri::command]
fn launch_game(app: AppHandle, id: String) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let Some(idx) = library.games.iter().position(|game| game.id == id) else {
        return Err("Game not found.".to_string());
    };

    let game_id = library.games[idx].id.clone();
    let game_title = library.games[idx].title.clone();
    let steam_app_id = library.games[idx].steam_app_id.clone();

    let child = if let Some(app_id) = steam_app_id.filter(|value| !value.trim().is_empty()) {
        let mut command = Command::new("cmd");
        command
            .arg("/c")
            .arg("start")
            .arg("")
            .arg(format!("steam://rungameid/{}", app_id.trim()));
        command
            .spawn()
            .map_err(|error| format!("Unable to launch Steam app {app_id}: {error}"))?
    } else {
        let game = &library.games[idx];
        let executable = PathBuf::from(&game.executable_path);
        if game.executable_path.trim().is_empty() || !executable.exists() {
            return Err("Executable path no longer exists.".to_string());
        }
        let mut command = Command::new(&game.executable_path);
        if !game.working_directory.trim().is_empty() {
            command.current_dir(&game.working_directory);
        }
        for arg in game.launch_args.split_whitespace() {
            command.arg(arg);
        }
        command
            .spawn()
            .map_err(|error| format!("Unable to launch {}: {error}", game_title))?
    };

    handoff_focus_to_child(&app, child, game_id, None);

    let game = &mut library.games[idx];
    game.last_played_at = Some(Utc::now().to_rfc3339());
    game.play_count = game.play_count.saturating_add(1);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn detect_displays(app: AppHandle) -> Result<Vec<DisplayInfo>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| format!("Unable to read display layout: {error}"))?;

    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            DisplayInfo {
                id: index as u32,
                name: monitor.name().map(ToString::to_string),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect())
}

#[tauri::command]
fn steamgriddb_search_games(app: AppHandle, query: String) -> Result<Vec<SteamGridDbGame>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let encoded = urlencoding::encode(query);
    let games =
        steamgriddb_get::<Vec<SteamGridDbGame>>(&app, &format!("/search/autocomplete/{encoded}"))?;
    Ok(games.into_iter().take(12).collect())
}

#[tauri::command]
fn steamgriddb_game_artwork(app: AppHandle, game_id: u32) -> Result<SteamGridDbArtwork, String> {
    let covers = steamgriddb_get::<Vec<SteamGridDbImage>>(
        &app,
        &format!("/grids/game/{game_id}"),
    )
    .map(|images| sgdb_assets(images, "cover"))
    .unwrap_or_default();
    let heroes = steamgriddb_get::<Vec<SteamGridDbImage>>(&app, &format!("/heroes/game/{game_id}"))
        .map(|images| sgdb_assets(images, "hero"))
        .unwrap_or_default();
    let logos = steamgriddb_get::<Vec<SteamGridDbImage>>(&app, &format!("/logos/game/{game_id}"))
        .map(|images| sgdb_assets(images, "logo"))
        .unwrap_or_default();
    let icons = steamgriddb_get::<Vec<SteamGridDbImage>>(&app, &format!("/icons/game/{game_id}"))
        .map(|images| sgdb_assets(images, "icon"))
        .unwrap_or_default();

    Ok(SteamGridDbArtwork {
        covers,
        heroes,
        logos,
        icons,
    })
}

#[tauri::command]
fn steamgriddb_download_artwork(
    app: AppHandle,
    url: String,
    kind: String,
    game_id: String,
) -> Result<String, String> {
    let kind = safe_artwork_kind(&kind)?;
    validate_steamgriddb_asset_url(&url)?;

    let response = http_client()?
        .get(&url)
        .send()
        .map_err(|error| format!("Unable to download artwork: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Artwork download returned HTTP {status}."));
    }

    let path =
        artwork_dir(&app)?.join(format!("{game_id}-{kind}-{}.webp", Uuid::new_v4()));
    let bytes = response
        .bytes()
        .map_err(|error| format!("Unable to read artwork bytes: {error}"))?;
    let saved = save_bytes_as_webp(&bytes, &path, artwork_kind_from_label(kind))?;
    Ok(saved.to_string_lossy().to_string())
}

#[tauri::command]
fn google_download_artwork(
    app: AppHandle,
    url: String,
    kind: String,
    game_id: String,
) -> Result<String, String> {
    let kind = safe_artwork_kind(&kind)?;
    validate_download_url(&url)?;

    let response = http_client()?
        .get(&url)
        .send()
        .map_err(|error| format!("Unable to download artwork: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Artwork download returned HTTP {status}."));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if !content_type.is_empty() && !content_type.starts_with("image/") {
        return Err("Selected Google result is not an image response.".to_string());
    }

    let path = artwork_dir(&app)?.join(format!(
        "{game_id}-google-{kind}-{}.webp",
        Uuid::new_v4()
    ));
    let bytes = response
        .bytes()
        .map_err(|error| format!("Unable to read artwork bytes: {error}"))?;
    let saved = save_bytes_as_webp(&bytes, &path, artwork_kind_from_label(kind))?;
    Ok(saved.to_string_lossy().to_string())
}

#[tauri::command]
fn arrange_displays(app: AppHandle, assignment: State<DisplayAssignment>) -> Result<(), String> {
    let swapped = *assignment
        .swapped
        .lock()
        .map_err(|_| "Unable to read display assignment.".to_string())?;
    arrange_dual_windows(&app, swapped)?;
    let _ = app.emit("display-layout-changed", ());
    Ok(())
}

#[tauri::command]
fn swap_displays(app: AppHandle, assignment: State<DisplayAssignment>) -> Result<(), String> {
    let swapped = {
        let mut swapped = assignment
            .swapped
            .lock()
            .map_err(|_| "Unable to update display assignment.".to_string())?;
        *swapped = !*swapped;
        *swapped
    };

    arrange_dual_windows(&app, swapped)?;
    let _ = app.emit("display-layout-changed", ());
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(DisplayAssignment::default())
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(error) = arrange_dual_windows(&handle, false) {
                eprintln!("Unable to arrange KnightLauncher windows: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_library,
            load_settings,
            save_settings,
            save_library,
            upsert_game,
            remove_game,
            select_executable,
            select_executable_path,
            select_image_path,
            select_folder,
            scan_folder,
            emudeck::scan_emudeck_roms,
            ps3::scan_rpcs3_games,
            steam::scan_steam_library,
            ps3::ps3_trophies_link_game,
            ps3::ps3_trophies_refresh,
            ps3::ps3_trophies_unlink,
            steam::steam_achievements_link_game,
            steam::steam_achievements_refresh,
            steam::steam_achievements_unlink,
            launch_game,
            emudeck::launch_rom_variant,
            ra::retroachievements_search_games,
            ra::retroachievements_link_game,
            ra::retroachievements_refresh,
            ra::retroachievements_unlink,
            ra::retroachievements_link_variant,
            ra::retroachievements_refresh_variant,
            ra::retroachievements_unlink_variant,
            set_preferred_achievement_variant,
            convert_library_artwork_to_webp,
            cleanup_orphan_artwork,
            rename_variant,
            swap_game_positions,
            set_game_hidden,
            merge_games,
            split_variant,
            detect_displays,
            steamgriddb_search_games,
            steamgriddb_game_artwork,
            steamgriddb_download_artwork,
            google_download_artwork,
            arrange_displays,
            swap_displays
        ])
        .run(tauri::generate_context!())
        .expect("error while running KnightLauncher");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_xml_text_strips_cdata_and_trims() {
        let xml = "<wrap><name><![CDATA[Hello World]]></name></wrap>";
        assert_eq!(extract_xml_text(xml, "name").as_deref(), Some("Hello World"));

        let plain = "<n>  Plain  </n>";
        assert_eq!(extract_xml_text(plain, "n").as_deref(), Some("Plain"));

        let missing = "<other>x</other>";
        assert_eq!(extract_xml_text(missing, "name"), None);
    }

}
