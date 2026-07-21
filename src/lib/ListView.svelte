<script>
  // A single task list (Inbox, Compras, …).
  import { api } from "./api.js";
  import { ensureTaskId } from "./taskId.js";
  import { listName } from "./paths.js";
  import { S } from "./strings.js";
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

  // A task can have no id yet — written by hand in another editor, or just
  // respawned by a repetition — and both of these are what earns one.
  const complete = (l, task) =>
    act(async () => {
      const id = await ensureTaskId(l, task);
      await api.completeTask(l, id);
    });

  const pull = (period, task) =>
    act(async () => {
      const id = await ensureTaskId(list, task);
      await api.pullInto(period, list, id);
    });

  const edit = (l, id, text) => act(() => api.editTaskText(l, id, text));
</script>

<h2>{listName(list)}</h2>

{#if tasks.length === 0}
  <p class="empty">{S.emptyList}</p>
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
        <button onclick={() => pull("day", task)}>{S.pullToToday}</button>
        <button onclick={() => pull("week", task)}>{S.pullToWeek}</button>
      </TaskRow>
    {/each}
  </ul>
{/if}

<!-- Below the list, not above it: adding is what you do after reading what is
     already there, and a field on top pushes the list down every render. -->
{#if !readOnly}
  <form onsubmit={(e) => (e.preventDefault(), add())}>
    <input placeholder={S.newTaskPlaceholder} bind:value={newText} />
    <button type="submit">{S.addTask}</button>
  </form>
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
    margin-top: 1rem;
  }
  form input {
    flex: 1;
  }
</style>
