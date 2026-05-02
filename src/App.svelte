<script lang="ts">
  import {
    Activity,
    BadgePlus,
    FolderSearch,
    Gamepad2,
    Library,
    Pencil,
    Image,
    Monitor,
    Play,
    Sparkles,
    Star
  } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { emit, listen } from '@tauri-apps/api/event';
  import { get } from 'svelte/store';
  import {
    arrangeDisplays,
    loadSettings,
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
  import type { Game, GoogleImageResult, SteamGridDbAsset, SteamGridDbArtwork, SteamGridDbGame } from './lib/types';
  import {
    addExecutable,
    busyLabel,
    displays,
    errorMessage,
    filteredGames,
    hasDualDisplay,
    initializeLibrary,
    launchSelectedGame,
    launchState,
    moveSelection,
    query,
    refreshLibraryPreservingSelection,
    saveGameEdits,
    scanForGames,
    selectedGame,
    selectedId,
    toggleFavorite
  } from './lib/libraryStore';

  type WindowRole = 'single' | 'detail' | 'library';
  type ControllerAction = 'next' | 'previous' | 'launch' | 'back' | 'swap' | 'favorite' | 'edit';

  let windowRole: WindowRole = 'single';
  let displayMessage: string | null = null;
  let controllerName: string | null = null;
  let suppressSelectionBroadcast = false;
  let editingGame: Game | null = null;
  let artworkSource: 'steamgriddb' | 'google' = 'steamgriddb';
  let steamGridDbApiKey = '';
  let googleApiKey = '';
  let googleSearchEngineId = '';
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

      if (event.target instanceof HTMLInputElement) return;
      if (event.target instanceof HTMLTextAreaElement) return;

      if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
        event.preventDefault();
        moveSelection(1);
      }
      if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
        event.preventDefault();
        moveSelection(-1);
      }
      if (event.key === 'Enter' || event.key.toLowerCase() === 'a') {
        event.preventDefault();
        launchSelectedGame();
      }
      if (event.key === 'Escape' || event.key.toLowerCase() === 'b') {
        query.set('');
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

    if (action === 'next') {
      moveSelection(1);
      return;
    }

    if (action === 'previous') {
      moveSelection(-1);
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

    if (action === 'favorite') {
      const game = get(selectedGame);
      if (game) toggleFavorite(game);
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
        emitOnce('y', pressed.y, 'favorite');
        emitOnce('menu', pressed.menu, 'edit');
        emitRepeated('right', pressed.right || pressed.down || pressed.rb, 'next', now);
        emitRepeated('left', pressed.left || pressed.up || pressed.lb, 'previous', now);
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

  function openEditor() {
    if (windowRole === 'detail') return;
    const game = get(selectedGame);
    if (!game) return;
    editingGame = {
      ...game,
      tags: [...game.tags]
    };
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
      const settings = await saveSettings({
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
      const settings = await saveSettings({
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
    return `background-image: linear-gradient(90deg, rgba(9, 12, 17, 0.92), rgba(9, 12, 17, 0.44)), url("${url}")`;
  }

  function artworkList(results: SteamGridDbArtwork | null, kind: 'covers' | 'heroes' | 'logos' | 'icons') {
    return results?.[kind] ?? [];
  }
</script>

<main class:dual={$hasDualDisplay} class:detail-only={windowRole === 'detail'} class:library-only={windowRole === 'library'}>
  <section class="top-display" style={heroStyle($selectedGame)}>
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

      <div class="achievement-slot" aria-hidden="true">
        <Sparkles size={18} />
      </div>
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
        <div>
          <h2>Game Library</h2>
          <p>{$filteredGames.length} games</p>
        </div>

        <div class="clean-actions">
          <button title="Add executable" on:click={addExecutable}>
            <BadgePlus size={18} />
          </button>
          <button title="Scan folder" on:click={scanForGames}>
            <FolderSearch size={18} />
          </button>
          <button title="Edit selected game" on:click={openEditor} disabled={!$selectedGame}>
            <Pencil size={18} />
          </button>
          <button title="Launch selected game" on:click={launchSelectedGame} disabled={!$selectedGame}>
            <Play size={18} fill="currentColor" />
          </button>
        </div>
      </header>

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
            <button
              class="game-card"
              class:selected={$selectedId === game.id}
              on:click={() => selectedId.set(game.id)}
              on:dblclick={launchSelectedGame}
            >
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

              <div class="game-meta">
                <strong>{game.title}</strong>
                <span>{game.platform ?? 'Windows'}</span>
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
            <input bind:value={editingGame.platform} placeholder="Windows" />
          </label>

          <label class="wide">
            <span>Description</span>
            <textarea bind:value={editingGame.description} rows="3"></textarea>
          </label>

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
            {:else}
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
          </div>

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
