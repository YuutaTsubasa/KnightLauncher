use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use uuid::Uuid;
use walkdir::WalkDir;

const DETAIL_WINDOW: &str = "detail";
const LIBRARY_WINDOW: &str = "main";
const STEAMGRIDDB_API_BASE: &str = "https://www.steamgriddb.com/api/v2";
const GOOGLE_CUSTOM_SEARCH_URL: &str = "https://www.googleapis.com/customsearch/v1";

#[derive(Default)]
struct DisplayAssignment {
    swapped: Mutex<bool>,
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
    pub google_api_key: Option<String>,
    pub google_search_engine_id: Option<String>,
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

#[derive(Debug, Deserialize)]
struct GoogleSearchResponse {
    #[serde(default)]
    items: Vec<GoogleSearchItem>,
}

#[derive(Debug, Deserialize)]
struct GoogleSearchItem {
    #[serde(default)]
    title: String,
    link: String,
    #[serde(default)]
    mime: Option<String>,
    image: GoogleSearchImage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleSearchImage {
    #[serde(default)]
    context_link: Option<String>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    width: Option<u32>,
    thumbnail_link: String,
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

fn read_library_from_disk(app: &AppHandle) -> Result<Library, String> {
    let path = library_path(app)?;
    if !path.exists() {
        return Ok(Library::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Unable to parse {}: {error}", path.display()))
}

fn read_settings_from_disk(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Unable to parse {}: {error}", path.display()))
}

fn write_library_to_disk(app: &AppHandle, library: &Library) -> Result<(), String> {
    let path = library_path(app)?;
    let contents = serde_json::to_string_pretty(library)
        .map_err(|error| format!("Unable to serialize library: {error}"))?;
    fs::write(&path, contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))
}

fn write_settings_to_disk(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Unable to serialize settings: {error}"))?;
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

fn google_search_settings(app: &AppHandle) -> Result<(String, String), String> {
    let settings = read_settings_from_disk(app)?;
    let api_key = settings
        .google_api_key
        .unwrap_or_default()
        .trim()
        .to_string();
    let search_engine_id = settings
        .google_search_engine_id
        .unwrap_or_default()
        .trim()
        .to_string();

    if api_key.is_empty() || search_engine_id.is_empty() {
        return Err("Google Search API key or Search Engine ID is not configured.".to_string());
    }

    Ok((api_key, search_engine_id))
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(18))
        .user_agent("KnightLauncher/0.1.3")
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
        .take(16)
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

fn extension_from_url(url: &str) -> &str {
    let path = url.split('?').next().unwrap_or(url);
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "png" => "png",
        _ => "png",
    }
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
    }
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
        google_api_key: normalize_optional_secret(settings.google_api_key),
        google_search_engine_id: normalize_optional_secret(settings.google_search_engine_id),
    };
    write_settings_to_disk(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
fn save_library(app: AppHandle, library: Library) -> Result<Library, String> {
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

#[tauri::command]
fn launch_game(app: AppHandle, id: String) -> Result<Library, String> {
    let mut library = read_library_from_disk(&app)?;
    let Some(game) = library.games.iter_mut().find(|game| game.id == id) else {
        return Err("Game not found.".to_string());
    };

    let executable = PathBuf::from(&game.executable_path);
    if !executable.exists() {
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
        .map_err(|error| format!("Unable to launch {}: {error}", game.title))?;

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
        &format!("/grids/game/{game_id}?dimensions=600x900,342x482,660x930,512x512"),
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

    let extension = extension_from_url(&url);
    let path =
        artwork_dir(&app)?.join(format!("{game_id}-{kind}-{}.{}", Uuid::new_v4(), extension));
    let bytes = response
        .bytes()
        .map_err(|error| format!("Unable to read artwork bytes: {error}"))?;
    let mut file = fs::File::create(&path)
        .map_err(|error| format!("Unable to create {}: {error}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn google_image_search(app: AppHandle, query: String) -> Result<Vec<GoogleImageResult>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let (api_key, search_engine_id) = google_search_settings(&app)?;
    let response = http_client()?
        .get(GOOGLE_CUSTOM_SEARCH_URL)
        .query(&[
            ("key", api_key.as_str()),
            ("cx", search_engine_id.as_str()),
            ("q", query),
            ("searchType", "image"),
            ("num", "10"),
            ("safe", "active"),
        ])
        .send()
        .map_err(|error| format!("Unable to reach Google Search: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Google Search returned HTTP {status}."));
    }

    let payload = response
        .json::<GoogleSearchResponse>()
        .map_err(|error| format!("Unable to read Google Search response: {error}"))?;

    Ok(payload
        .items
        .into_iter()
        .map(|item| GoogleImageResult {
            title: item.title,
            link: item.link,
            thumbnail: item.image.thumbnail_link,
            context_link: item.image.context_link,
            width: item.image.width,
            height: item.image.height,
            mime: item.mime,
        })
        .collect())
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

    let extension = extension_from_url(&url);
    let path = artwork_dir(&app)?.join(format!(
        "{game_id}-google-{kind}-{}.{}",
        Uuid::new_v4(),
        extension
    ));
    let bytes = response
        .bytes()
        .map_err(|error| format!("Unable to read artwork bytes: {error}"))?;
    let mut file = fs::File::create(&path)
        .map_err(|error| format!("Unable to create {}: {error}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;

    Ok(path.to_string_lossy().to_string())
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
            launch_game,
            detect_displays,
            steamgriddb_search_games,
            steamgriddb_game_artwork,
            steamgriddb_download_artwork,
            google_image_search,
            google_download_artwork,
            arrange_displays,
            swap_displays
        ])
        .run(tauri::generate_context!())
        .expect("error while running KnightLauncher");
}
