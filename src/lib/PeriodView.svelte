<script>
  // Today and This Week. Both are the same screen: what is pulled, plus what
  // could be pulled. Neither stores content — a task created here is written
  // to the Inbox by the core.
  import { api } from "./api.js";
  import TaskRow from "./TaskRow.svelte";

  let { period, clock, readOnly, onChanged, onError, reloadKey } = $props();

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
        api.periodSuggestions(period),
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

  const complete = (list, id) => act(() => api.completeTask(list, id));
  const edit = (list, id, text) => act(() => api.editTaskText(list, id, text));
  const pull = (list, id) => act(() => api.pullInto(period, list, id));
  const remove = (list, id) => act(() => api.removeFrom(period, list, id));

  let title = $derived(period === "day" ? "Hoje" : "Meta da semana");
  let subtitle = $derived(
    period === "day" ? clock?.today : `semana de ${clock?.weekStart ?? ""}`,
  );
</script>

<h2>{title} <small>{subtitle}</small></h2>

{#if !readOnly}
  <form onsubmit={(e) => (e.preventDefault(), add())}>
    <input placeholder="Nova tarefa (vai para a Inbox)…" bind:value={newText} />
    <button type="submit">Adicionar</button>
  </form>
{/if}

{#if pulled.length === 0}
  <p class="empty">Nada escolhido ainda. Puxe algo das sugestões abaixo.</p>
{:else}
  <ul>
    {#each pulled as entry, i (`${entry.list}/${entry.task.id ?? ""}#${i}`)}
      <TaskRow
        task={entry.task}
        list={entry.list}
        showList
        onComplete={complete}
        onEdit={edit}
      >
        <button onclick={() => remove(entry.list, entry.task.id)}>tirar</button>
      </TaskRow>
    {/each}
  </ul>
{/if}

<h3>Sugestões</h3>
{#if suggestions.length === 0}
  <p class="empty">Nenhuma tarefa disponível.</p>
{:else}
  <ul>
    {#each suggestions as entry, i (`${entry.list}/${entry.task.id ?? ""}#${i}`)}
      <li>
        <span>{entry.task.text}</span>
        <small>{entry.list}</small>
        <button onclick={() => pull(entry.list, entry.task.id)}>puxar</button>
      </li>
    {/each}
  </ul>
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
