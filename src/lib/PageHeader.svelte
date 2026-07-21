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
    menu = [],
  } = $props();

  let open = $state(false);
</script>

<header class="page-header">
  <div class="arrows">
    <button disabled={!canBack} aria-label={S.goBack} onclick={() => onBack?.()}
      >←</button
    >
    <button
      disabled={!canForward}
      aria-label={S.goForward}
      onclick={() => onForward?.()}>→</button
    >
  </div>

  <h1>
    {title}{#if subtitle}<span class="subtitle"> — {subtitle}</span>{/if}
  </h1>

  {#if menu.length > 0}
    <div class="menu">
      <button aria-label={S.pageMenu} onclick={() => (open = !open)}>•••</button>
      {#if open}
        <!-- Closing on each item, not on the list: a click handler on a
             non-interactive element is invisible to a keyboard and to a
             screen reader. -->
        <ul>
          {#each menu as item (item.label)}
            <li>
              <button
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
  h1 {
    flex: 1;
    margin: 0;
    text-align: center;
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #666;
  }
  .subtitle {
    font-weight: 400;
  }
  .arrows,
  .menu {
    display: flex;
    gap: 0.15rem;
  }
  .menu {
    position: relative;
  }
  button {
    background: none;
    border: none;
    font: inherit;
    color: #666;
    cursor: pointer;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
  }
  button:hover:not(:disabled) {
    background: #eee;
    color: #222;
  }
  button:disabled {
    color: #ccc;
    cursor: default;
  }
  ul {
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
  ul button {
    width: 100%;
    text-align: left;
    color: #222;
    padding: 0.3rem 0.5rem;
  }
</style>
