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
    dateFormat = "dd-mm-yyyy",
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

<h2 class="list-view__title">{listName(list)}</h2>

{#if tasks.length === 0}
  <p class="list-view__empty">{S.emptyList}</p>
{:else}
  <ul class="list-view__list">
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
        {dateFormat}
      >
        <button class="list-view__action" onclick={() => pull("day", task)}
          >{S.pullToToday}</button
        >
        <button class="list-view__action" onclick={() => pull("week", task)}
          >{S.pullToWeek}</button
        >
      </TaskRow>
    {/each}
  </ul>
{/if}

<!-- Below the list, not above it: adding is what you do after reading what is
     already there, and a field on top pushes the list down every render. -->
{#if !readOnly}
  <form class="list-view__form" onsubmit={(e) => (e.preventDefault(), add())}>
    <input
      class="list-view__input"
      placeholder={S.newTaskPlaceholder}
      bind:value={newText}
    />
    <button class="list-view__submit" type="submit">{S.addTask}</button>
  </form>
{/if}

<style>
  .list-view__list {
    list-style: none;
    padding: 0;
  }
  .list-view__empty {
    color: #666;
  }
  .list-view__form {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  .list-view__input {
    flex: 1;
  }
</style>
