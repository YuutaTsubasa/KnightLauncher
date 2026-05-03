import { derived, get, writable } from 'svelte/store';
import type { DisplayInfo, Game, LibraryFilter, SortMode } from './types';
import {
  detectDisplays,
  launchGame,
  launchRomVariant,
  loadLibrary,
  mergeGames,
  notifyLibraryChanged,
  removeGame,
  renameVariant,
  retroAchievementsLinkGame,
  retroAchievementsLinkVariant,
  retroAchievementsRefresh,
  retroAchievementsRefreshVariant,
  retroAchievementsUnlink,
  retroAchievementsUnlinkVariant,
  saveLibrary,
  scanEmudeckRoms,
  scanFolder,
  selectExecutable,
  selectFolder,
  setGameHidden,
  setPreferredAchievementVariant,
  splitVariant,
  swapGamePositions,
  upsertGame
} from './tauri';
import type { RetroAchievementsLink } from './types';

export const games = writable<Game[]>([]);
export const selectedId = writable<string | null>(null);
export const displays = writable<DisplayInfo[]>([]);
export const query = writable('');
export const filter = writable<LibraryFilter>('all');
export const sortMode = writable<SortMode>('manual');
export const busyLabel = writable<string | null>(null);
export const errorMessage = writable<string | null>(null);
export const launchState = writable<string | null>(null);
export const pickingVariantsFor = writable<Game | null>(null);
export const showingAchievementsFor = writable<Game | null>(null);
export const reorderMode = writable(false);

export const selectedGame = derived([games, selectedId], ([$games, $selectedId]) => {
  return $games.find((game) => game.id === $selectedId) ?? $games[0] ?? null;
});

export const filteredGames = derived(
  [games, query, filter, sortMode],
  ([$games, $query, $filter, $sortMode]) => {
    const normalizedQuery = $query.trim().toLowerCase();

    let result = $games.filter((game) => {
      if ($filter === 'hidden') {
        if (!game.hidden) return false;
      } else {
        if (game.hidden) return false;
        if ($filter === 'favorites' && !game.favorite) return false;
        if ($filter === 'recent' && !game.lastPlayedAt) return false;
      }
      if (!normalizedQuery) return true;

      return [game.title, game.platform, ...(game.tags ?? [])]
        .filter(Boolean)
        .join(' ')
        .toLowerCase()
        .includes(normalizedQuery);
    });

    result = [...result].sort((left, right) => {
      if ($sortMode === 'manual') {
        const diff = (left.position || 0) - (right.position || 0);
        return diff !== 0 ? diff : left.title.localeCompare(right.title);
      }
      if ($sortMode === 'recent') {
        return Date.parse(right.lastPlayedAt ?? '0') - Date.parse(left.lastPlayedAt ?? '0');
      }
      if ($sortMode === 'playCount') {
        return right.playCount - left.playCount;
      }
      return left.title.localeCompare(right.title);
    });

    return result;
  }
);

export const hasDualDisplay = derived(displays, ($displays) => $displays.length > 1);

export async function initializeLibrary() {
  busyLabel.set('Loading library');
  errorMessage.set(null);

  try {
    const [library, detectedDisplays] = await Promise.all([loadLibrary(), detectDisplays()]);
    games.set(library.games);
    displays.set(detectedDisplays);
    selectedId.set(library.games[0]?.id ?? null);
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export async function refreshLibraryPreservingSelection() {
  try {
    const currentSelectedId = get(selectedId);
    const library = await loadLibrary();
    games.set(library.games);
    selectedId.set(
      library.games.some((game) => game.id === currentSelectedId)
        ? currentSelectedId
        : (library.games[0]?.id ?? null)
    );
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function addExecutable() {
  busyLabel.set('Adding executable');
  errorMessage.set(null);

  try {
    const game = await selectExecutable();
    if (!game) return;

    const library = await upsertGame(game);
    games.set(library.games);
    selectedId.set(game.id);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export async function scanForGames() {
  busyLabel.set('Scanning folder');
  errorMessage.set(null);

  try {
    const folder = await selectFolder();
    if (!folder) return;

    const foundGames = await scanFolder(folder);
    const existingGames = get(games);
    const existingPaths = new Set(existingGames.map((game) => game.executablePath.toLowerCase()));
    const nextGames = [...existingGames, ...foundGames.filter((game) => !existingPaths.has(game.executablePath.toLowerCase()))]
      .sort((left, right) => left.title.localeCompare(right.title));

    const library = await saveLibrary({ games: nextGames });
    games.set(library.games);
    selectedId.set(foundGames[0]?.id ?? library.games[0]?.id ?? null);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export async function toggleFavorite(game: Game) {
  const nextGame = { ...game, favorite: !game.favorite };
  const library = await upsertGame(nextGame);
  games.set(library.games);
  selectedId.set(nextGame.id);
  notifyLibraryChanged().catch(() => {});
}

export async function saveGameEdits(game: Game) {
  errorMessage.set(null);

  try {
    const library = await upsertGame(game);
    games.set(library.games);
    selectedId.set(game.id);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  }
}

export async function deleteSelectedGame() {
  const id = get(selectedId);
  if (!id) return;

  const library = await removeGame(id);
  games.set(library.games);
  selectedId.set(library.games[0]?.id ?? null);
  notifyLibraryChanged().catch(() => {});
}

export async function deleteGameById(gameId: string) {
  errorMessage.set(null);
  try {
    const library = await removeGame(gameId);
    games.set(library.games);
    if (get(selectedId) === gameId) {
      selectedId.set(library.games[0]?.id ?? null);
    }
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  }
}

export async function launchSelectedGame() {
  const game = get(selectedGame);
  if (!game) return;

  if (game.variants.length > 1) {
    pickingVariantsFor.set(game);
    return;
  }
  if (game.variants.length === 1) {
    return launchVariant(game, game.variants[0].id);
  }

  launchState.set(`Launching ${game.title}`);
  errorMessage.set(null);

  try {
    const library = await launchGame(game.id);
    games.set(library.games);
    selectedId.set(game.id);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    window.setTimeout(() => launchState.set(null), 900);
  }
}

export async function launchVariant(game: Game, variantId: string) {
  pickingVariantsFor.set(null);
  const variant = game.variants.find((entry) => entry.id === variantId);
  launchState.set(`Launching ${game.title}${variant ? ` — ${variant.label}` : ''}`);
  errorMessage.set(null);

  try {
    const library = await launchRomVariant(game.id, variantId);
    games.set(library.games);
    selectedId.set(game.id);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    window.setTimeout(() => launchState.set(null), 900);
  }
}

export async function linkRetroAchievements(gameId: string, raGameId: number) {
  busyLabel.set('Linking RetroAchievements');
  errorMessage.set(null);
  try {
    const library = await retroAchievementsLinkGame(gameId, raGameId);
    games.set(library.games);
    selectedId.set(gameId);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  } finally {
    busyLabel.set(null);
  }
}

export async function refreshRetroAchievements(gameId: string) {
  errorMessage.set(null);
  try {
    const library = await retroAchievementsRefresh(gameId);
    games.set(library.games);
    selectedId.set(gameId);
    const refreshed = library.games.find((entry) => entry.id === gameId);
    if (refreshed) {
      const current = get(showingAchievementsFor);
      if (current && current.id === gameId) {
        showingAchievementsFor.set(refreshed);
      }
    }
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function unlinkRetroAchievements(gameId: string) {
  busyLabel.set('Unlinking RetroAchievements');
  errorMessage.set(null);
  try {
    const library = await retroAchievementsUnlink(gameId);
    games.set(library.games);
    selectedId.set(gameId);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export async function linkVariantRetroAchievements(gameId: string, variantId: string, raGameId: number) {
  busyLabel.set('Linking variant achievements');
  errorMessage.set(null);
  try {
    const library = await retroAchievementsLinkVariant(gameId, variantId, raGameId);
    games.set(library.games);
    selectedId.set(gameId);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  } finally {
    busyLabel.set(null);
  }
}

export async function refreshVariantRetroAchievements(gameId: string, variantId: string) {
  errorMessage.set(null);
  try {
    const library = await retroAchievementsRefreshVariant(gameId, variantId);
    games.set(library.games);
    selectedId.set(gameId);
    const refreshed = library.games.find((entry) => entry.id === gameId);
    if (refreshed) {
      const current = get(showingAchievementsFor);
      if (current && current.id === gameId) {
        showingAchievementsFor.set(refreshed);
      }
    }
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function unlinkVariantRetroAchievements(gameId: string, variantId: string) {
  busyLabel.set('Unlinking variant achievements');
  errorMessage.set(null);
  try {
    const library = await retroAchievementsUnlinkVariant(gameId, variantId);
    games.set(library.games);
    selectedId.set(gameId);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export async function renameVariantLabel(gameId: string, variantId: string, label: string) {
  errorMessage.set(null);
  try {
    const library = await renameVariant(gameId, variantId, label);
    games.set(library.games);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  }
}

export function effectiveAchievements(game: Game): RetroAchievementsLink | null {
  if (game.preferredAchievementVariantId) {
    const preferred = game.variants.find((variant) => variant.id === game.preferredAchievementVariantId);
    if (preferred?.retroAchievements) {
      return preferred.retroAchievements;
    }
  }
  const overrideVariants = game.variants.filter((variant) => variant.retroAchievements);
  if (overrideVariants.length) {
    const sorted = [...overrideVariants].sort((left, right) => {
      return Date.parse(right.lastPlayedAt ?? '0') - Date.parse(left.lastPlayedAt ?? '0');
    });
    return sorted[0].retroAchievements ?? game.retroAchievements ?? null;
  }
  return game.retroAchievements ?? null;
}

export async function setPreferredVariantForAchievements(gameId: string, variantId: string | null) {
  errorMessage.set(null);
  try {
    const library = await setPreferredAchievementVariant(gameId, variantId);
    games.set(library.games);
    selectedId.set(gameId);
    const refreshed = library.games.find((entry) => entry.id === gameId);
    if (refreshed) {
      const current = get(showingAchievementsFor);
      if (current && current.id === gameId) {
        showingAchievementsFor.set(refreshed);
      }
    }
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function scanForRoms() {
  busyLabel.set('Scanning EmuDeck ROMs');
  errorMessage.set(null);

  try {
    const folder = await selectFolder();
    if (!folder) return;

    const library = await scanEmudeckRoms(folder);
    games.set(library.games);
    selectedId.set(library.games[0]?.id ?? null);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    busyLabel.set(null);
  }
}

export function moveSelection(direction: 1 | -1) {
  const list = get(filteredGames);
  if (!list.length) return;

  const currentId = get(selectedId);
  const currentIndex = Math.max(0, list.findIndex((game) => game.id === currentId));
  const nextIndex = (currentIndex + direction + list.length) % list.length;
  selectedId.set(list[nextIndex].id);
}

export async function swapSelectedWith(targetId: string) {
  const currentId = get(selectedId);
  if (!currentId || currentId === targetId) return;
  if (get(sortMode) !== 'manual') {
    sortMode.set('manual');
  }
  try {
    const library = await swapGamePositions(currentId, targetId);
    games.set(library.games);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function toggleHiddenForSelected() {
  const id = get(selectedId);
  if (!id) return;
  const game = get(games).find((entry) => entry.id === id);
  if (!game) return;

  try {
    const library = await setGameHidden(id, !game.hidden);
    games.set(library.games);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
  }
}

export async function mergeIntoTarget(sourceId: string, targetId: string) {
  busyLabel.set('Merging games');
  errorMessage.set(null);
  try {
    const library = await mergeGames(sourceId, targetId);
    games.set(library.games);
    selectedId.set(targetId);
    notifyLibraryChanged().catch(() => {});
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  } finally {
    busyLabel.set(null);
  }
}

export async function splitVariantInto(gameId: string, variantId: string) {
  errorMessage.set(null);
  try {
    const library = await splitVariant(gameId, variantId);
    games.set(library.games);
    notifyLibraryChanged().catch(() => {});
    return library;
  } catch (error) {
    errorMessage.set(String(error));
    throw error;
  }
}
