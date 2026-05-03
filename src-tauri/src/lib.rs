use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
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

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(18))
        .user_agent("KnightLauncher/0.1.23")
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
        rom_system: None,
        variants: Vec::new(),
        retro_achievements: None,
        position: 0,
        hidden: false,
        preferred_achievement_variant_id: None,
    }
}

struct EmuSystem {
    folder: &'static str,
    platform_id: &'static str,
    extensions: &'static [&'static str],
    launchers: &'static [&'static str],
}

const EMU_SYSTEMS: &[EmuSystem] = &[
    EmuSystem { folder: "snes", platform_id: "sfc", extensions: &["smc", "sfc", "fig"], launchers: &["retroarch"] },
    EmuSystem { folder: "nes", platform_id: "fc", extensions: &["nes"], launchers: &["retroarch"] },
    EmuSystem { folder: "n64", platform_id: "n64", extensions: &["n64", "z64", "v64"], launchers: &["retroarch"] },
    EmuSystem { folder: "gb", platform_id: "gb", extensions: &["gb"], launchers: &["retroarch"] },
    EmuSystem { folder: "gbc", platform_id: "gbc", extensions: &["gbc", "gb"], launchers: &["retroarch"] },
    EmuSystem { folder: "gba", platform_id: "gba", extensions: &["gba"], launchers: &["retroarch"] },
    EmuSystem { folder: "nds", platform_id: "ds", extensions: &["nds"], launchers: &["melonDS", "retroarch"] },
    EmuSystem { folder: "n3ds", platform_id: "3ds", extensions: &["3ds", "cci", "cia", "app", "cxi"], launchers: &["azahar"] },
    EmuSystem { folder: "gc", platform_id: "gc", extensions: &["iso", "gcm", "ciso", "rvz", "wbfs"], launchers: &["dolphin"] },
    EmuSystem { folder: "wii", platform_id: "wii", extensions: &["iso", "wbfs", "rvz", "wad"], launchers: &["dolphin", "primehack"] },
    EmuSystem { folder: "wiiu", platform_id: "wiiu", extensions: &["wud", "wux", "wua", "rpx"], launchers: &["Cemu"] },
    EmuSystem { folder: "switch", platform_id: "switch", extensions: &["nsp", "xci"], launchers: &["Ryujinx"] },
    EmuSystem { folder: "genesis", platform_id: "md", extensions: &["md", "smd", "bin", "gen"], launchers: &["retroarch"] },
    EmuSystem { folder: "megadrive", platform_id: "md", extensions: &["md", "smd", "bin", "gen"], launchers: &["retroarch"] },
    EmuSystem { folder: "sega32x", platform_id: "32x", extensions: &["32x", "md", "smd", "bin"], launchers: &["retroarch"] },
    EmuSystem { folder: "segacd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"] },
    EmuSystem { folder: "megacd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"] },
    EmuSystem { folder: "sega-cd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"] },
    EmuSystem { folder: "mega-cd", platform_id: "segacd", extensions: &["cue", "iso", "chd", "bin", "img"], launchers: &["retroarch"] },
    EmuSystem { folder: "saturn", platform_id: "sat", extensions: &["cue", "iso", "chd", "mds"], launchers: &["retroarch"] },
    EmuSystem { folder: "dreamcast", platform_id: "dc", extensions: &["gdi", "cdi", "cue", "chd"], launchers: &["retroarch"] },
    EmuSystem { folder: "gamegear", platform_id: "gg", extensions: &["gg"], launchers: &["retroarch"] },
    EmuSystem { folder: "mastersystem", platform_id: "sms", extensions: &["sms"], launchers: &["retroarch"] },
    EmuSystem { folder: "psx", platform_id: "ps1", extensions: &["chd", "cue", "iso", "pbp", "m3u", "ecm"], launchers: &["duckstation", "retroarch"] },
    EmuSystem { folder: "ps2", platform_id: "ps2", extensions: &["iso", "chd", "bin", "mdf"], launchers: &["pcsx2"] },
    EmuSystem { folder: "ps3", platform_id: "ps3", extensions: &["iso", "pkg", "rap"], launchers: &["rpcs3"] },
    EmuSystem { folder: "ps4", platform_id: "ps4", extensions: &["pkg", "iso"], launchers: &["shadps4"] },
    EmuSystem { folder: "psp", platform_id: "psp", extensions: &["iso", "cso", "pbp"], launchers: &["PPSSPP"] },
    EmuSystem { folder: "psvita", platform_id: "vita", extensions: &["vpk"], launchers: &["Vita3K"] },
    EmuSystem { folder: "ngp", platform_id: "ngpc", extensions: &["ngp"], launchers: &["retroarch"] },
    EmuSystem { folder: "ngpc", platform_id: "ngpc", extensions: &["ngc"], launchers: &["retroarch"] },
    EmuSystem { folder: "xbox", platform_id: "xbox", extensions: &["iso", "xbe"], launchers: &["xemu"] },
    EmuSystem { folder: "xbox360", platform_id: "xbox360", extensions: &["iso", "xex"], launchers: &["xenia"] },
    EmuSystem { folder: "scummvm", platform_id: "scummvm", extensions: &["scummvm"], launchers: &["ScummVM"] },
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

fn locate_launcher(emudeck_root: &Path, candidates: &[&str]) -> Option<PathBuf> {
    let dir = find_emudeck_subdir(emudeck_root, &["tools", "launchers"])?;
    for name in candidates {
        for ext in &["ps1", "bat", "cmd"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn lookup_emu_system(folder: &str) -> Option<&'static EmuSystem> {
    EMU_SYSTEMS.iter().find(|system| system.folder == folder)
}

fn spawn_launcher(launcher: &Path, rom_path: &Path) -> Result<Child, String> {
    let ext = launcher
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut command = match ext.as_str() {
        "bat" | "cmd" => {
            let mut c = Command::new("cmd");
            c.arg("/c").arg(launcher).arg(rom_path);
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell");
            c.arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(launcher)
                .arg(rom_path);
            c
        }
        other => return Err(format!("Unsupported launcher type: {other}")),
    };
    command
        .spawn()
        .map_err(|error| format!("Unable to launch ROM: {error}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameFinishedPayload {
    game_id: String,
    variant_id: Option<String>,
}

fn handoff_focus_to_child(
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

fn ra_console_id(platform_id: &str) -> Option<u32> {
    match platform_id {
        "md" => Some(1),
        "n64" => Some(2),
        "sfc" => Some(3),
        "gb" => Some(4),
        "gba" => Some(5),
        "gbc" => Some(6),
        "fc" => Some(7),
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

fn download_to(url: &str, target: &Path) -> Result<String, String> {
    if target.exists() {
        return Ok(target.to_string_lossy().to_string());
    }
    let response = http_client()?
        .get(url)
        .send()
        .map_err(|error| format!("Download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Download HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("Download read failed: {error}"))?;
    let mut file = fs::File::create(target)
        .map_err(|error| format!("Unable to create {}: {error}", target.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("Unable to write {}: {error}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
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
        let badge_path =
            download_to(&badge_url, &cache_dir.join(format!("{}.png", ach.badge_name))).ok();
        let badge_locked_path = download_to(
            &badge_locked_url,
            &cache_dir.join(format!("{}_lock.png", ach.badge_name)),
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
        .and_then(|url| download_to(url, &cache_dir.join("icon.png")).ok());

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
fn retroachievements_search_games(
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
fn retroachievements_link_game(
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
fn retroachievements_refresh(app: AppHandle, game_id: String) -> Result<Library, String> {
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
fn retroachievements_unlink(app: AppHandle, game_id: String) -> Result<Library, String> {
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
fn retroachievements_link_variant(
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
fn retroachievements_refresh_variant(
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
fn retroachievements_unlink_variant(
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

fn ensure_positions(library: &mut Library) {
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

fn next_position(library: &Library) -> u32 {
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

    let child = command
        .spawn()
        .map_err(|error| format!("Unable to launch {}: {error}", game.title))?;
    let game_id = game.id.clone();
    handoff_focus_to_child(&app, child, game_id, None);

    game.last_played_at = Some(Utc::now().to_rfc3339());
    game.play_count = game.play_count.saturating_add(1);
    write_library_to_disk(&app, &library)?;
    Ok(library)
}

#[tauri::command]
fn scan_emudeck_roms(app: AppHandle, root: String) -> Result<Library, String> {
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
fn launch_rom_variant(
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
    let launcher = locate_launcher(&emudeck_path, definition.launchers).ok_or_else(|| {
        format!(
            "Launcher not found for system '{system}'. Tried: {} under {}",
            definition.launchers.join(", "),
            emudeck_path.display()
        )
    })?;
    let child = spawn_launcher(&launcher, &rom_path)?;
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
            scan_emudeck_roms,
            launch_game,
            launch_rom_variant,
            retroachievements_search_games,
            retroachievements_link_game,
            retroachievements_refresh,
            retroachievements_unlink,
            retroachievements_link_variant,
            retroachievements_refresh_variant,
            retroachievements_unlink_variant,
            set_preferred_achievement_variant,
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
