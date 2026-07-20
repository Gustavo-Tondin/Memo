<script>
  // One task line. Knows how to render and edit itself; every action is a
  // callback, so the screen above decides what completing means.
  let { task, list, showList = false, onComplete, onEdit, children } = $props();

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

  // The file always stores ISO; display follows the user's preference, which
  // arrives already resolved from the shell. Default until settings exist.
  function formatDate(iso) {
    const [y, m, d] = iso.split("-");
    return `${d}-${m}-${y}`;
  }
</script>

<li>
  <input
    type="checkbox"
    checked={task.done}
    onchange={() => onComplete(list, task.id)}
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
    <button class="text" onclick={startEditing} title="clique para editar">
      {task.text}
    </button>
  {/if}

  <!-- In list view the fields stay quiet: a compact marker, and the detail
       only when the task is opened. Phase 9 gives this a real design. -->
  {#if task.priority}<span class="prio">!{task.priority}</span>{/if}
  {#if task.due}<span class="due">{formatDate(task.due)}</span>{/if}
  {#each task.tags ?? [] as tag}<span class="tag">#{tag}</span>{/each}
  {#if task.subtasks?.length}
    <span class="sub">{doneSubtasks}/{task.subtasks.length}</span>
  {/if}
  {#if task.repeat}<span class="repeat" title="repete">↻</span>{/if}

  {#if showList}<small class="list">{list}</small>{/if}
  {@render children?.()}
</li>

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
  .text {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: text;
    flex: 1;
  }
  .list {
    color: #666;
  }
  .edit {
    flex: 1;
    font: inherit;
  }
  .prio,
  .due,
  .tag,
  .sub,
  .repeat {
    font-size: 0.75rem;
    padding: 0.05rem 0.35rem;
    border-radius: 10px;
    background: #eee;
    color: #444;
    white-space: nowrap;
  }
  .prio {
    background: #ffe0e0;
    color: #a00;
  }
  .tag {
    background: #e3ecff;
    color: #24468a;
  }
  .description {
    padding: 0 0 0.35rem 2rem;
    font-size: 0.85rem;
    color: #666;
  }
</style>
