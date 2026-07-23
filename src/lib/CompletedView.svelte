<script>
  // Completed tasks. Unchecking one sends it back to the list it came from —
  // the core reads that from the `origin` recorded in the file.
  import { api } from "./api.js";
  import { S } from "./strings.js";

  // The list's on-disk name comes from the shell (NotebookInfo.layout); the
  // default only covers the instant before the first snapshot arrives.
  let {
    readOnly,
    onChanged,
    onError,
    reloadKey,
    completedList = "Tasks/Completed.md",
  } = $props();

  let tasks = $state([]);

  $effect(() => {
    reloadKey;
    load();
  });

  async function load() {
    try {
      // The list is named in English on disk since phase 5; the label the user
      // reads is a separate thing and stays translated.
      tasks = await api.listTasks(completedList);
    } catch (e) {
      onError(e);
    }
  }

  async function uncomplete(id) {
    try {
      await api.uncompleteTask(completedList, id);
      await load();
      onChanged();
    } catch (e) {
      onError(e);
    }
  }
</script>

<h2 class="completed-view__title">{S.completed}</h2>

{#if tasks.length === 0}
  <p class="completed-view__empty">{S.nothingCompleted}</p>
{:else}
  <ul class="completed-view__list">
    <!-- See ListView: position is part of the key so a duplicated id cannot
         take the whole screen down. -->
    {#each tasks as task, i (`${task.id ?? ""}#${i}`)}
      <li class="completed-view__item">
        <input
          class="completed-view__checkbox"
          type="checkbox"
          checked={task.done}
          disabled={readOnly || !task.id}
          onchange={() => uncomplete(task.id)}
          aria-label={S.uncheck}
        />
        <span class="completed-view__text">{task.text}</span>
        {#if task.origin}<small class="completed-view__origin"
            >{S.goesBackTo(task.origin)}</small
          >{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .completed-view__list {
    list-style: none;
    padding: 0;
  }
  .completed-view__item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.15rem 0;
  }
  .completed-view__text {
    text-decoration: line-through;
    color: #555;
  }
  .completed-view__origin {
    color: #666;
  }
  .completed-view__empty {
    color: #666;
  }
</style>
