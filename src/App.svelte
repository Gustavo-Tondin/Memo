<script>
  // Phase 4: the app is usable end to end, still without design.
  // Everything visual here is placeholder — phase 5 rebuilds it from Figma.
  import { listen } from "@tauri-apps/api/event";
  import { api, describeError } from "./lib/api.js";
  import ListView from "./lib/ListView.svelte";
  import PeriodView from "./lib/PeriodView.svelte";
  import CompletedView from "./lib/CompletedView.svelte";

  const COMPLETED = "Completas";

  let notebook = $state(null);
  let clock = $state(null);
  let view = $state({ kind: "period", period: "day" });
  let error = $state(null);
  let busy = $state(true);
  /// Bumped to tell the open screen to re-read from disk.
  let reloadKey = $state(0);

  let userLists = $derived(
    (notebook?.lists ?? []).filter((name) => name !== COMPLETED),
  );

  function fail(e) {
    error = describeError(e);
    console.error("[memo]", e);
  }

  const reload = () => (reloadKey += 1);

  async function refreshNotebook() {
    notebook = await api.currentNotebook();
    if (notebook) clock = await api.periodClock();
  }

  async function openAt(path) {
    busy = true;
    error = null;
    try {
      notebook = await api.openNotebook(path);
      clock = await api.periodClock();
      scheduleTurn();
      reload();
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  async function chooseFolder() {
    try {
      const path = await api.pickFolder();
      if (path) await openAt(path);
    } catch (e) {
      fail(e);
    }
  }

  async function createList() {
    const name = prompt("Nome da nova lista:");
    if (!name) return;
    try {
      await api.createList(name.trim());
      await refreshNotebook();
      view = { kind: "list", list: name.trim() };
    } catch (e) {
      fail(e);
    }
  }

  async function renameCurrentList() {
    if (view.kind !== "list") return;
    const to = prompt(`Novo nome para "${view.list}":`, view.list);
    if (!to || to === view.list) return;
    try {
      await api.renameList(view.list, to.trim());
      await refreshNotebook();
      view = { kind: "list", list: to.trim() };
      reload();
    } catch (e) {
      fail(e);
    }
  }

  async function deleteCurrentList() {
    if (view.kind !== "list") return;
    const list = view.list;
    if (!confirm(`Apagar "${list}"? As tarefas restantes vão para a Inbox.`))
      return;
    try {
      const rescued = await api.deleteList(list);
      await refreshNotebook();
      view = { kind: "list", list: "Inbox" };
      reload();
      if (rescued > 0) {
        error = `${rescued} tarefa(s) de "${list}" foram movidas para a Inbox.`;
      }
    } catch (e) {
      fail(e);
    }
  }

  // The rollover has to happen with the app open too, not only when the
  // notebook is reopened. The core says when; this schedules the wake-up.
  let turnTimer = null;
  async function scheduleTurn() {
    if (turnTimer) clearTimeout(turnTimer);
    if (!clock) return;

    const next = Math.min(
      new Date(clock.nextDailyTurn).getTime(),
      new Date(clock.nextWeeklyTurn).getTime(),
    );
    // Cap the wait: a long sleep or a clock jump would otherwise leave the
    // screen showing yesterday until something else refreshed it.
    const delay = Math.min(Math.max(next - Date.now(), 1000), 60 * 60 * 1000);

    turnTimer = setTimeout(async () => {
      try {
        await api.refreshPeriods();
        clock = await api.periodClock();
        reload();
      } catch (e) {
        fail(e);
      }
      scheduleTurn();
    }, delay);
  }

  // Someone else wrote to the notebook (Syncthing, Obsidian, a text editor).
  listen("notebook://changed", async (event) => {
    const kind = event.payload?.kind;
    if (kind === "list") await refreshNotebook();
    reload();
  });

  // Reopen the last notebook so the app is usable straight away.
  (async () => {
    try {
      const last = await api.lastNotebook();
      if (last) await openAt(last);
      else await refreshNotebook();
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  })();
</script>

<main>
  {#if !notebook}
    <section class="onboarding">
      <h1>Memo</h1>
      <p>
        Escolha uma pasta para ser o seu caderno. Se ela ainda não for um
        caderno, o Memo cria a estrutura dentro dela — seus arquivos continuam
        sendo `.md` comuns, legíveis em qualquer editor.
      </p>
      <button onclick={chooseFolder} disabled={busy}>
        Escolher pasta do caderno…
      </button>
      {#if error}<p class="error">{error}</p>{/if}
    </section>
  {:else}
    <div class="app">
      <nav>
        <div class="notebook" title={notebook.path}>
          <strong>{notebook.name}</strong>
          {#if notebook.readOnly}<span class="ro">somente leitura</span>{/if}
        </div>

        <button
          class:active={view.kind === "period" && view.period === "day"}
          onclick={() => (view = { kind: "period", period: "day" })}>Hoje</button
        >
        <button
          class:active={view.kind === "period" && view.period === "week"}
          onclick={() => (view = { kind: "period", period: "week" })}
          >Semana</button
        >

        <hr />
        {#each userLists as list}
          <button
            class:active={view.kind === "list" && view.list === list}
            onclick={() => (view = { kind: "list", list })}>{list}</button
          >
        {/each}

        <hr />
        <button
          class:active={view.kind === "completed"}
          onclick={() => (view = { kind: "completed" })}>Completas</button
        >

        {#if !notebook.readOnly}
          <button class="secondary" onclick={createList}>+ nova lista</button>
        {/if}
        <button class="secondary" onclick={chooseFolder}>trocar caderno…</button>
      </nav>

      <section class="content">
        {#if error}
          <p class="error">{error} <button onclick={() => (error = null)}>ok</button></p>
        {/if}

        {#if view.kind === "period"}
          <PeriodView
            period={view.period}
            {clock}
            readOnly={notebook.readOnly}
            onChanged={refreshNotebook}
            onError={fail}
            {reloadKey}
          />
        {:else if view.kind === "list"}
          <ListView
            list={view.list}
            readOnly={notebook.readOnly}
            onChanged={refreshNotebook}
            onError={fail}
            {reloadKey}
          />
          {#if !notebook.readOnly && view.list !== "Inbox"}
            <p class="list-actions">
              <button onclick={renameCurrentList}>renomear lista</button>
              <button onclick={deleteCurrentList}>apagar lista</button>
            </p>
          {/if}
        {:else}
          <CompletedView
            readOnly={notebook.readOnly}
            onChanged={refreshNotebook}
            onError={fail}
            {reloadKey}
          />
        {/if}
      </section>
    </div>
  {/if}
</main>

<style>
  /* Structural only. The real design lands in phase 5. */
  main {
    font-family: system-ui, sans-serif;
    height: 100vh;
  }
  .onboarding {
    max-width: 32rem;
    margin: 4rem auto;
    padding: 1rem;
  }
  .app {
    display: grid;
    grid-template-columns: 14rem 1fr;
    height: 100%;
  }
  nav {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.25rem;
    padding: 0.75rem;
    border-right: 1px solid #ccc;
    overflow-y: auto;
  }
  nav button {
    text-align: left;
    padding: 0.35rem 0.5rem;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    cursor: pointer;
  }
  nav button:hover {
    background: #eee;
  }
  nav button.active {
    background: #ddd;
    font-weight: 600;
  }
  nav button.secondary {
    color: #555;
    font-size: 0.85rem;
  }
  .notebook {
    padding: 0.25rem 0.5rem 0.5rem;
    font-size: 0.9rem;
  }
  .ro {
    display: block;
    color: #b00;
    font-size: 0.8rem;
  }
  .content {
    padding: 1rem;
    overflow-y: auto;
  }
  .error {
    background: #fee;
    border: 1px solid #f99;
    padding: 0.5rem;
    border-radius: 4px;
  }
  .list-actions {
    margin-top: 2rem;
    display: flex;
    gap: 0.5rem;
  }
  hr {
    border: none;
    border-top: 1px solid #ddd;
    width: 100%;
    margin: 0.25rem 0;
  }
</style>
