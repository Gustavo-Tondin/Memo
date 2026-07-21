<script>
  // Phase 4: the app is usable end to end, still without design.
  // Everything visual here is placeholder — phase 5 rebuilds it from Figma.
  import { listen } from "@tauri-apps/api/event";
  import { api, describeError } from "./lib/api.js";
  import ListView from "./lib/ListView.svelte";
  import PeriodView from "./lib/PeriodView.svelte";
  import CompletedView from "./lib/CompletedView.svelte";
  import TaskInspector from "./lib/TaskInspector.svelte";
  import { COMPLETED_LIST, INBOX_LIST } from "./lib/names.js";

  let notebook = $state(null);
  let clock = $state(null);
  let view = $state({ kind: "period", period: "day" });
  let error = $state(null);
  let busy = $state(true);
  /// Bumped to tell the open screen to re-read from disk.
  let reloadKey = $state(0);
  let counts = $state({});
  let conflicts = $state([]);
  /// The task open in the right-hand panel, as `{ list, task }`.
  let selected = $state(null);

  const select = (list, task) => (selected = { list, task });

  /// A screen as a string, so the shell can store it without knowing what a
  /// screen is. Same strings go back through `restoreView`.
  function viewToId(v) {
    if (v.kind === "period") return v.period;
    if (v.kind === "list") return `list:${v.list}`;
    return v.kind;
  }

  function restoreView(id) {
    if (!id) return null;
    if (id === "day" || id === "week") return { kind: "period", period: id };
    if (id === "completed") return { kind: "completed" };
    if (id.startsWith("list:")) return { kind: "list", list: id.slice(5) };
    return null;
  }

  // Records where the user is. The shell ignores this when the notebook has
  // the preference off, so no check is needed here.
  //
  // Changing screen also closes the inspector: the panel would otherwise keep
  // showing a task from a list that is no longer on screen.
  $effect(() => {
    const id = viewToId(view);
    selected = null;
    if (notebook) api.rememberScreen(id).catch(() => {});
  });

  let userLists = $derived(
    (notebook?.lists ?? []).filter((name) => name !== COMPLETED_LIST),
  );

  function fail(e) {
    error = describeError(e);
    console.error("[memo]", e);
  }

  const reload = () => (reloadKey += 1);

  async function refreshNotebook() {
    notebook = await api.currentNotebook();
    if (!notebook) return;
    clock = await api.periodClock();
    // Counts come back empty when the user turned them off.
    [counts, conflicts] = await Promise.all([
      api.listCounts(),
      api.listConflicts(),
    ]);
  }

  async function openAt(path) {
    busy = true;
    error = null;
    try {
      notebook = await api.openNotebook(path);
      clock = await api.periodClock();
      await refreshNotebook();

      // Only after the notebook is open do we know whether it wants the last
      // screen back.
      const restored = restoreView(await api.screenToRestore());
      if (restored) view = restored;

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
      view = { kind: "list", list: INBOX_LIST };
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
    <div class="app" class:with-panel={selected}>
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
            onclick={() => (view = { kind: "list", list })}
          >
            {list}
            {#if counts[list]}<span class="count">{counts[list]}</span>{/if}
          </button>
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

        {#if conflicts.length > 0}
          <!-- The one case where the user can silently lose work: two devices
               edited the same list and the sync tool kept both. -->
          <div class="conflict">
            <strong
              >{conflicts.length} conflito(s) de sincronização neste caderno</strong
            >
            <p>
              Outro dispositivo editou os mesmos arquivos. O Memo não escolhe
              por você — abra a pasta e decida qual versão fica.
            </p>
            <ul>
              {#each conflicts as conflict}
                <li>
                  {#if conflict.list}<strong>{conflict.list}</strong>{/if}
                  <code>{conflict.path}</code>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if view.kind === "period"}
          <PeriodView
            period={view.period}
            {clock}
            readOnly={notebook.readOnly}
            onChanged={refreshNotebook}
            onError={fail}
            {reloadKey}
            onSelect={select}
            selectedId={selected?.task?.id ?? null}
          />
        {:else if view.kind === "list"}
          <ListView
            list={view.list}
            readOnly={notebook.readOnly}
            onChanged={refreshNotebook}
            onError={fail}
            {reloadKey}
            onSelect={select}
            selectedId={selected?.task?.id ?? null}
          />
          {#if !notebook.readOnly && view.list !== INBOX_LIST}
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

      <!-- The third pane only exists while a task is open. Phase 8.5 turns
           this into the permanent inspector of the wireframe. -->
      {#if selected}
        <TaskInspector
          task={selected.task}
          list={selected.list}
          readOnly={notebook.readOnly}
          onSaved={() => {
            refreshNotebook();
            reload();
          }}
          onError={fail}
          onClose={() => (selected = null)}
        />
      {/if}
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
    min-height: 0;
  }
  /* Driven by a class, not `:has(aside)`: the panel is a child component and
     Svelte's scoped CSS would never match its element from here. */
  .app.with-panel {
    grid-template-columns: 14rem 1fr 20rem;
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
  .conflict {
    background: #fff8e1;
    border: 1px solid #e6c34a;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }
  .conflict ul {
    margin: 0.5rem 0 0;
    padding-left: 1.2rem;
  }
  .conflict code {
    word-break: break-all;
    font-size: 0.8rem;
  }
  .count {
    float: right;
    color: #666;
    font-size: 0.85rem;
    font-weight: normal;
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
