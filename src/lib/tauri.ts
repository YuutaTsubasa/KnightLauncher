import { invoke } from '@tauri-apps/api/core';
import type { DisplayInfo, Game, GameLibrary } from './types';

const isTauri = '__TAURI_INTERNALS__' in window;

export async function loadLibrary(): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('load_library');
}

export async function saveLibrary(library: GameLibrary): Promise<GameLibrary> {
  if (!isTauri) return library;
  return invoke<GameLibrary>('save_library', { library });
}

export async function upsertGame(game: Game): Promise<GameLibrary> {
  if (!isTauri) return { games: [game, ...demoGames] };
  return invoke<GameLibrary>('upsert_game', { game });
}

export async function removeGame(id: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames.filter((game) => game.id !== id) };
  return invoke<GameLibrary>('remove_game', { id });
}

export async function selectExecutable(): Promise<Game | null> {
  if (!isTauri) return demoGames[0];
  return invoke<Game | null>('select_executable');
}

export async function selectFolder(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>('select_folder');
}

export async function scanFolder(path: string): Promise<Game[]> {
  if (!isTauri) return demoGames;
  return invoke<Game[]>('scan_folder', { path });
}

export async function launchGame(id: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('launch_game', { id });
}

export async function detectDisplays(): Promise<DisplayInfo[]> {
  if (!isTauri) {
    return [
      { id: 0, name: 'Top detail display', x: 0, y: 0, width: 1920, height: 1080, scaleFactor: 1 },
      { id: 1, name: 'Bottom control display', x: 0, y: 1080, width: 1920, height: 720, scaleFactor: 1 }
    ];
  }
  return invoke<DisplayInfo[]>('detect_displays');
}

const demoGames: Game[] = [
  {
    id: 'demo-elden-ring',
    title: 'Elden Ring',
    executablePath: 'C:\\Games\\Elden Ring\\eldenring.exe',
    launchArgs: '',
    workingDirectory: 'C:\\Games\\Elden Ring',
    coverImage: null,
    heroImage: null,
    favorite: true,
    lastPlayedAt: '2026-05-02T14:20:00Z',
    playCount: 18,
    description: 'A large action RPG profile used to preview the dual-screen layout.',
    platform: 'Windows',
    tags: ['Action RPG', 'Favorite']
  },
  {
    id: 'demo-hades',
    title: 'Hades II',
    executablePath: 'C:\\Games\\Hades II\\hades2.exe',
    launchArgs: '',
    workingDirectory: 'C:\\Games\\Hades II',
    coverImage: null,
    heroImage: null,
    favorite: false,
    lastPlayedAt: '2026-04-28T10:00:00Z',
    playCount: 7,
    description: 'Fast launch profile with keyboard and controller friendly navigation.',
    platform: 'Windows',
    tags: ['Roguelite']
  },
  {
    id: 'demo-cyberpunk',
    title: 'Cyberpunk 2077',
    executablePath: 'C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe',
    launchArgs: '-fullscreen',
    workingDirectory: 'C:\\Games\\Cyberpunk 2077\\bin\\x64',
    coverImage: null,
    heroImage: null,
    favorite: false,
    lastPlayedAt: null,
    playCount: 0,
    description: 'Example profile showing missing recent play data and launch arguments.',
    platform: 'Windows',
    tags: ['RPG']
  }
];
