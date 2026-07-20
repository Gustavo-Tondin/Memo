<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // Phase 3 bench: proves every bridge command answers what it should.
  // There is no design here on purpose — the roadmap only starts styling at
  // phase 5, and phase 4 replaces this file with the real screens.

  let notebook = $state(null);
  let log = $state([]);
  let running = $state(false);

  function note(label, value) {
    const line = {
      label,
      value: JSON.stringify(value),
      at: new Date().toLocaleTimeString(),
    };
    console.log(`[memo] ${label}`, value);
    log = [line, ...log].slice(0, 200);
  }

  function fail(label, error) {
    console.error(`[memo] ${label} failed`, error);
    log = [
      {
        label: `${label} — ERRO`,
        value: JSON.stringify(error),
        at: new Date().toLocaleTimeString(),
      },
      ...log,
    ];
  }

  async function call(label, command, args) {
    const value = await invoke(command, args);
    note(label, value);
    return value;
  }

  // External changes (Syncthing, Obsidian) arrive as events, not polling.
  listen("notebook://changed", (event) =>
    note("evento notebook://changed", event.payload),
  );

  async function chooseNotebook() {
    try {
      const path = await invoke("pick_notebook_folder");
      if (!path) return note("seletor de pasta", "cancelado");
      notebook = await call("open_notebook", "open_notebook", { path });
    } catch (e) {
      fail("open_notebook", e);
    }
  }

  // Walks every command in order, against the open notebook.
  async function runBench() {
    if (!notebook) return;
    running = true;
    log = [];
    try {
      await call("core_version", "core_version");
      await call("current_notebook", "current_notebook");
      await call("period_clock", "period_clock");
      await call("notebook_settings", "notebook_settings");

      const listName = `Bench ${Date.now() % 100000}`;
      await call("create_list", "create_list", { name: listName });
      await call("list_names", "list_names");

      const id = await call("create_task", "create_task", {
        list: listName,
        text: "tarefa de teste",
      });
      await call("edit_task_text", "edit_task_text", {
        list: listName,
        id,
        text: "tarefa editada",
      });
      await call("list_tasks", "list_tasks", { list: listName });

      await call("pull_into_period (week)", "pull_into_period", {
        period: "week",
        list: listName,
        id,
      });
      await call("pull_into_period (day)", "pull_into_period", {
        period: "day",
        list: listName,
        id,
      });
      await call("period_state (day)", "period_state", { period: "day" });

      const inboxId = await call("add_task_in_period", "add_task_in_period", {
        period: "day",
        text: "criada no dia",
      });
      await call("remove_from_period", "remove_from_period", {
        period: "day",
        list: "Inbox",
        id: inboxId,
      });

      await call("complete_task", "complete_task", { list: listName, id });
      await call("period_state após completar", "period_state", {
        period: "day",
      });
      await call("list_tasks (Completas)", "list_tasks", { list: "Completas" });
      await call("uncomplete_task", "uncomplete_task", { id });
      await call("list_tasks após desfazer", "list_tasks", { list: listName });

      const renamed = `${listName} renomeada`;
      await call("rename_list", "rename_list", { from: listName, to: renamed });
      await call("delete_list", "delete_list", { name: renamed });

      await call("refresh_periods", "refresh_periods");

      // Errors must arrive typed, not as an opaque string.
      try {
        await invoke("complete_task", { list: "Inbox", id: "nao-existe" });
      } catch (e) {
        note("erro esperado (taskNotFound)", e);
      }
      try {
        await invoke("delete_list", { name: "Inbox" });
      } catch (e) {
        note("erro esperado (protectedList)", e);
      }

      note("SUÍTE COMPLETA", "todos os comandos responderam");
    } catch (e) {
      fail("suíte", e);
    } finally {
      running = false;
      notebook = await invoke("current_notebook");
    }
  }

  invoke("current_notebook").then((nb) => {
    notebook = nb;
    note("current_notebook (inicial)", nb);
  });
</script>

<main>
  <h1>Memo — banco de testes da ponte (Fase 3)</h1>

  <div class="row">
    <button onclick={chooseNotebook}>Escolher pasta do caderno…</button>
    <button onclick={runBench} disabled={!notebook || running}>
      {running ? "rodando…" : "Rodar suíte de comandos"}
    </button>
  </div>

  {#if notebook}
    <p class="notebook">
      Caderno: <strong>{notebook.name}</strong> — {notebook.path}
      {#if notebook.readOnly}<span class="ro">somente leitura</span>{/if}
      <br />Listas: {notebook.lists.join(", ")}
    </p>
  {:else}
    <p class="notebook">Nenhum caderno aberto. Escolha uma pasta para começar.</p>
  {/if}

  <ol class="log">
    {#each log as line}
      <li>
        <span class="time">{line.at}</span>
        <span class="label">{line.label}</span>
        <code>{line.value}</code>
      </li>
    {/each}
  </ol>
</main>

<style>
  /* Deliberately plain: this screen exists to read output, not to look good. */
  main {
    font-family: system-ui, sans-serif;
    padding: 1rem;
    max-width: 60rem;
    margin: 0 auto;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .notebook {
    font-size: 0.9rem;
  }
  .ro {
    color: #b00;
    font-weight: bold;
  }
  .log {
    font-size: 0.8rem;
    line-height: 1.5;
    padding-left: 1.5rem;
  }
  .log li {
    margin-bottom: 0.25rem;
  }
  .time {
    color: #888;
    margin-right: 0.5rem;
  }
  .label {
    font-weight: 600;
    margin-right: 0.5rem;
  }
  code {
    word-break: break-all;
  }
</style>
