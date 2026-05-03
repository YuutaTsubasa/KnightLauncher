<script lang="ts">
  import type { Platform } from './platforms';

  export let platform: Platform;
</script>

<span class="badge" title={platform.label} style="background: {platform.color1};">
  {#if platform.icon.kind === 'svg'}
    <svg
      viewBox={platform.icon.viewBox}
      width="58%"
      height="58%"
      fill="white"
      aria-hidden="true"
    >
      <path d={platform.icon.path} />
    </svg>
  {:else}
    <span
      class="mask"
      style="-webkit-mask-image: url({platform.icon.src}); mask-image: url({platform.icon.src});"
    ></span>
  {/if}
</span>

<style>
  .badge {
    display: flex;
    width: 100%;
    height: 100%;
    align-items: center;
    justify-content: center;
  }

  .mask {
    width: 60%;
    height: 60%;
    background: white;
    -webkit-mask-repeat: no-repeat;
    -webkit-mask-position: center;
    -webkit-mask-size: contain;
    mask-repeat: no-repeat;
    mask-position: center;
    mask-size: contain;
  }
</style>
