<script>
  // The document tabs, above the centre panel.
  //
  // Presentation only: every decision about what a tab *is* lives in
  // `tabs.js`, so this file can be read as "draw these, report gestures".
  import { S } from "./strings.js";
  import { currentView } from "./tabs.js";

  let {
    tabs = [],
    active = 0,
    titleOf,
    onSelect,
    onClose,
    onOpenNew,
    onMove,
  } = $props();

  let bar;
  /// While closing with the mouse, tabs keep the width they had.
  ///
  /// This is the browser behaviour worth copying: the × of the next tab lands
  /// under the pointer that just clicked, so closing several in a row is one
  /// gesture instead of a hunt. Widths are released when the pointer leaves.
  let locked = $state(false);

  function closeAt(index) {
    if (bar && tabs.length > 2) {
      for (const el of bar.querySelectorAll(".tab")) {
        el.style.width = `${el.getBoundingClientRect().width}px`;
      }
      locked = true;
    }
    onClose?.(index);
  }

  function unlock() {
    if (!locked) return;
    locked = false;
    for (const el of bar?.querySelectorAll(".tab") ?? []) el.style.width = "";
  }

  // Middle click closes, the way it does everywhere else.
  function onAuxClick(event, index) {
    if (event.button === 1) {
      event.preventDefault();
      closeAt(index);
    }
  }

  let dragging = $state(null);

  function onDrop(event, index) {
    event.preventDefault();
    if (dragging !== null && dragging !== index) onMove?.(dragging, index);
    dragging = null;
  }
</script>

<!-- The mouse handler lives on the wrapper, not on the tablist: attaching
     it to the role would make the list itself look interactive, when what is
     interactive are the tabs inside it. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="tabbar-outer" bind:this={bar} onmouseleave={unlock}>
  <div class="tabbar" role="tablist">
  {#each tabs as tab, i (i)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="tab"
      class:active={i === active}
      role="presentation"
      draggable="true"
      ondragstart={() => (dragging = i)}
      ondragover={(e) => e.preventDefault()}
      ondrop={(e) => onDrop(e, i)}
      ondragend={() => (dragging = null)}
      onauxclick={(e) => onAuxClick(e, i)}
    >
      <button
        role="tab"
        aria-selected={i === active}
        title={titleOf(currentView(tab))}
        onclick={() => onSelect?.(i)}
      >
        {titleOf(currentView(tab))}
      </button>
      {#if tabs.length > 1}
        <button
          class="close"
          aria-label={S.closeTab}
          onclick={() => closeAt(i)}>×</button
        >
      {/if}
    </div>
  {/each}
  </div>
</div>

<style>
  .tabbar-outer {
    border-bottom: 1px solid #ccc;
  }
  .tabbar {
    display: flex;
    align-items: stretch;
    gap: 0.25rem;
    padding: 0.35rem 0.5rem 0;
    overflow-x: auto;
  }
  .tab {
    display: flex;
    align-items: center;
    border: 1px solid #ddd;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    background: #f6f6f6;
    /* Bounded on both ends: a one-word tab should not be a sliver, and a long
       note title should not push every other tab off the bar. */
    min-width: 6rem;
    max-width: 14rem;
    flex: 1 1 auto;
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
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    text-align: left;
  }
  .tab .close {
    flex: 0 0 auto;
    padding: 0.3rem 0.45rem 0.3rem 0;
    color: #888;
  }
  .tab .close:hover {
    color: #333;
  }
</style>
