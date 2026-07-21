<script>
  // Completed tasks. Unchecking one sends it back to the list it came from —
  // the core reads that from the `origin` recorded in the file.
  import { api } from "./api.js";
  import { COMPLETED_LIST } from "./names.js";

  let { readOnly, onChanged, onError, reloadKey } = $props();

  let tasks = $state([]);

  $effect(() => {
    reloadKey;
    load();
  });

  async function load() {
    try {
      // The list is named in English on disk since phase 5; the label the user
      // reads is a separate thing and stays translated.
      tasks = await api.listTasks(COMPLETED_LIST);
    } catch (e) {
      onError(e);
    }
  }

  async function uncomplete(id) {
    try {
      await api.uncompleteTask(id);
      await load();
      onChanged();
    } catch (e) {
      onError(e);
    }
  }
</script>

<h2>Completas</h2>

{#if tasks.length === 0}
  <p class="empty">Nada concluído ainda.</p>
{:else}
  <ul>
    <!-- See ListView: position is part of the key so a duplicated id cannot
         take the whole screen down. -->
    {#each tasks as task, i (`${task.id ?? ""}#${i}`)}
      <li>
        <input
          type="checkbox"
          checked={task.done}
          disabled={readOnly || !task.id}
          onchange={() => uncomplete(task.id)}
          aria-label="desmarcar"
        />
        <span class="done">{task.text}</span>
        {#if task.origin}<small>volta para {task.origin}</small>{/if}
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
  .done {
    text-decoration: line-through;
    color: #555;
  }
  small {
    color: #666;
  }
  .empty {
    color: #666;
  }
</style>
