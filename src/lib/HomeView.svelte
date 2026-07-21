<script>
  // Home: the day. Today's tasks on top, today's notes below, with a quick
  // capture box between them — the wireframe's opening screen.
  //
  // It owns nothing. The tasks are the day state the Tasks side already
  // keeps, and the notes are a view of the notes inbox filtered by `created`
  // (spec 5) — so nothing is moved when the day turns.
  import { api } from "./api.js";
  import { S } from "./strings.js";
  import { ensureTaskId } from "./taskId.js";
  import TaskRow from "./TaskRow.svelte";

  let {
    notesFolder,
    notesInbox = "Inbox",
    folders = [],
    readOnly = false,
    onChanged,
    onError,
    onOpenNote,
    onSelectTask,
    selectedId = null,
    reloadKey = 0,
  } = $props();

  let tasks = $state([]);
  let notes = $state([]);
  let capture = $state("");
  /// Null until the user picks: the destination defaults to the inbox but
  /// must follow the prop if it arrives later, so a state seeded from it
  /// would freeze on whatever the prop was at first render.
  let chosenFolder = $state(null);
  let captureTo = $derived(chosenFolder ?? notesInbox);

  $effect(() => {
    reloadKey;
    notesFolder;
    load();
  });

  async function load() {
    try {
      const [pulled, today] = await Promise.all([
        api.periodTasks("day"),
        notesFolder ? api.notesCreatedToday(notesFolder) : Promise.resolve([]),
      ]);
      tasks = pulled;
      notes = today;
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

  const complete = (path, task) =>
    act(async () => {
      const id = await ensureTaskId(path, task);
      await api.completeTask(path, id);
    });

  const edit = (path, id, text) => act(() => api.editTaskText(path, id, text));

  const save = () =>
    act(async () => {
      const text = capture.trim();
      if (!text) return;
      capture = "";
      await api.quickCaptureNote(notesFolder, captureTo, text);
    });
</script>

<section class="card">
  <h2>{S.todaysTasks}</h2>
  {#if tasks.length === 0}
    <p class="empty">{S.noTasksToday}</p>
  {:else}
    <ul>
      {#each tasks as entry, i (`${entry.path}/${entry.task.id ?? ""}#${i}`)}
        <TaskRow
          task={entry.task}
          list={entry.path}
          showList
          onSelect={onSelectTask}
          selected={!!entry.task.id && entry.task.id === selectedId}
          onComplete={complete}
          onEdit={edit}
        />
      {/each}
    </ul>
  {/if}
</section>

<section class="card">
  <h2>{S.todaysNotes}</h2>

  {#if !readOnly && notesFolder}
    <form class="capture" onsubmit={(e) => (e.preventDefault(), save())}>
      <textarea
        rows="2"
        placeholder={S.quickNote}
        aria-label={S.quickNote}
        bind:value={capture}
        onkeydown={(e) => {
          // Enter saves, Shift+Enter is a new line: a quick note is usually
          // one line, and reaching for a button breaks the flow.
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            save();
          }
        }}
      ></textarea>
      <label>
        {S.quickNoteTo}
        <select value={captureTo} onchange={(e) => (chosenFolder = e.currentTarget.value)} aria-label={S.quickNoteTo}>
          <option value={notesInbox}>{notesInbox}</option>
          {#each folders.filter((f) => f !== notesInbox) as name (name)}
            <option value={name}>{name}</option>
          {/each}
        </select>
      </label>
    </form>
  {/if}

  {#if notes.length === 0}
    <p class="empty">{S.noNotesToday}</p>
  {:else}
    <ul class="notes">
      {#each notes as note (note.path)}
        <li>
          <button onclick={() => onOpenNote?.(note.path)}>
            <strong>{note.title}</strong>
            <span>{note.preview || S.emptyNote}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .card {
    border: 1px solid #ddd;
    border-radius: 8px;
    padding: 0.75rem 1rem 1rem;
    margin-bottom: 1rem;
  }
  h2 {
    margin: 0 0 0.6rem;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #666;
    text-align: center;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .capture {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
    margin-bottom: 0.75rem;
  }
  .capture textarea {
    flex: 1;
    font: inherit;
    resize: vertical;
  }
  .capture label {
    font-size: 0.8rem;
    color: #666;
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  /* The note cards mirror the notes board, so the same thing looks the same
     in both places. */
  .notes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.6rem;
  }
  .notes button {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    width: 100%;
    text-align: left;
    background: none;
    border: 1px solid #ddd;
    border-radius: 6px;
    font: inherit;
    cursor: pointer;
    padding: 0.6rem 0.7rem;
  }
  .notes span {
    color: #555;
    font-size: 0.85rem;
    max-height: 4em;
    overflow: hidden;
  }
  .empty {
    color: #666;
    margin: 0;
  }
</style>
