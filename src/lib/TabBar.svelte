<script>
  // The document tabs, inside the title bar.
  //
  // Presentation only: every decision about what a tab *is* lives in
  // `tabs.js`, so this file can be read as "draw these, report gestures".
  import { S } from "./strings.js";
  import { currentView } from "./tabs.js";
  import Icon from "./Icon.svelte";

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
      for (const el of bar.querySelectorAll(".tabs__item")) {
        el.style.width = `${el.getBoundingClientRect().width}px`;
      }
      locked = true;
    }
    onClose?.(index);
  }

  function unlock() {
    if (!locked) return;
    locked = false;
    for (const el of bar?.querySelectorAll(".tabs__item") ?? [])
      el.style.width = "";
  }

  // Middle click closes, the way it does everywhere else.
  function onAuxClick(event, index) {
    if (event.button === 1) {
      event.preventDefault();
      closeAt(index);
    }
  }

  // Reordering is pointer-based, not native drag-and-drop. HTML5 DnD in
  // WebKitGTK paints a red "no-drop" cursor over every gap the pointer crosses,
  // and no amount of dragover-accepting fully silenced the flicker. Driving it
  // by pointer means the OS never starts a drag, so there is no cursor to
  // fight — and a release in the wrong place simply does nothing.
  //
  // Plain object, not $state: none of this drives rendering.
  let drag = null; // { from, pointerId, startX, moved, el }
  let suppressClick = false;
  /// The live offset of the tab being carried, so it follows the pointer. This
  /// one IS reactive — it is the only visible sign that a tab is on the move.
  let dragView = $state(null); // { index, dx }

  function onPointerDown(event, index) {
    if (event.button !== 0) return; // left button only; middle click closes
    suppressClick = false;
    drag = {
      from: index,
      pointerId: event.pointerId,
      startX: event.clientX,
      moved: false,
      el: event.currentTarget,
    };
  }

  function onPointerMove(event) {
    if (!drag || event.pointerId !== drag.pointerId) return;
    if (!drag.moved) {
      // Below the threshold it is still a click, not a drag.
      if (Math.abs(event.clientX - drag.startX) < 5) return;
      drag.moved = true;
      try {
        drag.el.setPointerCapture(drag.pointerId);
      } catch {
        // No pointer capture (jsdom) — reordering still resolves on release.
      }
    }
    dragView = { index: drag.from, dx: event.clientX - drag.startX };
  }

  function onPointerUp(event) {
    if (!drag || event.pointerId !== drag.pointerId) return;
    const d = drag;
    drag = null;
    dragView = null;
    try {
      d.el.releasePointerCapture(d.pointerId);
    } catch {
      // ignore
    }
    if (!d.moved) return; // a plain click — let the tab select itself
    // The drag reordered, so the click that WebKit fires next must not select.
    suppressClick = true;
    const to = indexAt(event.clientX);
    if (to !== -1 && to !== d.from) onMove?.(d.from, to);
  }

  function onPointerCancel() {
    drag = null;
    dragView = null;
  }

  /// Which slot the pointer is over: the first tab whose middle is past it, or
  /// the last one when the pointer is beyond them all.
  function indexAt(clientX) {
    const items = [...(bar?.querySelectorAll(".tabs__item") ?? [])];
    for (let i = 0; i < items.length; i++) {
      const r = items[i].getBoundingClientRect();
      if (clientX < r.left + r.width / 2) return i;
    }
    return items.length - 1;
  }

  function onLabelClick(index) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    onSelect?.(index);
  }
</script>

<!-- The mouse handler lives on the wrapper, not on the tablist: attaching
     it to the role would make the list itself look interactive, when what is
     interactive are the tabs inside it. The wrapper is also a drag region, so
     the window moves when the pointer grabs the empty space between tabs. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="tabs"
  bind:this={bar}
  onmouseleave={unlock}
  data-tauri-drag-region
>
  <!-- The list fills the strip, so its empty gaps are the likeliest place to
       grab the window when few tabs are open — it must be a drag region too.
       The tabs and buttons inside carry no such attribute, so they stay
       clickable and still reorder by native drag. -->
  <div class="tabs__list" role="tablist" data-tauri-drag-region>
    {#each tabs as tab, i (i)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="tabs__item"
        class:tabs__item--active={i === active}
        class:tabs__item--dragging={dragView?.index === i}
        style={dragView?.index === i
          ? `transform: translateX(${dragView.dx}px);`
          : ""}
        role="presentation"
        onpointerdown={(e) => onPointerDown(e, i)}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerCancel}
        onauxclick={(e) => onAuxClick(e, i)}
      >
        <button
          class="tabs__label"
          role="tab"
          aria-selected={i === active}
          title={titleOf(currentView(tab))}
          onclick={() => onLabelClick(i)}
        >
          {titleOf(currentView(tab))}
        </button>
        {#if tabs.length > 1}
          <button
            class="tabs__close"
            aria-label={S.closeTab}
            onclick={() => closeAt(i)}
          >
            <Icon name="x-bold" size="12px" />
          </button>
        {/if}
      </div>
    {/each}
  </div>

  {#if onOpenNew}
    <button
      class="tabs__add"
      aria-label={S.newTab}
      onclick={() => onOpenNew?.()}
    >
      <Icon name="plus" size="16px" />
    </button>
  {/if}
</div>

<style>
  .tabs {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: var(--theme-space-8);
    padding-inline: var(--theme-space-8);
    overflow-x: auto;
    scrollbar-width: none;
    /* A tab is grabbed and clicked, never its text: selecting the label mid-
       drag looks broken. */
    user-select: none;
    -webkit-user-select: none;
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tabs__list {
    /* Content width, not flex:1 — so the + button glues to the last tab and
       the leftover space falls to the right (a drag region on .tabs). */
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--theme-space-8);
  }

  .tabs__item {
    position: relative;
    flex: 0 1 auto;
    min-width: 80px;
    max-width: 160px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--theme-space-8);
    padding: 5px var(--theme-space-10) 4px;
    background: var(--theme-surface);
    border-radius: var(--theme-radius-xs);
    color: var(--theme-ink-faint);
    /* Smooth settle back into place when a drag is released; the carried tab
       turns this off so it tracks the pointer with no lag. */
    transition: transform 0.12s ease;
  }
  .tabs__item--active {
    color: var(--theme-ink);
  }
  .tabs__item--dragging {
    transition: none;
    z-index: 2;
    opacity: 0.9;
    box-shadow: 0 4px 10px rgba(33, 44, 59, 0.18);
    cursor: grabbing;
  }
  .tabs__item--dragging .tabs__label {
    cursor: grabbing;
  }

  .tabs__label {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: var(--theme-text-14);
    color: inherit;
    cursor: pointer;
    text-align: start;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tabs__close {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--theme-ink-faint);
    border-radius: var(--theme-radius-xs);
    transition: color var(--theme-transition-fast);
  }
  .tabs__item--active .tabs__close {
    color: var(--theme-ink-muted);
  }
  .tabs__close:hover {
    color: var(--theme-ink);
  }

  .tabs__add {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--theme-ink-faint);
    border-radius: var(--theme-radius-xs);
    transition: color var(--theme-transition-fast);
  }
  .tabs__add:hover {
    color: var(--theme-ink-muted);
  }
</style>
