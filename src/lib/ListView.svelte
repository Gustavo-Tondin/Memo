<script>
  // A single task list (Inbox, Compras, …).
  import { api } from "./api.js";
  import TaskRow from "./TaskRow.svelte";

  let { list, readOnly, onChanged, onError, reloadKey } = $props();

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
  const pull = (period, id) => act(() => api.pullInto(period, list, id));
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
    {#each tasks as task (task.id ?? task.text)}
      <TaskRow {task} {list} onComplete={complete} onEdit={edit}>
        <button onclick={() => pull("day", task.id)}>→ Hoje</button>
        <button onclick={() => pull("week", task.id)}>→ Semana</button>
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
