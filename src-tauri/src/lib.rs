use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use walkdir::WalkDir;

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

fn library_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to resolve app data directory: {error}"))?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Unable to create app data directory: {error}"))?;
    Ok(dir.join("library.json"))
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

fn write_library_to_disk(app: &AppHandle, library: &Library) -> Result<(), String> {
    let path = library_path(app)?;
    let contents = serde_json::to_string_pretty(library)
        .map_err(|error| format!("Unable to serialize library: {error}"))?;
    fs::write(&path, contents).map_err(|error| format!("Unable to write {}: {error}", path.display()))
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
        favorite: false,
        last_played_at: None,
        play_count: 0,
        description: None,
        platform: Some("Windows".to_string()),
        tags: Vec::new(),
    }
}

#[tauri::command]
fn load_library(app: AppHandle) -> Result<Library, String> {
    read_library_from_disk(&app)
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

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_library,
            save_library,
            upsert_game,
            remove_game,
            select_executable,
            select_folder,
            scan_folder,
            launch_game,
            detect_displays
        ])
        .run(tauri::generate_context!())
        .expect("error while running KnightLauncher");
}
