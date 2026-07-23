<script>
  // The `notes` widget: a board of note cards, or a folder tree, plus search.
  //
  // Same note in both views — the layout is a preference, never a change to
  // the file (spec 5). Opening a note hands over to the editor; this screen
  // only ever lists.
  import { api } from "./api.js";
  import { S } from "./strings.js";

  let {
    widget,
    readOnly = false,
    notesInbox = "Inbox",
    onChanged,
    onError,
    onOpenNote,
    reloadKey = 0,
  } = $props();

  // The widget's folder is its address for every notes command.
  let folder = $derived(widget?.folder ?? null);

  let notes = $state([]);
  let folders = $state([]);
  let query = $state("");
  /// `grid` (cards, Keep-like) or `tree` (by folder).
  ///
  /// The config option picks the starting layout and the user's choice wins
  /// from then on — hence a null-until-chosen override rather than a state
  /// seeded from the prop, which would freeze on the value the widget had
  /// when it first rendered.
  let chosenLayout = $state(null);
  let layout = $derived(
    chosenLayout ?? (widget?.options?.layout === "tree" ? "tree" : "grid"),
  );
  let openFolder = $state(null);

  $effect(() => {
    folder;
    query;
    reloadKey;
    load();
  });

  async function load() {
    if (!folder) return;
    try {
      [notes, folders] = await Promise.all([
        api.listNotes(folder, query),
        api.noteFolders(folder),
      ]);
    } catch (e) {
      onError?.(e);
    }
  }

  async function act(fn) {
    try {
      await fn();
      await load();
      onChanged?.();
    } catch (e) {
      onError?.(e);
    }
  }

  const create = () =>
    act(async () => {
      const title = prompt(S.promptNewNote, S.newNoteTitle);
      if (!title) return;
      // A note created from a folder lands in it; from the board, in the
      // inbox — the spec's "loose notes go to Notes/Inbox".
      const target = openFolder ?? notesInbox;
      const path = await api.createNote(folder, target, title.trim());
      onOpenNote?.(path, folder);
    });

  const createFolder = () =>
    act(async () => {
      const name = prompt(S.promptNewNoteFolder);
      if (!name) return;
      const parent = openFolder ? `${openFolder}/` : "";
      await api.createNoteFolder(folder, `${parent}${name.trim()}`);
    });

  const togglePin = (entry) =>
    act(() => api.setNotePinned(folder, entry.path, !entry.pinned));

  const renameFolder = () =>
    act(async () => {
      if (!openFolder) return;
      const current = openFolder.split("/").pop();
      const next = prompt(S.promptRenameFolder(current), current);
      if (!next || next.trim() === current) return;
      openFolder = await api.renameNoteFolder(folder, openFolder, next.trim());
    });

  const deleteFolder = () =>
    act(async () => {
      if (!openFolder) return;
      const name = openFolder.split("/").pop();
      if (!confirm(S.confirmDeleteFolder(name))) return;
      const moved = await api.deleteNoteFolder(folder, openFolder);
      openFolder = null;
      if (moved > 0) onError?.({ kind: "info", message: S.folderEmptied(moved, name) });
    });

  // In the tree view, only the notes of the folder being looked at.
  let shown = $derived(
    layout === "tree" && openFolder !== null
      ? notes.filter((n) => n.folder === openFolder)
      : notes,
  );
</script>

<!-- Opening a note is the shell's business: it becomes a document tab, the
     same as a list. This screen only ever lists. -->
<div class="notes-widget">
  <div class="notes-widget__bar">
    <input
      class="notes-widget__search"
      placeholder={S.searchNotes}
      aria-label={S.searchNotes}
      bind:value={query}
    />
    <button
      class="notes-widget__bar-button"
      class:notes-widget__bar-button--active={layout === "grid"}
      onclick={() => (chosenLayout = "grid")}>{S.gridView}</button
    >
    <button
      class="notes-widget__bar-button"
      class:notes-widget__bar-button--active={layout === "tree"}
      onclick={() => (chosenLayout = "tree")}>{S.treeView}</button
    >
    {#if !readOnly}
      <button class="notes-widget__bar-button" onclick={create}>{S.newNote}</button>
      <button class="notes-widget__bar-button" onclick={createFolder}
        >{S.newNoteFolder}</button
      >
    {/if}
  </div>

  {#if layout === "tree"}
    <nav class="notes-widget__folders">
      <button
        class="notes-widget__folder"
        class:notes-widget__folder--active={openFolder === null}
        onclick={() => (openFolder = null)}>{S.allNotes}</button
      >
      {#each folders as name (name)}
        <button
          class="notes-widget__folder"
          class:notes-widget__folder--active={openFolder === name}
          onclick={() => (openFolder = name)}>{name}</button
        >
      {/each}
    </nav>

    <!-- Folder actions live next to the folder they act on, and only when
         one is open — a folder is not deletable from the board view, where
         nothing says which one you mean. -->
    {#if !readOnly && openFolder}
      <p class="notes-widget__folder-actions">
        <button class="notes-widget__folder-action" onclick={renameFolder}
          >{S.renameFolder}</button
        >
        <button class="notes-widget__folder-action" onclick={deleteFolder}
          >{S.deleteFolder}</button
        >
      </p>
    {/if}
  {/if}

  {#if shown.length === 0}
    <p class="notes-widget__empty">{query.trim() ? S.noNotesFound : S.noNotes}</p>
  {:else}
    <ul
      class="notes-widget__board"
      class:notes-widget__board--tree={layout === "tree"}
    >
      {#each shown as entry (entry.path)}
        <li
          class="notes-widget__item"
          class:notes-widget__item--pinned={entry.pinned}
        >
          <button
            class="notes-widget__card"
            onclick={() => onOpenNote?.(entry.path, folder)}
          >
            <strong class="notes-widget__card-title">{entry.title}</strong>
            <span class="notes-widget__preview">{entry.preview || S.emptyNote}</span>
            <small class="notes-widget__meta">
              {entry.folder}
              {#if entry.pinned}· {S.pinned}{/if}
            </small>
          </button>
          {#if !readOnly}
            <button
              class="notes-widget__pin"
              onclick={() => togglePin(entry)}
              aria-label={entry.pinned ? S.unpin : S.pin}>★</button
            >
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .notes-widget__bar {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 0.75rem;
  }
  .notes-widget__search {
    flex: 1;
    font: inherit;
  }
  .notes-widget__bar-button--active {
    font-weight: 600;
  }
  .notes-widget__folders {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-bottom: 0.75rem;
  }
  .notes-widget__folder {
    background: none;
    border: 1px solid #ddd;
    border-radius: 12px;
    padding: 0.1rem 0.6rem;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .notes-widget__folder--active {
    background: #eef2ff;
    border-color: #b9c6f5;
  }
  .notes-widget__folder-actions {
    display: flex;
    gap: 0.5rem;
    margin: -0.4rem 0 0.75rem;
    font-size: 0.85rem;
  }
  .notes-widget__folder-action {
    background: none;
    border: none;
    color: #666;
    font: inherit;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
  }
  /* The grid is a masonry-ish board of cards; the tree view is one column,
     since it is already filtered to a folder. */
  .notes-widget__board {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr));
    gap: 0.6rem;
  }
  .notes-widget__board--tree {
    grid-template-columns: 1fr;
  }
  .notes-widget__item {
    position: relative;
    border: 1px solid #ddd;
    border-radius: 6px;
  }
  .notes-widget__item--pinned {
    border-color: #d9c26a;
  }
  .notes-widget__card {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    font: inherit;
    cursor: pointer;
    padding: 0.6rem 0.7rem;
  }
  .notes-widget__preview {
    color: #555;
    font-size: 0.85rem;
    line-height: 1.35;
    max-height: 5.4em;
    overflow: hidden;
  }
  .notes-widget__meta {
    color: #888;
    font-size: 0.75rem;
  }
  .notes-widget__pin {
    position: absolute;
    top: 0.3rem;
    right: 0.35rem;
    background: none;
    border: none;
    cursor: pointer;
    color: #bbb;
    font-size: 0.9rem;
  }
  .notes-widget__item--pinned .notes-widget__pin {
    color: #c8a52e;
  }
  .notes-widget__empty {
    color: #666;
  }
</style>
