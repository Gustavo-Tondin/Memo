<script>
  import { listName } from "./paths.js";
  import { formatDate } from "./dates.js";
  import { S } from "./strings.js";

  // One task line. Knows how to render and edit itself; every action is a
  // callback, so the screen above decides what completing means.
  //
  // Two gestures on the text, on purpose: a single click opens the task in the
  // inspector (where the fields live), a double click renames it in place. The
  // rename is the one edit frequent enough to deserve staying on the row.
  let {
    task,
    list,
    showList = false,
    selected = false,
    onComplete,
    onEdit,
    onSelect,
    dateFormat = "dd-mm-yyyy",
    children,
  } = $props();

  let editing = $state(false);
  // Filled when editing starts; initialising from `task` here would only ever
  // capture the value the row was created with.
  let draft = $state("");

  function startEditing() {
    draft = task.text;
    editing = true;
  }

  async function save() {
    editing = false;
    const text = draft.trim();
    if (text && text !== task.text) await onEdit(list, task.id, text);
  }

  function onKey(event) {
    if (event.key === "Enter") save();
    if (event.key === "Escape") editing = false;
  }

  let doneSubtasks = $derived(
    (task.subtasks ?? []).filter((s) => s.done).length,
  );

  let hasFields = $derived(
    !!(
      task.due ||
      task.repeat ||
      task.priority ||
      task.subtasks?.length ||
      task.tags?.length ||
      showList
    ),
  );
</script>

<li class="task-row" class:task-row--selected={selected}>
  <input
    class="task-row__checkbox"
    type="checkbox"
    checked={task.done}
    onchange={() => onComplete(list, task)}
    aria-label={task.done ? S.uncheck : S.complete}
  />

  {#if editing}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="task-row__edit"
      bind:value={draft}
      onblur={save}
      onkeydown={onKey}
      autofocus
    />
  {:else}
    <button
      class="task-row__text"
      onclick={() => onSelect?.(list, task)}
      ondblclick={startEditing}
      title={S.taskRowHint}
    >
      {task.text}
    </button>
  {/if}

  {@render children?.()}
</li>

<!-- The fields belong under the task, not beside it. Sharing the line with
     the name made them compete with it for attention, when the name is the
     only thing being read most of the time. Phase 10 gives this a real look;
     the order here is already the one it should keep. -->
{#if hasFields}
  <li class="task-row__fields">
    {#if task.due}<span class="task-row__field task-row__field--due"
        >{formatDate(task.due, dateFormat)}</span
      >{/if}
    {#if task.repeat}<span
        class="task-row__field task-row__field--repeat"
        title={S.repeatsHint}>↻</span
      >{/if}
    {#if task.priority}<span class="task-row__field task-row__field--priority"
        >!{task.priority}</span
      >{/if}
    {#if task.subtasks?.length}
      <span class="task-row__field task-row__field--subtasks"
        >{doneSubtasks}/{task.subtasks.length}</span
      >
    {/if}
    {#each task.tags ?? [] as tag}<span
        class="task-row__field task-row__field--tag">#{tag}</span
      >{/each}
    {#if showList}<span class="task-row__field task-row__field--list"
        >{listName(list)}</span
      >{/if}
  </li>
{/if}

{#if task.description?.length}
  <li class="task-row__description">{task.description.join(" ")}</li>
{/if}

<style>
  .task-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.15rem 0;
  }
  .task-row--selected {
    background: #eef2ff;
    border-radius: 4px;
  }
  .task-row__text {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex: 1;
  }
  .task-row__edit {
    flex: 1;
    font: inherit;
  }
  /* Quiet by default: same size, same colour, no chips. A field is a note
     about the task, not a competing headline. */
  .task-row__fields {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0 0 0.2rem 1.9rem;
    font-size: 0.78rem;
    color: #777;
  }
  .task-row__field {
    white-space: nowrap;
  }
  .task-row__field--priority {
    color: #a00;
  }
  .task-row__field--tag {
    color: #24468a;
  }
  .task-row__description {
    display: flex;
    padding: 0 0 0.35rem 1.9rem;
    font-size: 0.85rem;
    color: #666;
  }
</style>
