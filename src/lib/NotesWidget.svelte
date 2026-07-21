<script>
  // The `notes` widget: a board of note cards, or a folder tree, plus search.
  //
  // Same note in both views — the layout is a preference, never a change to
  // the file (spec 5). Opening a note hands over to the editor; this screen
  // only ever lists.
  import { api } from "./api.js";
  import { S } from "./strings.js";
  import NoteEditor from "./NoteEditor.svelte";

  let {
    widget,
    readOnly = false,
    notesInbox = "Inbox",
    onChanged,
    onError,
    reloadKey = 0,
  } = $props();

  // The widget's folder is its address for every notes command.
  let folder = $derived(widget?.folder ?? null);

  let notes = $state([]);
  let folders = $state([]);
  let query = $state("");
  let open = $state(null);
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
      open = await api.createNote(folder, target, title.trim());
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

  // In the tree view, only the notes of the folder being looked at.
  let shown = $derived(
    layout === "tree" && openFolder !== null
      ? notes.filter((n) => n.folder === openFolder)
      : notes,
  );
</script>

{#if open}
  <NoteEditor
    {folder}
    path={open}
    {readOnly}
    onSaved={() => act(async () => {})}
    onError={(e) => onError?.(e)}
    onClose={() => (open = null)}
    onRenamed={(path) => (open = path)}
  />
{:else}
  <div class="bar">
    <input
      class="search"
      placeholder={S.searchNotes}
      aria-label={S.searchNotes}
      bind:value={query}
    />
    <button
      class:active={layout === "grid"}
      onclick={() => (chosenLayout = "grid")}>{S.gridView}</button
    >
    <button
      class:active={layout === "tree"}
      onclick={() => (chosenLayout = "tree")}>{S.treeView}</button
    >
    {#if !readOnly}
      <button onclick={create}>{S.newNote}</button>
      <button onclick={createFolder}>{S.newNoteFolder}</button>
    {/if}
  </div>

  {#if layout === "tree"}
    <nav class="folders">
      <button
        class:active={openFolder === null}
        onclick={() => (openFolder = null)}>{S.allNotes}</button
      >
      {#each folders as name (name)}
        <button
          class:active={openFolder === name}
          onclick={() => (openFolder = name)}>{name}</button
        >
      {/each}
    </nav>
  {/if}

  {#if shown.length === 0}
    <p class="empty">{query.trim() ? S.noNotesFound : S.noNotes}</p>
  {:else}
    <ul class="board" class:tree={layout === "tree"}>
      {#each shown as entry (entry.path)}
        <li class:pinned={entry.pinned}>
          <button class="card" onclick={() => (open = entry.path)}>
            <strong>{entry.title}</strong>
            <span class="preview">{entry.preview || S.emptyNote}</span>
            <small>
              {entry.folder}
              {#if entry.pinned}· {S.pinned}{/if}
            </small>
          </button>
          {#if !readOnly}
            <button
              class="pin"
              onclick={() => togglePin(entry)}
              aria-label={entry.pinned ? S.unpin : S.pin}>★</button
            >
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  .bar {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 0.75rem;
  }
  .search {
    flex: 1;
    font: inherit;
  }
  .bar button.active {
    font-weight: 600;
  }
  .folders {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-bottom: 0.75rem;
  }
  .folders button {
    background: none;
    border: 1px solid #ddd;
    border-radius: 12px;
    padding: 0.1rem 0.6rem;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .folders button.active {
    background: #eef2ff;
    border-color: #b9c6f5;
  }
  /* The grid is a masonry-ish board of cards; the tree view is one column,
     since it is already filtered to a folder. */
  .board {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr));
    gap: 0.6rem;
  }
  .board.tree {
    grid-template-columns: 1fr;
  }
  li {
    position: relative;
    border: 1px solid #ddd;
    border-radius: 6px;
  }
  li.pinned {
    border-color: #d9c26a;
  }
  .card {
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
  .preview {
    color: #555;
    font-size: 0.85rem;
    line-height: 1.35;
    max-height: 5.4em;
    overflow: hidden;
  }
  .card small {
    color: #888;
    font-size: 0.75rem;
  }
  .pin {
    position: absolute;
    top: 0.3rem;
    right: 0.35rem;
    background: none;
    border: none;
    cursor: pointer;
    color: #bbb;
    font-size: 0.9rem;
  }
  li.pinned .pin {
    color: #c8a52e;
  }
  .empty {
    color: #666;
  }
</style>
