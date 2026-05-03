<script lang="ts">
  import {
    Activity,
    ArrowDownAZ,
    BadgePlus,
    Eye,
    EyeOff,
    Filter,
    FolderSearch,
    Gamepad2,
    Joystick,
    Library,
    Move,
    Pencil,
    Image,
    Monitor,
    Play,
    RefreshCw,
    Scissors,
    Split,
    Star,
    Trophy,
    Unlink
  } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { emit, listen } from '@tauri-apps/api/event';
  import { get } from 'svelte/store';
  import {
    arrangeDisplays,
    loadSettings,
    retroAchievementsSearchGames,
    saveSettings,
    selectExecutablePath,
    selectFolder,
    selectImagePath,
    googleDownloadArtwork,
    googleImageSearch,
    steamGridDbDownloadArtwork,
    steamGridDbGameArtwork,
    steamGridDbSearchGames,
    swapDisplays
  } from './lib/tauri';
  import type { Game, GoogleImageResult, RaGameSearchResult, SteamGridDbAsset, SteamGridDbArtwork, SteamGridDbGame } from './lib/types';
  import PlatformBadge from './lib/PlatformBadge.svelte';
  import { PLATFORMS, frameGradient, isRaSupported, resolvePlatform } from './lib/platforms';
  import {
    addExecutable,
    busyLabel,
    displays,
    effectiveAchievements,
    errorMessage,
    filter,
    filteredGames,
    games,
    hasDualDisplay,
    initializeLibrary,
    launchSelectedGame,
    launchState,
    launchVariant,
    linkRetroAchievements,
    linkVariantRetroAchievements,
    mergeIntoTarget,
    moveSelection,
    pickingVariantsFor,
    query,
    refreshLibraryPreservingSelection,
    refreshRetroAchievements,
    refreshVariantRetroAchievements,
    renameVariantLabel,
    reorderMode,
    saveGameEdits,
    scanForGames,
    scanForRoms,
    selectedGame,
    selectedId,
    showingAchievementsFor,
    sortMode,
    splitVariantInto,
    swapSelectedWith,
    toggleFavorite,
    toggleHiddenForSelected,
    unlinkRetroAchievements,
    unlinkVariantRetroAchievements
  } from './lib/libraryStore';
  import type { LibraryFilter, SortMode } from './lib/types';

  type WindowRole = 'single' | 'detail' | 'library';
  type ControllerAction =
    | 'right'
    | 'left'
    | 'up'
    | 'down'
    | 'launch'
    | 'back'
    | 'swap'
    | 'achievements'
    | 'edit';

  let windowRole: WindowRole = 'single';
  let displayMessage: string | null = null;
  let controllerName: string | null = null;
  let suppressSelectionBroadcast = false;
  let editingGame: Game | null = null;
  let pickerIndex = 0;
  let mergePickerOpen = false;
  let mergePickerQuery = '';
  type RaLinkTarget = { kind: 'game' } | { kind: 'variant'; id: string };
  let raLinkTarget: RaLinkTarget = { kind: 'game' };

  const FILTER_ORDER: LibraryFilter[] = ['all', 'favorites', 'recent', 'hidden'];
  const SORT_ORDER: SortMode[] = ['title', 'recent', 'playCount', 'manual'];

  const filterLabel: Record<LibraryFilter, string> = {
    all: 'All',
    favorites: 'Favorites',
    recent: 'Recent',
    hidden: 'Hidden'
  };

  const sortLabel: Record<SortMode, string> = {
    title: 'A → Z',
    recent: 'Recently played',
    playCount: 'Most played',
    manual: 'Manual order'
  };

  function cycleFilter() {
    const current = $filter;
    const next = FILTER_ORDER[(FILTER_ORDER.indexOf(current) + 1) % FILTER_ORDER.length];
    filter.set(next);
  }

  function cycleSort() {
    const current = $sortMode;
    const next = SORT_ORDER[(SORT_ORDER.indexOf(current) + 1) % SORT_ORDER.length];
    sortMode.set(next);
  }

  type ArrowDirection = 'up' | 'down' | 'left' | 'right';

  function findSpatialTarget(direction: ArrowDirection): string | null {
    const list = $filteredGames;
    if (list.length < 2) return null;

    const currentId = $selectedId;
    if (!currentId) return list[0]?.id ?? null;

    const cards = document.querySelectorAll<HTMLElement>('.game-card');
    if (!cards.length || cards.length !== list.length) return null;

    const index = list.findIndex((game) => game.id === currentId);
    if (index < 0) return list[0].id;

    const currentTop = cards[index].offsetTop;
    const currentLeft = cards[index].offsetLeft;

    if (direction === 'left') {
      return index > 0 ? list[index - 1].id : null;
    }
    if (direction === 'right') {
      return index < list.length - 1 ? list[index + 1].id : null;
    }

    const offsets = Array.from(cards, (card) => ({ top: card.offsetTop, left: card.offsetLeft }));

    const targetRowTop = direction === 'down'
      ? offsets.map((offset) => offset.top).find((top) => top > currentTop)
      : offsets
          .map((offset) => offset.top)
          .filter((top) => top < currentTop)
          .reduce<number | null>((best, top) => (best === null || top > best ? top : best), null);

    if (targetRowTop === undefined || targetRowTop === null) return null;

    let bestIdx = -1;
    let bestDelta = Number.POSITIVE_INFINITY;
    offsets.forEach((offset, idx) => {
      if (offset.top !== targetRowTop) return;
      const delta = Math.abs(offset.left - currentLeft);
      if (delta < bestDelta) {
        bestDelta = delta;
        bestIdx = idx;
      }
    });

    return bestIdx >= 0 ? list[bestIdx].id : null;
  }

  function handleArrowDirection(direction: ArrowDirection) {
    if ($reorderMode) {
      const targetId = findSpatialTarget(direction);
      if (targetId) {
        swapSelectedWith(targetId);
      }
      return;
    }

    const targetId = findSpatialTarget(direction);
    if (targetId) {
      selectedId.set(targetId);
    } else {
      moveSelection(direction === 'right' || direction === 'down' ? 1 : -1);
    }
  }

  $: if ($pickingVariantsFor) {
    pickerIndex = 0;
  }
  let artworkSource: 'steamgriddb' | 'google' | 'retroachievements' = 'steamgriddb';
  let steamGridDbApiKey = '';
  let googleApiKey = '';
  let googleSearchEngineId = '';
  let retroAchievementsUser = '';
  let retroAchievementsApiKey = '';
  let raSearchQuery = '';
  let raSearchResults: RaGameSearchResult[] = [];
  let raBusy: string | null = null;
  let raError: string | null = null;
  let artworkQuery = '';
  let googleArtworkKind: 'cover' | 'hero' | 'logo' = 'hero';
  let artworkGames: SteamGridDbGame[] = [];
  let selectedArtworkGame: SteamGridDbGame | null = null;
  let artworkResults: SteamGridDbArtwork | null = null;
  let googleResults: GoogleImageResult[] = [];
  let artworkBusy: string | null = null;
  let artworkError: string | null = null;
  const isTauriRuntime = '__TAURI_INTERNALS__' in window;
  const controllerRepeatMs = 180;

  function readWindowRole(): WindowRole {
    const role = new URLSearchParams(window.location.search).get('window');
    return role === 'detail' || role === 'library' ? role : 'single';
  }

  onMount(() => {
    windowRole = readWindowRole();
    initializeLibrary()
      .then(() => {
        if (isTauriRuntime && get(displays).length < 2) {
          windowRole = 'single';
        }
        return arrangeDisplays();
      })
      .catch((error) => {
        displayMessage = String(error);
      });
    loadSettings()
      .then((settings) => {
        steamGridDbApiKey = settings.steamgriddbApiKey ?? '';
        googleApiKey = settings.googleApiKey ?? '';
        googleSearchEngineId = settings.googleSearchEngineId ?? '';
        retroAchievementsUser = settings.retroAchievementsUser ?? '';
        retroAchievementsApiKey = settings.retroAchievementsApiKey ?? '';
      })
      .catch(() => {});

    const unsubscribeSelectedId = selectedId.subscribe((id) => {
      if (suppressSelectionBroadcast || !id) return;
      if (isTauriRuntime) {
        emit('selected-game-changed', id).catch(() => {});
      }
    });

    const selectedListener = isTauriRuntime
      ? listen<string>('selected-game-changed', (event) => {
          if (event.payload === $selectedId) return;
          suppressSelectionBroadcast = true;
          selectedId.set(event.payload);
          queueMicrotask(() => {
            suppressSelectionBroadcast = false;
          });
        })
      : Promise.resolve(() => {});

    const unlistenDisplaySync = isTauriRuntime
      ? listen<void>('display-layout-changed', () => {
          displayMessage = 'Display layout updated';
          window.setTimeout(() => {
            displayMessage = null;
          }, 1200);
        })
      : Promise.resolve(() => {});

    const unlistenLibrarySync = isTauriRuntime
      ? listen<void>('library-changed', () => {
          refreshLibraryPreservingSelection();
        })
      : Promise.resolve(() => {});

    const unlistenGameFinished = isTauriRuntime
      ? listen<{ gameId: string; variantId: string | null }>('game-finished', (event) => {
          const { gameId, variantId } = event.payload;
          const game = get(games).find((entry) => entry.id === gameId);
          if (!game) return;
          if (variantId) {
            const variant = game.variants.find((entry) => entry.id === variantId);
            if (variant?.retroAchievements) {
              refreshVariantRetroAchievements(gameId, variantId).catch(() => {});
              return;
            }
          }
          if (game.retroAchievements) {
            refreshRetroAchievements(gameId).catch(() => {});
          }
        })
      : Promise.resolve(() => {});

    const onKeyDown = (event: KeyboardEvent) => {
      if (editingGame && (event.key === 'Escape' || event.key.toLowerCase() === 'b')) {
        event.preventDefault();
        closeEditor();
        return;
      }

      if (editingGame && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        saveEditor();
        return;
      }

      if ($showingAchievementsFor) {
        if (event.key === 'Escape' || event.key.toLowerCase() === 'b' || event.key.toLowerCase() === 'y') {
          event.preventDefault();
          showingAchievementsFor.set(null);
          return;
        }
        return;
      }

      const picking = $pickingVariantsFor;
      if (picking) {
        if (event.key === 'Escape' || event.key.toLowerCase() === 'b') {
          event.preventDefault();
          pickingVariantsFor.set(null);
          return;
        }
        if (event.key === 'ArrowDown' || event.key === 'ArrowRight') {
          event.preventDefault();
          pickerIndex = (pickerIndex + 1) % picking.variants.length;
          return;
        }
        if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') {
          event.preventDefault();
          pickerIndex = (pickerIndex - 1 + picking.variants.length) % picking.variants.length;
          return;
        }
        if (event.key === 'Enter' || event.key.toLowerCase() === 'a') {
          event.preventDefault();
          const variant = picking.variants[pickerIndex];
          if (variant) launchVariant(picking, variant.id);
          return;
        }
        return;
      }

      if (event.target instanceof HTMLInputElement) return;
      if (event.target instanceof HTMLTextAreaElement) return;

      if (event.key === 'ArrowRight') {
        event.preventDefault();
        handleArrowDirection('right');
      }
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        handleArrowDirection('left');
      }
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        handleArrowDirection('down');
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        handleArrowDirection('up');
      }
      if (event.key === 'Enter' || event.key.toLowerCase() === 'a') {
        event.preventDefault();
        launchSelectedGame();
      }
      if (event.key === 'Escape' || event.key.toLowerCase() === 'b') {
        if ($reorderMode) {
          reorderMode.set(false);
        } else {
          query.set('');
        }
      }
      if (event.key.toLowerCase() === 'x') {
        event.preventDefault();
        swapDisplays()
          .then(() => {
            displayMessage = 'Swapped displays';
            window.setTimeout(() => {
              displayMessage = null;
            }, 1200);
          })
          .catch((error) => {
            displayMessage = String(error);
          });
      }
      if (event.key.toLowerCase() === 'e') {
        event.preventDefault();
        openEditor();
      }
      if (event.key.toLowerCase() === 'y') {
        event.preventDefault();
        toggleAchievementsModal();
      }
    };

    const onRoleChange = (event: Event) => {
      const nextRole = (event as CustomEvent<WindowRole>).detail;
      if (nextRole === 'single' || nextRole === 'detail' || nextRole === 'library') {
        windowRole = nextRole;
      }
    };

    const onGamepadConnected = (event: GamepadEvent) => {
      controllerName = event.gamepad.id;
      displayMessage = 'Controller connected';
      window.setTimeout(() => {
        displayMessage = null;
      }, 1200);
    };

    const onGamepadDisconnected = () => {
      controllerName = null;
      displayMessage = 'Controller disconnected';
      window.setTimeout(() => {
        displayMessage = null;
      }, 1200);
    };

    const gamepadLoop = startControllerLoop((action) => {
      handleControllerAction(action);
    });

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('knightlauncher-window-role', onRoleChange);
    window.addEventListener('gamepadconnected', onGamepadConnected);
    window.addEventListener('gamepaddisconnected', onGamepadDisconnected);
    return () => {
      unsubscribeSelectedId();
      selectedListener.then((unsubscribe) => unsubscribe());
      unlistenDisplaySync.then((unsubscribe) => unsubscribe());
      unlistenLibrarySync.then((unsubscribe) => unsubscribe());
      unlistenGameFinished.then((unsubscribe) => unsubscribe());
      gamepadLoop.stop();
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('knightlauncher-window-role', onRoleChange);
      window.removeEventListener('gamepadconnected', onGamepadConnected);
      window.removeEventListener('gamepaddisconnected', onGamepadDisconnected);
    };
  });

  function handleControllerAction(action: ControllerAction) {
    if (windowRole === 'detail') return;

    if (editingGame) {
      if (action === 'back' || action === 'edit') closeEditor();
      return;
    }

    if ($showingAchievementsFor) {
      if (action === 'back' || action === 'achievements') {
        showingAchievementsFor.set(null);
      }
      return;
    }

    if ($reorderMode && action === 'back') {
      reorderMode.set(false);
      return;
    }

    const picking = $pickingVariantsFor;
    if (picking) {
      if (action === 'back') {
        pickingVariantsFor.set(null);
      } else if (action === 'launch') {
        const variant = picking.variants[pickerIndex];
        if (variant) launchVariant(picking, variant.id);
      } else if (action === 'right' || action === 'down') {
        pickerIndex = (pickerIndex + 1) % picking.variants.length;
      } else if (action === 'left' || action === 'up') {
        pickerIndex = (pickerIndex - 1 + picking.variants.length) % picking.variants.length;
      }
      return;
    }

    if (action === 'swap') {
      swapDisplays()
        .then(() => {
          displayMessage = 'Swapped displays';
          window.setTimeout(() => {
            displayMessage = null;
          }, 1200);
        })
        .catch((error) => {
          displayMessage = String(error);
        });
      return;
    }

    if (action === 'right' || action === 'left' || action === 'up' || action === 'down') {
      handleArrowDirection(action);
      return;
    }

    if (action === 'launch') {
      launchSelectedGame();
      return;
    }

    if (action === 'back') {
      query.set('');
      return;
    }

    if (action === 'achievements') {
      toggleAchievementsModal();
      return;
    }

    if (action === 'edit') {
      openEditor();
    }
  }

  function startControllerLoop(onAction: (action: ControllerAction) => void) {
    let frame = 0;
    let previousPressed = new Set<string>();
    const lastRepeated = new Map<string, number>();

    const buttonPressed = (gamepad: Gamepad, index: number) => gamepad.buttons[index]?.pressed ?? false;
    const axisActive = (gamepad: Gamepad, index: number, direction: 1 | -1) => {
      const value = gamepad.axes[index] ?? 0;
      return direction > 0 ? value > 0.55 : value < -0.55;
    };

    const emitOnce = (id: string, pressed: boolean, action: ControllerAction) => {
      if (pressed && !previousPressed.has(id)) onAction(action);
    };

    const emitRepeated = (id: string, pressed: boolean, action: ControllerAction, now: number) => {
      if (!pressed) {
        lastRepeated.delete(id);
        return;
      }

      const last = lastRepeated.get(id) ?? 0;
      if (now - last >= controllerRepeatMs) {
        lastRepeated.set(id, now);
        onAction(action);
      }
    };

    const tick = (now: number) => {
      const gamepads = navigator.getGamepads?.() ?? [];
      const gamepad = Array.from(gamepads).find((item): item is Gamepad => Boolean(item));
      const nextPressed = new Set<string>();

      if (gamepad) {
        controllerName = gamepad.id;

        const pressed = {
          a: buttonPressed(gamepad, 0),
          b: buttonPressed(gamepad, 1),
          x: buttonPressed(gamepad, 2),
          y: buttonPressed(gamepad, 3),
          lb: buttonPressed(gamepad, 4),
          rb: buttonPressed(gamepad, 5),
          menu: buttonPressed(gamepad, 9),
          up: buttonPressed(gamepad, 12) || axisActive(gamepad, 1, -1),
          down: buttonPressed(gamepad, 13) || axisActive(gamepad, 1, 1),
          left: buttonPressed(gamepad, 14) || axisActive(gamepad, 0, -1),
          right: buttonPressed(gamepad, 15) || axisActive(gamepad, 0, 1)
        };

        for (const [id, value] of Object.entries(pressed)) {
          if (value) nextPressed.add(id);
        }

        emitOnce('a', pressed.a, 'launch');
        emitOnce('b', pressed.b, 'back');
        emitOnce('x', pressed.x, 'swap');
        emitOnce('y', pressed.y, 'achievements');
        emitOnce('menu', pressed.menu, 'edit');
        emitRepeated('right', pressed.right || pressed.rb, 'right', now);
        emitRepeated('left', pressed.left || pressed.lb, 'left', now);
        emitRepeated('down', pressed.down, 'down', now);
        emitRepeated('up', pressed.up, 'up', now);
      }

      previousPressed = nextPressed;
      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return {
      stop() {
        cancelAnimationFrame(frame);
      }
    };
  }

  function toggleAchievementsModal() {
    if ($showingAchievementsFor) {
      showingAchievementsFor.set(null);
      return;
    }
    const game = get(selectedGame);
    if (!game || !effectiveAchievements(game)) return;
    showingAchievementsFor.set(game);
  }

  $: if ($showingAchievementsFor) {
    const liveGame = $selectedGame;
    if (liveGame && liveGame.id === $showingAchievementsFor.id && liveGame !== $showingAchievementsFor) {
      showingAchievementsFor.set(liveGame);
    }
  }

  async function saveRetroAchievementsSettings() {
    raBusy = 'Saving RetroAchievements credentials';
    raError = null;
    try {
      const current = await loadSettings();
      const settings = await saveSettings({
        ...current,
        retroAchievementsUser: retroAchievementsUser.trim() || null,
        retroAchievementsApiKey: retroAchievementsApiKey.trim() || null
      });
      retroAchievementsUser = settings.retroAchievementsUser ?? '';
      retroAchievementsApiKey = settings.retroAchievementsApiKey ?? '';
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  async function searchRetroAchievementsGames() {
    if (!editingGame) return;
    const platformLabel = editingGame.platform ?? '';
    const platformId = resolvePlatform(platformLabel).id;
    const queryText = raSearchQuery.trim() || editingGame.title.trim();
    if (!queryText) return;
    raBusy = 'Searching RetroAchievements';
    raError = null;
    raSearchResults = [];
    try {
      raSearchResults = await retroAchievementsSearchGames(queryText, platformId);
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  async function applyRetroAchievementsLink(result: RaGameSearchResult) {
    if (!editingGame) return;
    raBusy = 'Linking RetroAchievements';
    raError = null;
    try {
      if (raLinkTarget.kind === 'variant') {
        await linkVariantRetroAchievements(editingGame.id, raLinkTarget.id, result.id);
      } else {
        await linkRetroAchievements(editingGame.id, result.id);
      }
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
      raSearchResults = [];
      raLinkTarget = { kind: 'game' };
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  function startVariantRaSearch(variantId: string, defaultQuery: string) {
    raLinkTarget = { kind: 'variant', id: variantId };
    raSearchQuery = defaultQuery;
    raSearchResults = [];
    raError = null;
  }

  function cancelVariantRaSearch() {
    raLinkTarget = { kind: 'game' };
    raSearchResults = [];
    raError = null;
  }

  async function refreshVariantLinkInEditor(variantId: string) {
    if (!editingGame) return;
    raBusy = 'Refreshing variant achievements';
    raError = null;
    try {
      await refreshVariantRetroAchievements(editingGame.id, variantId);
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  async function unlinkVariantInEditor(variantId: string) {
    if (!editingGame) return;
    raBusy = 'Unlinking variant achievements';
    raError = null;
    try {
      await unlinkVariantRetroAchievements(editingGame.id, variantId);
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  async function commitVariantRename(variantId: string, label: string) {
    if (!editingGame) return;
    const trimmed = label.trim();
    if (!trimmed) return;
    try {
      await renameVariantLabel(editingGame.id, variantId, trimmed);
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
    } catch {}
  }

  function setVariantLabelLocal(variantId: string, label: string) {
    if (!editingGame) return;
    editingGame = {
      ...editingGame,
      variants: editingGame.variants.map((variant) =>
        variant.id === variantId ? { ...variant, label } : variant
      )
    };
  }

  async function refreshRetroAchievementsLink() {
    if (!editingGame) return;
    raBusy = 'Refreshing RetroAchievements';
    raError = null;
    try {
      await refreshRetroAchievements(editingGame.id);
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  async function unlinkRetroAchievementsLink() {
    if (!editingGame) return;
    raBusy = 'Unlinking RetroAchievements';
    raError = null;
    try {
      await unlinkRetroAchievements(editingGame.id);
      const linked = get(selectedGame);
      if (linked && linked.id === editingGame.id) {
        editingGame = { ...linked, tags: [...linked.tags] };
      }
    } catch (error) {
      raError = String(error);
    } finally {
      raBusy = null;
    }
  }

  $: filteredMergeCandidates = editingGame
    ? $games
        .filter(
          (candidate) =>
            candidate.id !== editingGame!.id &&
            candidate.variants.length > 0 &&
            (resolvePlatform(candidate.platform).id === resolvePlatform(editingGame!.platform).id) &&
            candidate.title.toLowerCase().includes(mergePickerQuery.toLowerCase().trim())
        )
        .slice(0, 20)
    : [];

  async function mergeCandidate(candidate: Game) {
    if (!editingGame) return;
    try {
      await mergeIntoTarget(candidate.id, editingGame.id);
      const refreshed = $games.find((entry) => entry.id === editingGame!.id);
      if (refreshed) {
        editingGame = { ...refreshed, tags: [...refreshed.tags] };
      }
      mergePickerOpen = false;
      mergePickerQuery = '';
    } catch {}
  }

  async function splitVariantFromEditor(variantId: string) {
    if (!editingGame) return;
    try {
      const library = await splitVariantInto(editingGame.id, variantId);
      const refreshed = library.games.find((entry) => entry.id === editingGame!.id);
      if (refreshed) {
        editingGame = { ...refreshed, tags: [...refreshed.tags] };
      }
    } catch {}
  }

  function openEditor() {
    if (windowRole === 'detail') return;
    const game = get(selectedGame);
    if (!game) return;
    editingGame = {
      ...game,
      platform: resolvePlatform(game.platform).label,
      tags: [...game.tags]
    };
    mergePickerOpen = false;
    mergePickerQuery = '';
    artworkQuery = game.title;
    artworkGames = [];
    selectedArtworkGame = null;
    artworkResults = null;
    googleResults = [];
    artworkError = null;
    artworkBusy = null;
  }

  function closeEditor() {
    editingGame = null;
  }

  async function saveEditor() {
    if (!editingGame) return;
    await saveGameEdits({
      ...editingGame,
      title: editingGame.title.trim() || 'Untitled Game',
      executablePath: editingGame.executablePath.trim(),
      launchArgs: editingGame.launchArgs.trim(),
      workingDirectory: editingGame.workingDirectory.trim(),
      coverImage: editingGame.coverImage?.trim() || null,
      heroImage: editingGame.heroImage?.trim() || null,
      logoImage: editingGame.logoImage?.trim() || null,
      description: editingGame.description?.trim() || null,
      platform: editingGame.platform?.trim() || null,
      tags: editingGame.tags.map((tag) => tag.trim()).filter(Boolean)
    });
    closeEditor();
  }

  async function pickExecutableForEditor() {
    const path = await selectExecutablePath();
    if (!path || !editingGame) return;

    const separator = path.includes('\\') ? '\\' : '/';
    editingGame = {
      ...editingGame,
      executablePath: path,
      workingDirectory: path.split(separator).slice(0, -1).join(separator) || editingGame.workingDirectory
    };
  }

  async function pickWorkingDirectoryForEditor() {
    const path = await selectFolder();
    if (!path || !editingGame) return;
    editingGame = { ...editingGame, workingDirectory: path };
  }

  async function pickImageForEditor(field: 'coverImage' | 'heroImage' | 'logoImage') {
    const path = await selectImagePath();
    if (!path || !editingGame) return;
    editingGame = { ...editingGame, [field]: path };
  }

  async function saveSteamGridDbSettings() {
    artworkBusy = 'Saving SteamGridDB key';
    artworkError = null;
    try {
      const current = await loadSettings();
      const settings = await saveSettings({
        ...current,
        steamgriddbApiKey: steamGridDbApiKey.trim() || null,
        googleApiKey: googleApiKey.trim() || null,
        googleSearchEngineId: googleSearchEngineId.trim() || null
      });
      steamGridDbApiKey = settings.steamgriddbApiKey ?? '';
      googleApiKey = settings.googleApiKey ?? '';
      googleSearchEngineId = settings.googleSearchEngineId ?? '';
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function searchSteamGridDbArtwork() {
    const query = artworkQuery.trim() || editingGame?.title.trim() || '';
    if (!query) return;

    artworkBusy = 'Searching SteamGridDB';
    artworkError = null;
    artworkResults = null;
    googleResults = [];
    selectedArtworkGame = null;

    try {
      artworkGames = await steamGridDbSearchGames(query);
      if (artworkGames[0]) {
        await loadArtworkForGame(artworkGames[0]);
      }
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function loadArtworkForGame(game: SteamGridDbGame) {
    selectedArtworkGame = game;
    artworkBusy = `Loading artwork for ${game.name}`;
    artworkError = null;

    try {
      artworkResults = await steamGridDbGameArtwork(game.id);
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function applyArtwork(asset: SteamGridDbAsset) {
    if (!editingGame) return;

    artworkBusy = 'Downloading artwork';
    artworkError = null;
    try {
      const path = await steamGridDbDownloadArtwork(asset.url, asset.kind, editingGame.id);
      const field = asset.kind === 'hero' ? 'heroImage' : asset.kind === 'logo' ? 'logoImage' : 'coverImage';
      editingGame = { ...editingGame, [field]: path };
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function saveGoogleSettings() {
    artworkBusy = 'Saving Google Search settings';
    artworkError = null;
    try {
      const current = await loadSettings();
      const settings = await saveSettings({
        ...current,
        steamgriddbApiKey: steamGridDbApiKey.trim() || null,
        googleApiKey: googleApiKey.trim() || null,
        googleSearchEngineId: googleSearchEngineId.trim() || null
      });
      steamGridDbApiKey = settings.steamgriddbApiKey ?? '';
      googleApiKey = settings.googleApiKey ?? '';
      googleSearchEngineId = settings.googleSearchEngineId ?? '';
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function searchGoogleArtwork() {
    const query = artworkQuery.trim() || editingGame?.title.trim() || '';
    if (!query) return;

    artworkBusy = 'Searching Google Images';
    artworkError = null;
    artworkResults = null;
    artworkGames = [];
    selectedArtworkGame = null;

    try {
      googleResults = await googleImageSearch(query);
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function applyRaArtwork(url: string | null, kind: 'cover' | 'hero' | 'logo') {
    if (!url || !editingGame) return;
    artworkBusy = 'Downloading RetroAchievements artwork';
    artworkError = null;
    try {
      const path = await googleDownloadArtwork(url, kind, editingGame.id);
      const field = kind === 'hero' ? 'heroImage' : kind === 'logo' ? 'logoImage' : 'coverImage';
      editingGame = { ...editingGame, [field]: path };
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  async function applyGoogleArtwork(result: GoogleImageResult) {
    if (!editingGame) return;

    artworkBusy = 'Downloading Google image';
    artworkError = null;
    try {
      const path = await googleDownloadArtwork(result.link, googleArtworkKind, editingGame.id);
      const field = googleArtworkKind === 'hero' ? 'heroImage' : googleArtworkKind === 'logo' ? 'logoImage' : 'coverImage';
      editingGame = { ...editingGame, [field]: path };
    } catch (error) {
      artworkError = String(error);
    } finally {
      artworkBusy = null;
    }
  }

  function imageUrl(path: string | null) {
    if (!path) return '';
    try {
      return convertFileSrc(path);
    } catch {
      return path;
    }
  }

  function initials(title: string) {
    return title
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]?.toUpperCase())
      .join('');
  }

  function heroStyle(game: Game | null) {
    const url = imageUrl(game?.heroImage ?? null);
    if (!url) return '';
    return `background-image: url("${url}")`;
  }

  function artworkList(results: SteamGridDbArtwork | null, kind: 'covers' | 'heroes' | 'logos' | 'icons') {
    return results?.[kind] ?? [];
  }
</script>

<main class:dual={$hasDualDisplay} class:detail-only={windowRole === 'detail'} class:library-only={windowRole === 'library'}>
  <section class="top-display" class:has-hero={!!$selectedGame?.heroImage} style={heroStyle($selectedGame)}>
    <div class="status-bar">
      <div class="brand">
        <Gamepad2 size={22} />
        <span>KnightLauncher</span>
      </div>

      <div class="display-pill" title="Display mode">
        <Monitor size={16} />
        {windowRole === 'single' ? ($hasDualDisplay ? 'Dual display' : 'Single display') : windowRole}
      </div>

      {#if controllerName}
        <div class="display-pill" title={controllerName}>
          <Gamepad2 size={16} />
          Controller
        </div>
      {/if}
    </div>

    {#if $selectedGame}
      <div class="hero-copy">
        {#if $selectedGame.logoImage}
          <img class="hero-logo" src={imageUrl($selectedGame.logoImage)} alt={$selectedGame.title} />
        {:else}
          <h1>{$selectedGame.title}</h1>
        {/if}
      </div>

      {@const heroAchievements = effectiveAchievements($selectedGame)}
      {#if heroAchievements}
        <div class="achievement-slot" title="Achievements">
          <Trophy size={18} />
          <span>{heroAchievements.achievementsEarned} / {heroAchievements.achievementsTotal}</span>
        </div>
      {/if}
    {:else}
      <div class="hero-copy empty-hero">
        <div class="eyebrow">
          <Library size={16} />
          Empty library
        </div>
        <h1>Add your first game</h1>
        <p>Use the bottom display controls to add a single executable or scan a folder for Windows games.</p>
      </div>
    {/if}
  </section>

  <section class="bottom-display">
    <div class="library-panel">
      <header class="library-header">
        <div class="library-title">
          <h2>Game Library</h2>
          <div class="library-meta">
            <p>{$filteredGames.length} games</p>
            <button class="chip" title="Filter" on:click={cycleFilter}>
              <Filter size={13} />
              {filterLabel[$filter]}
            </button>
            <button class="chip" title="Sort order" on:click={cycleSort}>
              <ArrowDownAZ size={13} />
              {sortLabel[$sortMode]}
            </button>
          </div>
        </div>

        <div class="clean-actions">
          <button title="Add executable" on:click={addExecutable}>
            <BadgePlus size={18} />
          </button>
          <button title="Scan folder" on:click={scanForGames}>
            <FolderSearch size={18} />
          </button>
          <button title="Scan EmuDeck ROMs" on:click={scanForRoms}>
            <Joystick size={18} />
          </button>
          <button
            title={$selectedGame?.hidden ? 'Unhide selected' : 'Hide selected'}
            on:click={toggleHiddenForSelected}
            disabled={!$selectedGame}
          >
            {#if $selectedGame?.hidden}
              <Eye size={18} />
            {:else}
              <EyeOff size={18} />
            {/if}
          </button>
          <button
            title="Reorder mode"
            class:active={$reorderMode}
            on:click={() => reorderMode.update((value) => !value)}
          >
            <Move size={18} />
          </button>
          <button title="Edit selected game" on:click={openEditor} disabled={!$selectedGame}>
            <Pencil size={18} />
          </button>
          {#if $selectedGame && effectiveAchievements($selectedGame)}
            <button title="Achievements (Y)" on:click={toggleAchievementsModal}>
              <Trophy size={18} />
            </button>
          {/if}
          <button title="Launch selected game" on:click={launchSelectedGame} disabled={!$selectedGame}>
            <Play size={18} fill="currentColor" />
          </button>
        </div>
      </header>

      {#if $reorderMode}
        <div class="notice launch">
          <Move size={16} />
          Reorder mode · Arrows / D-pad swap with neighbour · B / Esc to exit
        </div>
      {/if}

      {#if $busyLabel}
        <div class="notice busy">
          <Activity size={18} />
          {$busyLabel}
        </div>
      {/if}

      {#if $launchState}
        <div class="notice launch">
          <Play size={18} />
          {$launchState}
        </div>
      {/if}

      {#if $errorMessage}
        <div class="notice error">{$errorMessage}</div>
      {/if}

      {#if displayMessage}
        <div class="notice launch">
          <Monitor size={18} />
          {displayMessage}
        </div>
      {/if}

      {#if $filteredGames.length}
        <div class="game-grid" aria-label="Games">
          {#each $filteredGames as game}
            {@const platform = resolvePlatform(game.platform)}
            <button
              class="game-card"
              class:selected={$selectedId === game.id}
              class:reordering={$reorderMode && $selectedId === game.id}
              class:hidden-card={game.hidden}
              title={game.title}
              aria-label={game.title}
              on:click={() => selectedId.set(game.id)}
              on:dblclick={launchSelectedGame}
            >
              <div class="platform-frame" style="background: {frameGradient(platform)};">
                <div class="cover">
                  {#if game.coverImage}
                    <img src={imageUrl(game.coverImage)} alt="" />
                  {:else}
                    <span>{initials(game.title)}</span>
                  {/if}
                  {#if game.favorite}
                    <Star size={16} fill="currentColor" />
                  {/if}
                </div>
                <span class="badge-corner">
                  <PlatformBadge {platform} />
                </span>
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <div class="empty-library">
          <FolderSearch size={42} />
          <h3>No games match this view</h3>
          <p>Add an executable, scan a folder, or clear the current search.</p>
        </div>
      {/if}
    </div>

    {#if $showingAchievementsFor && effectiveAchievements($showingAchievementsFor)}
      {@const link = effectiveAchievements($showingAchievementsFor)!}
      <div class="achievements-panel" role="dialog" aria-label="Achievements">
        <div class="achievements-header">
          <div>
            <p>Achievements</p>
            <h3>{$showingAchievementsFor.title}</h3>
            <small>{link.achievementsEarned} / {link.achievementsTotal} earned · {link.pointsEarned} / {link.pointsTotal} pts</small>
          </div>
          <button class="icon-action" title="Close" on:click={() => showingAchievementsFor.set(null)}>X</button>
        </div>
        <div class="achievements-list">
          {#each link.achievements as ach}
            <div class="achievement-row" class:earned={ach.earnedDate !== null}>
              <img src={imageUrl(ach.earnedDate ? ach.badgePath : ach.badgeLockedPath)} alt="" />
              <div class="achievement-meta">
                <strong>{ach.title}</strong>
                <p>{ach.description}</p>
                <small>
                  {#if ach.earnedDate}
                    Earned {new Date(ach.earnedDate).toLocaleDateString()} · {ach.points} pts
                  {:else}
                    Locked · {ach.points} pts
                  {/if}
                </small>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if $pickingVariantsFor}
      {@const picking = $pickingVariantsFor}
      <div class="variant-picker" role="dialog" aria-label="Choose version">
        <div class="variant-picker-header">
          <div>
            <p>Choose version</p>
            <h3>{picking.title}</h3>
          </div>
          <button class="icon-action" title="Close" on:click={() => pickingVariantsFor.set(null)}>X</button>
        </div>
        <div class="variant-list">
          {#each picking.variants as variant, idx}
            <button
              type="button"
              class="variant-row"
              class:selected={pickerIndex === idx}
              on:click={() => launchVariant(picking, variant.id)}
              on:mouseenter={() => (pickerIndex = idx)}
            >
              <strong>{variant.label}</strong>
              <span class="meta">
                {variant.playCount} plays
                {#if variant.lastPlayedAt}
                  · last {new Date(variant.lastPlayedAt).toLocaleDateString()}
                {/if}
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if editingGame}
      <div class="edit-panel" role="dialog" aria-label="Edit game">
        <div class="edit-panel-header">
          <div>
            <p>Edit Game</p>
            <h3>{editingGame.title || 'Untitled Game'}</h3>
          </div>
          <button class="icon-action" title="Close editor" on:click={closeEditor}>X</button>
        </div>

        <div class="edit-form">
          <label>
            <span>Title</span>
            <input bind:value={editingGame.title} />
          </label>

          <label>
            <span>Platform</span>
            <select bind:value={editingGame.platform}>
              {#each PLATFORMS as platformOption}
                <option value={platformOption.label}>{platformOption.label}</option>
              {/each}
            </select>
          </label>

          <label class="wide">
            <span>Description</span>
            <textarea bind:value={editingGame.description} rows="3"></textarea>
          </label>

          {#if editingGame.variants.length > 0}
            <div class="wide variant-section">
              <span>ROM Variants</span>
              <div class="variant-rows">
                {#each editingGame.variants as variant (variant.id)}
                  <div class="variant-edit-row">
                    <div class="variant-edit-info">
                      <input
                        class="variant-label-input"
                        value={variant.label}
                        placeholder="Variant label"
                        on:input={(event) => setVariantLabelLocal(variant.id, (event.currentTarget as HTMLInputElement).value)}
                        on:change={(event) => commitVariantRename(variant.id, (event.currentTarget as HTMLInputElement).value)}
                      />
                      <small class="path" title={variant.romPath}>{variant.romPath}</small>
                      <small>{variant.playCount} plays{variant.lastPlayedAt ? ` · last ${new Date(variant.lastPlayedAt).toLocaleDateString()}` : ''}</small>
                      {#if variant.retroAchievements}
                        {@const variantRa = variant.retroAchievements}
                        <small class="variant-ra">
                          <Trophy size={12} />
                          {variantRa.title} · {variantRa.achievementsEarned} / {variantRa.achievementsTotal}
                        </small>
                      {/if}
                    </div>
                    <div class="variant-edit-actions">
                      {#if variant.retroAchievements}
                        <button type="button" title="Refresh achievements" on:click={() => refreshVariantLinkInEditor(variant.id)}>
                          <RefreshCw size={12} />
                        </button>
                        <button type="button" title="Unlink achievements" on:click={() => unlinkVariantInEditor(variant.id)}>
                          <Unlink size={12} />
                        </button>
                      {:else if isRaSupported(editingGame.platform)}
                        <button
                          type="button"
                          title="Link RetroAchievements override"
                          on:click={() => startVariantRaSearch(variant.id, variant.label)}
                        >
                          <Trophy size={12} />
                          Link RA
                        </button>
                      {/if}
                      {#if editingGame.variants.length > 1}
                        <button type="button" title="Split into separate game" on:click={() => splitVariantFromEditor(variant.id)}>
                          <Split size={12} />
                          Split
                        </button>
                      {/if}
                    </div>
                  </div>
                  {#if raLinkTarget.kind === 'variant' && raLinkTarget.id === variant.id}
                    <div class="variant-ra-search">
                      <div class="path-row">
                        <input
                          type="text"
                          bind:value={raSearchQuery}
                          placeholder="Search RA games for this variant"
                        />
                        <button type="button" on:click={searchRetroAchievementsGames}>
                          <Trophy size={14} />
                          Search
                        </button>
                        <button type="button" on:click={cancelVariantRaSearch}>Cancel</button>
                      </div>
                      {#if raBusy}
                        <div class="artwork-message">{raBusy}</div>
                      {/if}
                      {#if raError}
                        <div class="artwork-message error">{raError}</div>
                      {/if}
                      {#if raSearchResults.length}
                        <div class="ra-results">
                          {#each raSearchResults as result}
                            <button
                              type="button"
                              class="ra-result-row"
                              on:click={() => applyRetroAchievementsLink(result)}
                            >
                              {#if result.iconUrl}
                                <img src={result.iconUrl} alt="" />
                              {/if}
                              <div class="ra-result-meta">
                                <strong>{result.title}</strong>
                                <small>{result.numAchievements} achievements · {result.points} pts</small>
                              </div>
                            </button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/if}
                {/each}
              </div>
            </div>
          {/if}

          {#if editingGame.variants.length > 0 || editingGame.romSystem}
            <div class="wide merge-section">
              <div class="ra-heading">
                <span>Combine games</span>
                <strong>Merge another game's variants into this one</strong>
              </div>
              {#if !mergePickerOpen}
                <div class="path-row">
                  <button type="button" on:click={() => (mergePickerOpen = true)}>
                    <Scissors size={14} />
                    Merge from another game
                  </button>
                </div>
              {:else}
                <div class="path-row">
                  <input
                    type="text"
                    bind:value={mergePickerQuery}
                    placeholder="Search by title"
                    autocomplete="off"
                  />
                  <button
                    type="button"
                    on:click={() => {
                      mergePickerOpen = false;
                      mergePickerQuery = '';
                    }}
                  >
                    Cancel
                  </button>
                </div>
                {#if filteredMergeCandidates.length}
                  <div class="ra-results">
                    {#each filteredMergeCandidates as candidate}
                      <button
                        type="button"
                        class="ra-result-row"
                        on:click={() => mergeCandidate(candidate)}
                      >
                        <div class="ra-result-meta">
                          <strong>{candidate.title}</strong>
                          <small>{candidate.variants.length} variant{candidate.variants.length === 1 ? '' : 's'} · {candidate.playCount} plays</small>
                        </div>
                      </button>
                    {/each}
                  </div>
                {:else}
                  <small class="merge-hint">No other games on this platform have variants.</small>
                {/if}
              {/if}
            </div>
          {/if}

          <label class="wide">
            <span>Executable</span>
            <div class="path-row">
              <input bind:value={editingGame.executablePath} />
              <button type="button" on:click={pickExecutableForEditor}>Browse</button>
            </div>
          </label>

          <label class="wide">
            <span>Working directory</span>
            <div class="path-row">
              <input bind:value={editingGame.workingDirectory} />
              <button type="button" on:click={pickWorkingDirectoryForEditor}>Browse</button>
            </div>
          </label>

          <label class="wide">
            <span>Launch arguments</span>
            <input bind:value={editingGame.launchArgs} placeholder="-fullscreen" />
          </label>

          <label class="wide">
            <span>Cover image</span>
            <div class="path-row">
              <input bind:value={editingGame.coverImage} placeholder="Optional image path" />
              <button type="button" on:click={() => pickImageForEditor('coverImage')}>
                <Image size={16} />
                Pick
              </button>
            </div>
          </label>

          <label class="wide">
            <span>Hero image</span>
            <div class="path-row">
              <input bind:value={editingGame.heroImage} placeholder="Optional image path" />
              <button type="button" on:click={() => pickImageForEditor('heroImage')}>
                <Image size={16} />
                Pick
              </button>
            </div>
          </label>

          <label class="wide">
            <span>Logo image</span>
            <div class="path-row">
              <input bind:value={editingGame.logoImage} placeholder="Optional transparent logo path" />
              <button type="button" on:click={() => pickImageForEditor('logoImage')}>
                <Image size={16} />
                Pick
              </button>
            </div>
          </label>

          <div class="wide artwork-picker">
            <div class="artwork-heading">
              <div>
                <span>Artwork search</span>
                <strong>Find artwork</strong>
              </div>
              <div class="source-tabs" role="tablist" aria-label="Artwork source">
                <button type="button" class:selected={artworkSource === 'steamgriddb'} on:click={() => (artworkSource = 'steamgriddb')}>
                  SteamGridDB
                </button>
                <button type="button" class:selected={artworkSource === 'google'} on:click={() => (artworkSource = 'google')}>
                  Google
                </button>
                {#if effectiveAchievements(editingGame)}
                  <button
                    type="button"
                    class:selected={artworkSource === 'retroachievements'}
                    on:click={() => (artworkSource = 'retroachievements')}
                  >
                    RetroAchievements
                  </button>
                {/if}
              </div>
            </div>

            {#if artworkSource === 'steamgriddb'}
              <div class="path-row">
                <input
                  type="password"
                  bind:value={steamGridDbApiKey}
                  placeholder="SteamGridDB API key"
                  autocomplete="off"
                />
                <button type="button" on:click={saveSteamGridDbSettings}>Save</button>
              </div>

              <div class="path-row">
                <input bind:value={artworkQuery} placeholder="Search by game title" />
                <button type="button" on:click={searchSteamGridDbArtwork}>
                  <Image size={16} />
                  Search
                </button>
              </div>
            {:else if artworkSource === 'google'}
              <div class="path-row">
                <input
                  type="password"
                  bind:value={googleApiKey}
                  placeholder="Google API key"
                  autocomplete="off"
                />
              </div>

              <div class="path-row">
                <input
                  type="password"
                  bind:value={googleSearchEngineId}
                  placeholder="Programmable Search Engine ID"
                  autocomplete="off"
                />
                <button type="button" on:click={saveGoogleSettings}>Save</button>
              </div>

              <div class="path-row">
                <input bind:value={artworkQuery} placeholder="Search Google Images" />
                <select bind:value={googleArtworkKind} title="Target image field">
                  <option value="cover">Cover</option>
                  <option value="hero">Hero</option>
                  <option value="logo">Logo</option>
                </select>
                <button type="button" on:click={searchGoogleArtwork}>
                  <Image size={16} />
                  Search
                </button>
              </div>
            {/if}

            {#if artworkBusy}
              <div class="artwork-message">{artworkBusy}</div>
            {/if}

            {#if artworkError}
              <div class="artwork-message error">{artworkError}</div>
            {/if}

            {#if artworkSource === 'steamgriddb' && artworkGames.length}
              <div class="sgdb-games" aria-label="SteamGridDB game matches">
                {#each artworkGames as game}
                  <button
                    type="button"
                    class:selected={selectedArtworkGame?.id === game.id}
                    on:click={() => loadArtworkForGame(game)}
                  >
                    {game.name}
                  </button>
                {/each}
              </div>
            {/if}

            {#if artworkSource === 'steamgriddb' && artworkResults}
              <div class="artwork-section">
                <span>Cover / Icon</span>
                <div class="artwork-grid square">
                  {#each [...artworkList(artworkResults, 'covers'), ...artworkList(artworkResults, 'icons')] as asset}
                    <button type="button" title="Use as cover" on:click={() => applyArtwork(asset)}>
                      <img src={asset.thumb} alt="" />
                    </button>
                  {/each}
                </div>
              </div>

              <div class="artwork-section">
                <span>Hero</span>
                <div class="artwork-grid wide">
                  {#each artworkList(artworkResults, 'heroes') as asset}
                    <button type="button" title="Use as hero" on:click={() => applyArtwork(asset)}>
                      <img src={asset.thumb} alt="" />
                    </button>
                  {/each}
                </div>
              </div>

              <div class="artwork-section">
                <span>Logo</span>
                <div class="artwork-grid logo">
                  {#each artworkList(artworkResults, 'logos') as asset}
                    <button type="button" title="Use as logo" on:click={() => applyArtwork(asset)}>
                      <img src={asset.thumb} alt="" />
                    </button>
                  {/each}
                </div>
              </div>
            {/if}

            {#if artworkSource === 'google' && googleResults.length}
              <div class="artwork-section">
                <span>Google Images</span>
                <div class="artwork-grid google">
                  {#each googleResults as result}
                    <button type="button" title={result.title} on:click={() => applyGoogleArtwork(result)}>
                      <img src={result.thumbnail} alt="" />
                      <small>{result.width ?? '?'} x {result.height ?? '?'}</small>
                    </button>
                  {/each}
                </div>
              </div>
            {/if}

            {#if artworkSource === 'retroachievements'}
              {@const ra = effectiveAchievements(editingGame)}
              {#if ra}
                {#if ra.boxArtUrl}
                  <div class="artwork-section">
                    <span>Box art (cover)</span>
                    <div class="artwork-grid square">
                      <button type="button" title="Use as cover" on:click={() => applyRaArtwork(ra.boxArtUrl, 'cover')}>
                        <img src={ra.boxArtUrl} alt="" />
                      </button>
                    </div>
                  </div>
                {/if}
                {#if ra.titleUrl}
                  <div class="artwork-section">
                    <span>Title screen (hero)</span>
                    <div class="artwork-grid wide">
                      <button type="button" title="Use as hero" on:click={() => applyRaArtwork(ra.titleUrl, 'hero')}>
                        <img src={ra.titleUrl} alt="" />
                      </button>
                    </div>
                  </div>
                {/if}
                {#if ra.ingameUrl}
                  <div class="artwork-section">
                    <span>In-game (hero)</span>
                    <div class="artwork-grid wide">
                      <button type="button" title="Use as hero" on:click={() => applyRaArtwork(ra.ingameUrl, 'hero')}>
                        <img src={ra.ingameUrl} alt="" />
                      </button>
                    </div>
                  </div>
                {/if}
                {#if ra.iconUrl}
                  <div class="artwork-section">
                    <span>Icon (logo)</span>
                    <div class="artwork-grid logo">
                      <button type="button" title="Use as logo" on:click={() => applyRaArtwork(ra.iconUrl, 'logo')}>
                        <img src={ra.iconUrl} alt="" />
                      </button>
                    </div>
                  </div>
                {/if}
                {#if !ra.boxArtUrl && !ra.titleUrl && !ra.ingameUrl && !ra.iconUrl}
                  <div class="artwork-message">No RetroAchievements artwork available; refresh the link to fetch it.</div>
                {/if}
              {/if}
            {/if}
          </div>

          {#if isRaSupported(editingGame.platform)}
            <div class="wide ra-section">
              <div class="ra-heading">
                <span>RetroAchievements</span>
                <strong>Track progress for this game</strong>
              </div>

              <div class="path-row">
                <input
                  type="text"
                  bind:value={retroAchievementsUser}
                  placeholder="RA username"
                  autocomplete="off"
                />
              </div>
              <div class="path-row">
                <input
                  type="password"
                  bind:value={retroAchievementsApiKey}
                  placeholder="RA API key"
                  autocomplete="off"
                />
                <button type="button" on:click={saveRetroAchievementsSettings}>Save</button>
              </div>

              {#if raBusy}
                <div class="artwork-message">{raBusy}</div>
              {/if}
              {#if raError}
                <div class="artwork-message error">{raError}</div>
              {/if}

              {#if editingGame.retroAchievements}
                {@const link = editingGame.retroAchievements}
                <div class="ra-link">
                  <strong>{link.title}</strong>
                  <small>{link.consoleName}</small>
                  <small>{link.achievementsEarned} / {link.achievementsTotal} achievements · {link.pointsEarned} / {link.pointsTotal} pts</small>
                  {#if link.lastSyncedAt}
                    <small>Last synced {new Date(link.lastSyncedAt).toLocaleString()}</small>
                  {/if}
                  <div class="path-row">
                    <button type="button" on:click={refreshRetroAchievementsLink}>
                      <RefreshCw size={14} />
                      Refresh
                    </button>
                    <button type="button" on:click={unlinkRetroAchievementsLink}>
                      <Unlink size={14} />
                      Unlink
                    </button>
                  </div>
                </div>
              {:else if raLinkTarget.kind === 'game'}
                <div class="path-row">
                  <input
                    type="text"
                    bind:value={raSearchQuery}
                    placeholder="Search RA games (game-level link)"
                  />
                  <button type="button" on:click={searchRetroAchievementsGames}>
                    <Trophy size={14} />
                    Search
                  </button>
                </div>

                {#if raSearchResults.length}
                  <div class="ra-results">
                    {#each raSearchResults as result}
                      <button
                        type="button"
                        class="ra-result-row"
                        on:click={() => applyRetroAchievementsLink(result)}
                      >
                        {#if result.iconUrl}
                          <img src={result.iconUrl} alt="" />
                        {/if}
                        <div class="ra-result-meta">
                          <strong>{result.title}</strong>
                          <small>{result.numAchievements} achievements · {result.points} pts</small>
                        </div>
                      </button>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {/if}

          <label class="wide">
            <span>Tags</span>
            <input
              value={editingGame.tags.join(', ')}
              on:input={(event) => {
                editingGame = {
                  ...editingGame!,
                  tags: (event.currentTarget as HTMLInputElement).value.split(',')
                };
              }}
              placeholder="Action RPG, Favorite"
            />
          </label>
        </div>

        <div class="edit-actions">
          <button on:click={closeEditor}>Cancel</button>
          <button class="save" on:click={saveEditor}>Save</button>
        </div>
      </div>
    {/if}
  </section>
</main>
