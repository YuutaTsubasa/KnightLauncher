import { derived, get, writable } from 'svelte/store';
import type { DisplayInfo, Game, LibraryFilter, SortMode } from './types';
import {
  detectDisplays,
  launchGame,
  loadLibrary,
  removeGame,
  saveLibrary,
  scanFolder,
  selectExecutable,
  selectFolder,
  upsertGame
} from './tauri';

export const games = writable<Game[]>([]);
export const selectedId = writable<string | null>(null);
export const displays = writable<DisplayInfo[]>([]);
export const query = writable('');
export const filter = writable<LibraryFilter>('all');
export const sortMode = writable<SortMode>('title');
export const busyLabel = writable<string | null>(null);
export const errorMessage = writable<string | null>(null);
export const launchState = writable<string | null>(null);

export const selectedGame = derived([games, selectedId], ([$games, $selectedId]) => {
  return $games.find((game) => game.id === $selectedId) ?? $games[0] ?? null;
});

export const filteredGames = derived(
  [games, query, filter, sortMode],
  ([$games, $query, $filter, $sortMode]) => {
    const normalizedQuery = $query.trim().toLowerCase();

    let result = $games.filter((game) => {
      if ($filter === 'favorites' && !game.favorite) return false;
      if ($filter === 'recent' && !game.lastPlayedAt) return false;
      if (!normalizedQuery) return true;

      return [game.title, game.platform, ...(game.tags ?? [])]
        .filter(Boolean)
        .join(' ')
        .toLowerCase()
        .includes(normalizedQuery);
    });

    result = [...result].sort((left, right) => {
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

export async function addExecutable() {
  busyLabel.set('Adding executable');
  errorMessage.set(null);

  try {
    const game = await selectExecutable();
    if (!game) return;

    const library = await upsertGame(game);
    games.set(library.games);
    selectedId.set(game.id);
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
}

export async function deleteSelectedGame() {
  const id = get(selectedId);
  if (!id) return;

  const library = await removeGame(id);
  games.set(library.games);
  selectedId.set(library.games[0]?.id ?? null);
}

export async function launchSelectedGame() {
  const game = get(selectedGame);
  if (!game) return;

  launchState.set(`Launching ${game.title}`);
  errorMessage.set(null);

  try {
    const library = await launchGame(game.id);
    games.set(library.games);
    selectedId.set(game.id);
  } catch (error) {
    errorMessage.set(String(error));
  } finally {
    window.setTimeout(() => launchState.set(null), 900);
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
