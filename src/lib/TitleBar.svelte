<script>
  // The custom title bar of a frameless window.
  //
  // Three zones on one grid whose columns match the shell's sidebars, so the
  // tab strip lands exactly over the centre panel below it:
  //   [ brand | tabs (children) | window controls ]
  //
  // The window is frameless (no OS decorations), so this bar is the ONLY way to
  // move, maximize or close the window — it renders even before a notebook is
  // open. Empty areas carry `data-tauri-drag-region`: the OS drags the window
  // by them and double-click maximizes, exactly like a native title bar.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { S } from "./strings.js";

  let { children } = $props();

  const win = getCurrentWindow();
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="titlebar__brand" data-tauri-drag-region>
    <span class="titlebar__mark">M</span>
    <span class="titlebar__wordmark">Memo</span>
  </div>

  <div class="titlebar__tabs" data-tauri-drag-region>
    {@render children?.()}
  </div>

  <div class="titlebar__controls" data-tauri-drag-region>
    <button
      class="titlebar__control"
      aria-label={S.minimizeWindow}
      onclick={() => win.minimize()}
    >
      <svg viewBox="0 0 20 20" class="titlebar__glyph" aria-hidden="true">
        <circle cx="10" cy="10" r="9" />
        <line x1="6" y1="10" x2="14" y2="10" />
      </svg>
    </button>

    <button
      class="titlebar__control"
      aria-label={S.maximizeWindow}
      onclick={() => win.toggleMaximize()}
    >
      <svg viewBox="0 0 20 20" class="titlebar__glyph" aria-hidden="true">
        <circle cx="10" cy="10" r="9" />
        <rect x="6.25" y="6.25" width="7.5" height="7.5" rx="1" />
      </svg>
    </button>

    <button
      class="titlebar__control titlebar__control--close"
      aria-label={S.closeWindow}
      onclick={() => win.close()}
    >
      <svg viewBox="0 0 20 20" class="titlebar__glyph" aria-hidden="true">
        <circle cx="10" cy="10" r="9" />
        <line x1="6.5" y1="6.5" x2="13.5" y2="13.5" />
        <line x1="13.5" y1="6.5" x2="6.5" y2="13.5" />
      </svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    display: grid;
    grid-template-columns: var(--theme-sidebar-left) 1fr var(--theme-sidebar-right);
    align-items: center;
    height: var(--theme-titlebar-height);
    flex-shrink: 0;
    background: var(--theme-bg);
    /* The bar is a drag surface; text selection while dragging looks broken. */
    user-select: none;
  }

  /* ---- brand ---- */
  .titlebar__brand {
    display: flex;
    align-items: center;
    gap: var(--theme-space-10);
    padding-inline: var(--theme-space-12);
    min-width: 0;
  }
  .titlebar__mark {
    display: flex;
    align-items: center;
    justify-content: center;
    /* Non-interactive labels: let clicks fall through to the brand container,
       which is the drag region — otherwise grabbing the name/logo would not
       move the window. */
    pointer-events: none;
    height: 18px;
    padding-inline: var(--theme-space-6);
    background: var(--theme-brand);
    border-radius: var(--theme-radius-xs);
    color: var(--theme-on-brand);
    font-weight: var(--theme-weight-bold);
    font-size: var(--theme-text-16);
    letter-spacing: var(--theme-tracking-wide);
  }
  .titlebar__wordmark {
    pointer-events: none;
    color: var(--theme-brand);
    font-weight: var(--theme-weight-bold);
    font-size: var(--theme-text-16);
    letter-spacing: var(--theme-tracking-wide);
    white-space: nowrap;
  }

  /* ---- tabs zone (the centre column; TabBar renders inside) ---- */
  .titlebar__tabs {
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
  }

  /* ---- window controls ---- */
  .titlebar__controls {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--theme-space-12);
    padding-inline: var(--theme-space-12);
  }
  .titlebar__control {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--theme-ink-muted);
    border-radius: var(--theme-radius-pill);
    transition: color var(--theme-transition-fast);
  }
  .titlebar__glyph {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
  }
  .titlebar__control:hover {
    color: var(--theme-ink);
  }
  .titlebar__control--close:hover {
    color: var(--theme-danger);
  }
</style>
