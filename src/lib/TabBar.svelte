<script>
  // The document tabs, above the centre panel.
  //
  // Presentation only: every decision about what a tab *is* lives in
  // `tabs.js`, so this file can be read as "draw these, report clicks".
  import { S } from "./strings.js";
  import { currentView } from "./tabs.js";

  let { tabs = [], active = 0, titleOf, onSelect, onClose } = $props();
</script>

<div class="tabbar" role="tablist">
  {#each tabs as tab, i}
    <div class="tab" class:active={i === active} role="presentation">
      <button
        role="tab"
        aria-selected={i === active}
        onclick={() => onSelect?.(i)}
      >
        {titleOf(currentView(tab))}
      </button>
      {#if tabs.length > 1}
        <button
          class="close"
          aria-label={S.closeTab}
          onclick={() => onClose?.(i)}>×</button
        >
      {/if}
    </div>
  {/each}
</div>

<style>
  .tabbar {
    display: flex;
    align-items: stretch;
    gap: 0.25rem;
    padding: 0.35rem 0.5rem 0;
    border-bottom: 1px solid #ccc;
    overflow-x: auto;
  }
  .tab {
    display: flex;
    align-items: center;
    border: 1px solid #ddd;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    background: #f6f6f6;
    /* The active tab merges into the page below it, which is what makes a
       tab read as "this is the sheet you are on". */
    margin-bottom: -1px;
  }
  .tab.active {
    background: #fff;
    border-color: #ccc;
    border-bottom: 1px solid #fff;
  }
  .tab button {
    background: none;
    border: none;
    font: inherit;
    cursor: pointer;
    padding: 0.3rem 0.6rem;
    white-space: nowrap;
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab .close {
    padding: 0.3rem 0.45rem 0.3rem 0;
    color: #888;
  }
  .tab .close:hover {
    color: #333;
  }
</style>
