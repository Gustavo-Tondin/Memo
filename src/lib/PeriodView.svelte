<script>
  // Today and This Week. Both are the same screen: what is pulled, plus what
  // could be pulled. Neither stores content — a task created here is written
  // to the Inbox by the core.
  import { api } from "./api.js";
  import { ensureTaskId } from "./taskId.js";
  import { listName } from "./paths.js";
  import { S } from "./strings.js";
  import TaskRow from "./TaskRow.svelte";

  let {
    period,
    clock,
    readOnly,
    onChanged,
    onError,
    reloadKey,
    onSelect,
    selectedId = null,
    dateFormat = "dd-mm-yyyy",
  } = $props();

  let pulled = $state([]);
  let suggestions = $state([]);
  let newText = $state("");

  $effect(() => {
    period;
    reloadKey;
    load();
  });

  async function load() {
    try {
      [pulled, suggestions] = await Promise.all([
        api.periodTasks(period),
        api.groupedSuggestions(period),
      ]);
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
    act(() => api.addTaskInPeriod(period, text));
  };

  const complete = (list, task) =>
    act(async () => {
      const id = await ensureTaskId(list, task);
      await api.completeTask(list, id);
    });

  const edit = (list, id, text) => act(() => api.editTaskText(list, id, text));
  const remove = (list, id) => act(() => api.removeFrom(period, list, id));

  // A suggestion may have no id yet — ids are handed out only when a task
  // needs to be addressed, and pulling it into a period is exactly that.
  const pull = (list, task) =>
    act(async () => {
      const id = await ensureTaskId(list, task);
      await api.pullInto(period, list, id);
    });

  const GROUPS = [
    { key: "urgent", label: S.groupUrgent },
    { key: "soon", label: S.groupSoon },
    { key: "thisWeek", label: S.groupThisWeek },
    { key: "lists", label: S.groupLists },
  ];

  let groups = $derived(
    GROUPS.map((g) => ({
      ...g,
      items: suggestions.filter((s) => s.group === g.key),
    })),
  );

  let title = $derived(period === "day" ? S.today : S.weekTitle);
  let subtitle = $derived(
    period === "day" ? clock?.today : S.weekOf(clock?.weekStart ?? ""),
  );
</script>

<h2>{title} <small>{subtitle}</small></h2>

{#if !readOnly}
  <form onsubmit={(e) => (e.preventDefault(), add())}>
    <input placeholder={S.newTaskToInboxPlaceholder} bind:value={newText} />
    <button type="submit">{S.addTask}</button>
  </form>
{/if}

{#if pulled.length === 0}
  <p class="empty">{S.nothingPulled}</p>
{:else}
  <ul>
    {#each pulled as entry, i (`${entry.path}/${entry.task.id ?? ""}#${i}`)}
      <TaskRow
        task={entry.task}
        list={entry.path}
        showList
        {onSelect}
        selected={!!entry.task.id && entry.task.id === selectedId}
        onComplete={complete}
        onEdit={edit}
        {dateFormat}
      >
        <button onclick={() => remove(entry.path, entry.task.id)}>{S.removeFromPeriod}</button>
      </TaskRow>
    {/each}
  </ul>
{/if}

<h3>{S.suggestionsTitle}</h3>
{#if suggestions.length === 0}
  <p class="empty">{S.noSuggestions}</p>
{:else}
  {#each groups as group}
    {#if group.items.length > 0}
      <details open={group.key !== "lists"}>
        <summary>{group.label} ({group.items.length})</summary>
        <ul>
          {#each group.items as entry, i (`${entry.path}/${entry.task.id ?? ""}#${i}`)}
            <li>
              <button
                class="text"
                onclick={() => onSelect?.(entry.path, entry.task)}
                title="clique para abrir">{entry.task.text}</button
              >
              {#if entry.task.due}<small class="due">{entry.task.due}</small>{/if}
              <small class="from">{listName(entry.path)}</small>
              <button onclick={() => pull(entry.path, entry.task)}>{S.pull}</button>
            </li>
          {/each}
        </ul>
      </details>
    {/if}
  {/each}
{/if}

<style>
  ul {
    list-style: none;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.15rem 0;
  }
  small {
    color: #666;
  }
  .text {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
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
