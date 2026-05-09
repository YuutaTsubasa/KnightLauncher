import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import type {
  AppSettings,
  DisplayInfo,
  Game,
  GameLibrary,
  RaGameSearchResult,
  SteamGridDbArtwork,
  SteamGridDbGame
} from './types';

const isTauri = '__TAURI_INTERNALS__' in window;

export async function loadLibrary(): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('load_library');
}

export async function loadSettings(): Promise<AppSettings> {
  if (!isTauri) {
    return {
      steamgriddbApiKey: null,
      emudeckRoot: null,
      retroAchievementsUser: null,
      retroAchievementsApiKey: null,
      steamRoot: null,
      steamUserId: null,
      rpcs3GamesRoot: null,
      rpcs3TrophyRoot: null
    };
  }
  return invoke<AppSettings>('load_settings');
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauri) return settings;
  return invoke<AppSettings>('save_settings', { settings });
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

export async function selectExecutablePath(): Promise<string | null> {
  if (!isTauri) return 'C:\\Games\\Example\\game.exe';
  return invoke<string | null>('select_executable_path');
}

export async function selectImagePath(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>('select_image_path');
}

export async function selectFolder(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>('select_folder');
}

export async function scanFolder(path: string): Promise<Game[]> {
  if (!isTauri) return demoGames;
  return invoke<Game[]>('scan_folder', { path });
}

export async function scanEmudeckRoms(root: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('scan_emudeck_roms', { root });
}

export async function scanRpcs3Games(root: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('scan_rpcs3_games', { root });
}

export async function scanSteamLibrary(steamRoot: string | null): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('scan_steam_library', { steamRoot });
}

export async function steamAchievementsLinkGame(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('steam_achievements_link_game', { gameId });
}

export async function steamAchievementsRefresh(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('steam_achievements_refresh', { gameId });
}

export async function steamAchievementsUnlink(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('steam_achievements_unlink', { gameId });
}

export async function ps3TrophiesLinkGame(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('ps3_trophies_link_game', { gameId });
}

export async function ps3TrophiesRefresh(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('ps3_trophies_refresh', { gameId });
}

export async function ps3TrophiesUnlink(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('ps3_trophies_unlink', { gameId });
}

export async function launchGame(id: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('launch_game', { id });
}

export async function launchRomVariant(gameId: string, variantId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('launch_rom_variant', { gameId, variantId });
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

export async function arrangeDisplays(): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('arrange_displays');
}

export async function swapDisplays(): Promise<void> {
  if (!isTauri) return;
  return invoke<void>('swap_displays');
}

export async function notifyLibraryChanged(): Promise<void> {
  if (!isTauri) return;
  return emit('library-changed');
}

export async function getLogPath(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>('get_log_path');
}

export async function steamGridDbSearchGames(query: string): Promise<SteamGridDbGame[]> {
  if (!isTauri) {
    return [
      { id: 1245, name: query || 'Elden Ring', types: ['game'], verified: true },
      { id: 1246, name: `${query || 'Elden Ring'} Deluxe`, types: ['game'], verified: false }
    ];
  }
  return invoke<SteamGridDbGame[]>('steamgriddb_search_games', { query });
}

export async function steamGridDbGameArtwork(gameId: number): Promise<SteamGridDbArtwork> {
  if (!isTauri) {
    return {
      covers: [],
      heroes: [],
      logos: [],
      icons: []
    };
  }
  return invoke<SteamGridDbArtwork>('steamgriddb_game_artwork', { gameId });
}

export async function steamGridDbDownloadArtwork(url: string, kind: string, gameId: string): Promise<string> {
  if (!isTauri) return url;
  return invoke<string>('steamgriddb_download_artwork', { url, kind, gameId });
}

export async function googleDownloadArtwork(url: string, kind: string, gameId: string): Promise<string> {
  if (!isTauri) return url;
  return invoke<string>('google_download_artwork', { url, kind, gameId });
}

export async function retroAchievementsSearchGames(query: string, platformId: string): Promise<RaGameSearchResult[]> {
  if (!isTauri) return [];
  return invoke<RaGameSearchResult[]>('retroachievements_search_games', { query, platformId });
}

export async function retroAchievementsLinkGame(gameId: string, raGameId: number): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_link_game', { gameId, raGameId });
}

export async function retroAchievementsRefresh(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_refresh', { gameId });
}

export async function retroAchievementsUnlink(gameId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_unlink', { gameId });
}

export async function retroAchievementsLinkVariant(gameId: string, variantId: string, raGameId: number): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_link_variant', { gameId, variantId, raGameId });
}

export async function retroAchievementsRefreshVariant(gameId: string, variantId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_refresh_variant', { gameId, variantId });
}

export async function retroAchievementsUnlinkVariant(gameId: string, variantId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('retroachievements_unlink_variant', { gameId, variantId });
}

export async function renameVariant(gameId: string, variantId: string, label: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('rename_variant', { gameId, variantId, label });
}

export async function setPreferredAchievementVariant(gameId: string, variantId: string | null): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('set_preferred_achievement_variant', { gameId, variantId });
}

export async function convertLibraryArtworkToWebp(): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('convert_library_artwork_to_webp');
}

export async function swapGamePositions(gameIdA: string, gameIdB: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('swap_game_positions', { gameIdA, gameIdB });
}

export async function setGameHidden(gameId: string, hidden: boolean): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('set_game_hidden', { gameId, hidden });
}

export async function mergeGames(sourceId: string, targetId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('merge_games', { sourceId, targetId });
}

export async function splitVariant(gameId: string, variantId: string): Promise<GameLibrary> {
  if (!isTauri) return { games: demoGames };
  return invoke<GameLibrary>('split_variant', { gameId, variantId });
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
    logoImage: null,
    favorite: true,
    lastPlayedAt: '2026-05-02T14:20:00Z',
    playCount: 18,
    description: 'A large action RPG profile used to preview the dual-screen layout.',
    platform: 'Windows',
    tags: ['Action RPG', 'Favorite'],
    romSystem: null,
    variants: [],
    retroAchievements: null,
    position: 0,
    hidden: false,
    preferredAchievementVariantId: null,
    steamAppId: null,
    steamAchievements: null,
    ps3Trophies: null
  },
  {
    id: 'demo-hades',
    title: 'Hades II',
    executablePath: 'C:\\Games\\Hades II\\hades2.exe',
    launchArgs: '',
    workingDirectory: 'C:\\Games\\Hades II',
    coverImage: null,
    heroImage: null,
    logoImage: null,
    favorite: false,
    lastPlayedAt: '2026-04-28T10:00:00Z',
    playCount: 7,
    description: 'Fast launch profile with keyboard and controller friendly navigation.',
    platform: 'Windows',
    tags: ['Roguelite'],
    romSystem: null,
    variants: [],
    retroAchievements: null,
    position: 0,
    hidden: false,
    preferredAchievementVariantId: null,
    steamAppId: null,
    steamAchievements: null,
    ps3Trophies: null
  },
  {
    id: 'demo-cyberpunk',
    title: 'Cyberpunk 2077',
    executablePath: 'C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe',
    launchArgs: '-fullscreen',
    workingDirectory: 'C:\\Games\\Cyberpunk 2077\\bin\\x64',
    coverImage: null,
    heroImage: null,
    logoImage: null,
    favorite: false,
    lastPlayedAt: null,
    playCount: 0,
    description: 'Example profile showing missing recent play data and launch arguments.',
    platform: 'Windows',
    tags: ['RPG'],
    romSystem: null,
    variants: [],
    retroAchievements: null,
    position: 0,
    hidden: false,
    preferredAchievementVariantId: null,
    steamAppId: null,
    steamAchievements: null,
    ps3Trophies: null
  }
];
