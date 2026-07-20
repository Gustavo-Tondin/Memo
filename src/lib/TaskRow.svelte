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

  {#if showList}<small class="list">{list}</small>{/if}
  {@render children?.()}
</li>

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
</style>
