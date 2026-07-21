<script>
  // One note, open for editing.
  //
  // Auto-saves like the task inspector, and for the same reason the user gave
  // when the Save button was removed: an edit you have to remember to confirm
  // is an edit you lose. The mechanics are the same too — a captured target,
  // a flush on close, and a baseline that only advances after the write
  // lands. See TaskInspector for why each of those exists.
  import { onDestroy } from "svelte";
  import { api } from "./api.js";
  import { S } from "./strings.js";
  import Editor from "./Editor.svelte";

  let {
    folder,
    path,
    readOnly = false,
    onSaved,
    onError,
    onClose,
    onRenamed,
    saveDelay = 500,
  } = $props();

  let title = $state("");
  let body = $state("");
  let pinned = $state(false);
  let loading = $state(true);

  // Plain, not reactive: none of this should re-render anything.
  let slot = null;
  let baseline = "";
  let pending = null;
  let timer = null;

  $effect(() => {
    folder;
    path;
    // Whatever was typed into the previous note goes out first, addressed to
    // that note, before this one replaces it.
    flush();
    load(folder, path);
  });

  onDestroy(() => flush());

  async function load(atFolder, atPath) {
    loading = true;
    try {
      const note = await api.readNote(atFolder, atPath);
      slot = { folder: atFolder, path: atPath };
      title = note.title;
      body = note.body;
      pinned = note.pinned;
      baseline = note.body;
    } catch (e) {
      onError?.(e);
    } finally {
      loading = false;
    }
  }

  // The auto-save. Comparing against the baseline is what tells a real edit
  // apart from `load` having just filled the field.
  $effect(() => {
    const snapshot = body;
    if (readOnly || loading || snapshot === baseline) return;

    pending = { target: slot, body: snapshot };
    if (timer) clearTimeout(timer);
    timer = setTimeout(write, saveDelay);
  });

  function flush() {
    if (!pending) return Promise.resolve();
    if (timer) clearTimeout(timer);
    timer = null;
    return write();
  }

  async function write() {
    const job = pending;
    pending = null;
    timer = null;
    if (!job?.target) return;

    try {
      await api.writeNote(job.target.folder, job.target.path, job.body);
      // Only after it lands, and only if we are still on the same note: a
      // failed write must be retried by the next edit, not counted as saved.
      if (job.target === slot) baseline = job.body;
      onSaved?.();
    } catch (e) {
      onError?.(e);
    }
  }

  async function act(fn) {
    try {
      await flush();
      await fn();
      onSaved?.();
    } catch (e) {
      onError?.(e);
    }
  }

  const togglePin = () =>
    act(async () => {
      await api.setNotePinned(folder, path, !pinned);
      pinned = !pinned;
    });

  const rename = () =>
    act(async () => {
      const next = prompt(S.promptRenameNote(title), title);
      if (!next || next.trim() === title) return;
      const moved = await api.renameNote(folder, path, next.trim());
      onRenamed?.(moved);
    });

  const remove = () =>
    act(async () => {
      if (!confirm(S.confirmDeleteNote(title))) return;
      await api.deleteNote(folder, path);
      onClose?.();
    });
</script>

<header>
  <button class="back" onclick={() => onClose?.()}>{S.backToNotes}</button>
  <h2>{title}</h2>
  {#if !readOnly}
    <button onclick={togglePin}>{pinned ? S.unpin : S.pin}</button>
    <button onclick={rename}>{S.renameNote}</button>
    <button onclick={remove}>{S.deleteNote}</button>
  {/if}
</header>

<div class="body" aria-label={S.noteBodyPlaceholder}>
  <Editor
    value={body}
    readOnly={readOnly || loading}
    placeholder={S.noteBodyPlaceholder}
    onChange={(next) => (body = next)}
  />
</div>

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  h2 {
    flex: 1;
    margin: 0;
  }
  .back {
    background: none;
    border: none;
    cursor: pointer;
    font: inherit;
    color: #555;
    padding: 0;
  }
  .body {
    min-height: 60vh;
  }
</style>
