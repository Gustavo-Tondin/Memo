<script>
  import { listName } from "./paths.js";

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

  // The file always stores ISO; display follows the user's preference, which
  // arrives already resolved from the shell. Default until settings exist.
  function formatDate(iso) {
    const [y, m, d] = iso.split("-");
    return `${d}-${m}-${y}`;
  }
</script>

<li class:selected>
  <input
    type="checkbox"
    checked={task.done}
    onchange={() => onComplete(list, task)}
    aria-label={task.done ? "desmarcar" : "concluir"}
  />

  {#if editing}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="edit"
      bind:value={draft}
      onblur={save}
      onkeydown={onKey}
      autofocus
    />
  {:else}
    <button
      class="text"
      onclick={() => onSelect?.(list, task)}
      ondblclick={startEditing}
      title="clique para abrir, duplo clique para renomear"
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
  <li class="fields">
    {#if task.due}<span class="due">{formatDate(task.due)}</span>{/if}
    {#if task.repeat}<span class="repeat" title="repete">↻</span>{/if}
    {#if task.priority}<span class="prio">!{task.priority}</span>{/if}
    {#if task.subtasks?.length}
      <span class="sub">{doneSubtasks}/{task.subtasks.length}</span>
    {/if}
    {#each task.tags ?? [] as tag}<span class="tag">#{tag}</span>{/each}
    {#if showList}<span class="list">{listName(list)}</span>{/if}
  </li>
{/if}

{#if task.description?.length}
  <li class="description">{task.description.join(" ")}</li>
{/if}

<style>
  li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.15rem 0;
  }
  li.selected {
    background: #eef2ff;
    border-radius: 4px;
  }
  .text {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex: 1;
  }
  .edit {
    flex: 1;
    font: inherit;
  }
  /* Quiet by default: same size, same colour, no chips. A field is a note
     about the task, not a competing headline. */
  .fields {
    gap: 0.6rem;
    padding: 0 0 0.2rem 1.9rem;
    font-size: 0.78rem;
    color: #777;
  }
  .fields span {
    white-space: nowrap;
  }
  .prio {
    color: #a00;
  }
  .tag {
    color: #24468a;
  }
  .description {
    padding: 0 0 0.35rem 1.9rem;
    font-size: 0.85rem;
    color: #666;
  }
</style>
