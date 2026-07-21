<script>
  // A single task list (Inbox, Compras, …).
  import { api } from "./api.js";
  import { ensureTaskId } from "./taskId.js";
  import TaskRow from "./TaskRow.svelte";

  let {
    list,
    readOnly,
    onChanged,
    onError,
    reloadKey,
    onSelect,
    selectedId = null,
  } = $props();

  let tasks = $state([]);
  let newText = $state("");

  // Reloads whenever the list changes or something external touched the disk.
  $effect(() => {
    list;
    reloadKey;
    load();
  });

  async function load() {
    try {
      tasks = await api.listTasks(list);
    } catch (e) {
      onError(e);
    }
  }

  async function act(fn) {
    try {
      await fn();
      await load();
      onChanged();
    } catch (e) {
      onError(e);
    }
  }

  const add = () => {
    const text = newText.trim();
    if (!text) return;
    newText = "";
    act(() => api.createTask(list, text));
  };

  const complete = (l, id) => act(() => api.completeTask(l, id));
  const edit = (l, id, text) => act(() => api.editTaskText(l, id, text));

  // A task written by hand in another editor has no id yet, and pulling it
  // into a period is exactly the kind of thing that earns one.
  const pull = (period, task) =>
    act(async () => {
      const id = await ensureTaskId(list, task);
      await api.pullInto(period, list, id);
    });
</script>

<h2>{list}</h2>

{#if !readOnly}
  <form onsubmit={(e) => (e.preventDefault(), add())}>
    <input placeholder="Nova tarefa…" bind:value={newText} />
    <button type="submit">Adicionar</button>
  </form>
{/if}

{#if tasks.length === 0}
  <p class="empty">Nenhuma tarefa nesta lista.</p>
{:else}
  <ul>
    <!-- Keyed by position as well as id: a duplicated id would otherwise be a
         duplicate key, and Svelte aborts rendering the whole list. A read-only
         notebook never gets its ids de-duplicated, so this can still happen. -->
    {#each tasks as task, i (`${task.id ?? ""}#${i}`)}
      <TaskRow
        {task}
        {list}
        {onSelect}
        selected={!!task.id && task.id === selectedId}
        onComplete={complete}
        onEdit={edit}
      >
        <button onclick={() => pull("day", task)}>→ Hoje</button>
        <button onclick={() => pull("week", task)}>→ Semana</button>
      </TaskRow>
    {/each}
  </ul>
{/if}

<style>
  ul {
    list-style: none;
    padding: 0;
  }
  .empty {
    color: #666;
  }
  form {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  form input {
    flex: 1;
  }
</style>
