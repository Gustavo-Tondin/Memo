<script>
  // The strip above the page: navigation arrows, the title, and the page
  // menu. The arrows act on the active tab's own history (see tabs.js).
  import { S } from "./strings.js";

  let {
    title,
    subtitle = "",
    canBack = false,
    canForward = false,
    onBack,
    onForward,
    onRenameTitle,
    menu = [],
  } = $props();

  let open = $state(false);

  // A menu left hanging over a screen the user has already left is a menu
  // whose items act on the wrong thing. Changing page closes it.
  $effect(() => {
    title;
    subtitle;
    open = false;
  });
</script>

<header class="page-header">
  <div class="page-header__nav">
    <button
      class="page-header__arrow"
      disabled={!canBack}
      aria-label={S.goBack}
      onclick={() => onBack?.()}>←</button
    >
    <button
      class="page-header__arrow"
      disabled={!canForward}
      aria-label={S.goForward}
      onclick={() => onForward?.()}>→</button
    >
  </div>

  <h1 class="page-header__heading">
    {#if onRenameTitle}
      <button
        class="page-header__name"
        title={S.promptRenameNote(title)}
        onclick={() => onRenameTitle()}
      >
        {title}
      </button>
    {:else}
      {title}
    {/if}{#if subtitle}<span class="page-header__subtitle"> — {subtitle}</span
      >{/if}
  </h1>

  {#if menu.length > 0}
    <div class="page-header__menu">
      <button
        class="page-header__menu-toggle"
        aria-label={S.pageMenu}
        onclick={() => (open = !open)}>•••</button
      >
      {#if open}
        <!-- Closing on each item, not on the list: a click handler on a
             non-interactive element is invisible to a keyboard and to a
             screen reader. -->
        <ul class="page-header__menu-list">
          {#each menu as item (item.label)}
            <li class="page-header__menu-item">
              <button
                class="page-header__menu-link"
                onclick={() => {
                  open = false;
                  item.run();
                }}>{item.label}</button
              >
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</header>

<style>
  .page-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
  }
  .page-header__heading {
    flex: 1;
    margin: 0;
    text-align: center;
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #666;
  }
  .page-header__subtitle {
    font-weight: 400;
  }
  /* The document's own name, clicked to rename it — the only place it
     appears, right under the tabs. */
  .page-header__name {
    font: inherit;
    color: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    padding: 0.1rem 0.4rem;
    background: none;
    border: none;
    cursor: pointer;
  }
  .page-header__name:hover {
    background: #eee;
    color: #222;
  }
  .page-header__nav,
  .page-header__menu {
    display: flex;
    gap: 0.15rem;
  }
  .page-header__menu {
    position: relative;
  }
  .page-header__arrow,
  .page-header__menu-toggle {
    background: none;
    border: none;
    font: inherit;
    color: #666;
    cursor: pointer;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
  }
  .page-header__arrow:hover:not(:disabled),
  .page-header__menu-toggle:hover:not(:disabled) {
    background: #eee;
    color: #222;
  }
  .page-header__arrow:disabled {
    color: #ccc;
    cursor: default;
  }
  .page-header__menu-list {
    position: absolute;
    right: 0;
    top: 1.6rem;
    z-index: 10;
    list-style: none;
    margin: 0;
    padding: 0.2rem;
    min-width: 10rem;
    background: #fff;
    border: 1px solid #ccc;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  }
  .page-header__menu-link {
    width: 100%;
    text-align: left;
    color: #222;
    padding: 0.3rem 0.5rem;
    background: none;
    border: none;
    font: inherit;
    cursor: pointer;
    border-radius: 4px;
  }
  .page-header__menu-link:hover {
    background: #eee;
  }
</style>
