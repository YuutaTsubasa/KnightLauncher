<script lang="ts">
  import {
    Activity,
    BadgePlus,
    FolderSearch,
    Gamepad2,
    Heart,
    Library,
    Monitor,
    Play,
    Sparkles,
    Star,
    Trash2
  } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Game } from './lib/types';
  import {
    addExecutable,
    busyLabel,
    deleteSelectedGame,
    errorMessage,
    filter,
    filteredGames,
    hasDualDisplay,
    initializeLibrary,
    launchSelectedGame,
    launchState,
    moveSelection,
    query,
    scanForGames,
    selectedGame,
    selectedId,
    toggleFavorite
  } from './lib/libraryStore';

  onMount(() => {
    initializeLibrary();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement) return;

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
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  });

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

  function formatDate(value: string | null) {
    if (!value) return 'Never played';
    return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(value));
  }

  function heroStyle(game: Game | null) {
    const url = imageUrl(game?.heroImage ?? null);
    if (!url) return '';
    return `background-image: linear-gradient(90deg, rgba(9, 12, 17, 0.92), rgba(9, 12, 17, 0.44)), url("${url}")`;
  }
</script>

<main class:dual={$hasDualDisplay}>
  <section class="top-display" style={heroStyle($selectedGame)}>
    <div class="status-bar">
      <div class="brand">
        <Gamepad2 size={22} />
        <span>KnightLauncher</span>
      </div>

      <div class="display-pill" title="Display mode">
        <Monitor size={16} />
        {$hasDualDisplay ? 'Dual display' : 'Single display'}
      </div>
    </div>

    {#if $selectedGame}
      <div class="hero-copy">
        <div class="eyebrow">
          <Sparkles size={16} />
          {$selectedGame.platform ?? 'Windows'}
        </div>
        <h1>{$selectedGame.title}</h1>
        <p>
          {$selectedGame.description ??
            'Ready to launch from your local Windows library. Add key art and a hero image later to make this profile feel complete.'}
        </p>

        <div class="hero-actions">
          <button class="primary-action" on:click={launchSelectedGame}>
            <Play size={20} fill="currentColor" />
            Launch
          </button>
          <button class="icon-action" title="Toggle favorite" on:click={() => toggleFavorite($selectedGame!)}>
            <Heart size={20} fill={$selectedGame.favorite ? 'currentColor' : 'none'} />
          </button>
          <button class="icon-action danger" title="Remove game" on:click={deleteSelectedGame}>
            <Trash2 size={19} />
          </button>
        </div>
      </div>

      <div class="detail-strip">
        <div>
          <span>Last played</span>
          <strong>{formatDate($selectedGame.lastPlayedAt)}</strong>
        </div>
        <div>
          <span>Launches</span>
          <strong>{$selectedGame.playCount}</strong>
        </div>
        <div>
          <span>Path</span>
          <strong title={$selectedGame.executablePath}>{$selectedGame.executablePath}</strong>
        </div>
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
  </section>
</main>
