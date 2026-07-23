<script>
  // The shell: three panels, document tabs, and the router that decides which
  // screen a tab is showing.
  //
  // Phase 8.5 put the pieces where the wireframe puts them. What this file
  // owns is arrangement and navigation; every screen below it owns its own
  // data, and every rule about tabs lives in `tabs.js`. Design comes in
  // phase 10, on top of a token layer that does not exist yet — so the CSS
  // here is still structural on purpose.
  import { listen } from "@tauri-apps/api/event";
  import { api, describeError } from "./lib/api.js";
  import ListView from "./lib/ListView.svelte";
  import TasksView from "./lib/TasksView.svelte";
  import CompletedView from "./lib/CompletedView.svelte";
  import TaskInspector from "./lib/TaskInspector.svelte";
  import WorkspaceView from "./lib/WorkspaceView.svelte";
  import NotesWidget from "./lib/NotesWidget.svelte";
  import NoteEditor from "./lib/NoteEditor.svelte";
  import HomeView from "./lib/HomeView.svelte";
  import SettingsView from "./lib/SettingsView.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import TitleBar from "./lib/TitleBar.svelte";
  import PageHeader from "./lib/PageHeader.svelte";
  import { listName } from "./lib/paths.js";
  import { S } from "./lib/strings.js";
  import * as Tabs from "./lib/tabs.js";

  let notebook = $state(null);
  let clock = $state(null);
  let error = $state(null);
  let busy = $state(true);
  /// Bumped to tell the open screen to re-read from disk.
  let reloadKey = $state(0);
  let counts = $state({});
  let conflicts = $state([]);
  let workspaces = $state([]);
  let noteFolders = $state([]);
  /// The task open in the right-hand panel, as `{ list, task }`.
  let selected = $state(null);
  /// What the open note reports about itself, for the page header and menu.
  let openNote = $state({ pinned: false, title: "" });

  // Tabs. The active tab's current view is what the centre panel shows.
  let tabs = $state([{ views: [{ kind: "home" }], at: 0 }]);
  let active = $state(0);
  let view = $derived(Tabs.currentView(tabs[active]) ?? { kind: "home" });

  /// Opens a view in its own tab (focusing it if already open).
  function openTab(next) {
    ({ tabs, active } = Tabs.open(tabs, active, next));
  }

  /// Navigates the active tab, replacing what it shows.
  function goTo(next) {
    ({ tabs, active } = Tabs.navigate(tabs, active, next));
  }

  /// Opens a fresh tab. Unlike following a link (which focuses an already-open
  /// document), the `+` is a deliberate "I want another tab" gesture, so it
  /// always appends — duplicates are the intent here, not an accident.
  function openNewTab() {
    tabs = [...tabs, { views: [{ kind: "home" }], at: 0 }];
    active = tabs.length - 1;
  }

  const closeTab = (i) => ({ tabs, active } = Tabs.close(tabs, active, i));
  const goBack = () => ({ tabs, active } = Tabs.back(tabs, active));
  const goForward = () => ({ tabs, active } = Tabs.forward(tabs, active));

  const isOpen = (v) => Tabs.viewId(view) === Tabs.viewId(v);

  function onKeydown(event) {
    if (event.key !== "Escape" || !selected) return;
    // A native date picker eats Escape to dismiss its own popup. Closing the
    // whole panel from under it would undo the edit the user came to make.
    if (event.target?.type === "date") return;
    selected = null;
  }

  const select = (list, task) => (selected = { list, task });

  // Leaving a screen closes the inspector — it would otherwise keep showing a
  // task from a list that is no longer visible.
  //
  // Depends on the active view and nothing else, on purpose: it used to read
  // `notebook` too, and since every save refreshes the notebook, the panel
  // slammed shut on each auto-save.
  $effect(() => {
    Tabs.viewId(view);
    selected = null;
  });

  // Records where the user is, for `restoreLastScreen`.
  $effect(() => {
    const id = Tabs.viewId(view);
    if (notebook) api.rememberScreen(id).catch(() => {});
  });

  function restoreView(id) {
    if (!id) return null;
    if (id === "day" || id === "week") return { kind: "period", period: id };
    if (
      id === "home" ||
      id === "completed" ||
      id === "notes" ||
      id === "tasks" ||
      id === "settings"
    )
      return { kind: id };
    if (id.startsWith("list:")) return { kind: "list", list: id.slice(5) };
    if (id.startsWith("ws:")) return { kind: "workspace", ws: id.slice(3) };
    return null;
  }

  // The addresses the core creates travel with the notebook. The frontend
  // used to mirror them in a names.js, and when the core renamed the
  // completed list the mirror went stale and a screen read a file that no
  // longer existed.
  let layout = $derived(
    notebook?.layout ?? {
      inbox: "Tasks/Inbox.md",
      completed: "Tasks/Completed.md",
      tasksFolder: "Tasks",
      completedName: "Completed",
      notesFolder: "Notes",
      notesInbox: "Inbox",
      dateDisplayFormat: "dd-mm-yyyy",
      closeInspectorOnClickAway: false,
      quickNoteFolder: "Inbox",
    },
  );

  let userLists = $derived(
    (notebook?.lists ?? []).filter(
      (entry) =>
        entry.path !== layout.completed &&
        entry.path !== layout.inbox &&
        // Lists of user workspaces are reached through their workspace, not
        // flattened into the fixed sidebar — two Inboxes side by side with
        // the same label would be unreadable.
        entry.path.startsWith(`${layout.tasksFolder}/`),
    ),
  );

  let userWorkspaces = $derived(workspaces.filter((ws) => !ws.fixed));

  /// What a tab calls itself. Titles are derived, never stored: renaming a
  /// list has to reach the tab showing it.
  function titleOf(v) {
    switch (v?.kind) {
      case "home":
        return S.home;
      case "period":
        return v.period === "day" ? S.today : S.week;
      case "tasks":
        return S.tasks;
      case "notes":
        return S.notes;
      case "settings":
        return S.settings;
      case "completed":
        return S.completed;
      case "list":
        return listName(v.list);
      case "note":
        return listName(v.path);
      case "workspace":
        return (
          workspaces.find((w) => w.folderName === v.ws)?.name ?? v.ws
        );
      default:
        return S.untitled;
    }
  }

  /// The page menu of the current screen — the `•••` of the wireframe.
  let pageMenu = $derived.by(() => {
    if (notebook?.readOnly) return [];

    // A note's own actions belong here, not to a second bar inside the page.
    if (view.kind === "note") {
      return [
        { label: openNote.pinned ? S.unpin : S.pin, run: toggleNotePin },
        { label: S.renameNote, run: renameCurrentNote },
        { label: S.deleteNote, run: deleteCurrentNote },
      ];
    }

    const items = [{ label: S.newListButton, run: createList }];
    // Renaming or deleting a list the app recreates on every open would only
    // confuse — the core refuses it anyway, so the menu must not offer it.
    if (
      view.kind === "list" &&
      view.list !== layout.inbox &&
      view.list !== layout.completed
    ) {
      items.unshift(
        { label: S.renameList, run: renameCurrentList },
        { label: S.deleteList, run: deleteCurrentList },
      );
    }
    return items;
  });

  // --- the open note's document actions, owned by the shell because each
  // one changes what the tab points at ---

  let noteEditor = $state(null);

  async function noteAction(fn) {
    try {
      // Anything still being typed goes out first: renaming or deleting
      // underneath a pending write would lose it.
      await noteEditor?.flushPending();
      await fn();
      await refreshNotebook();
      reload();
    } catch (e) {
      fail(e);
    }
  }

  const toggleNotePin = () =>
    noteAction(async () => {
      await api.setNotePinned(view.folder, view.path, !openNote.pinned);
      openNote = { ...openNote, pinned: !openNote.pinned };
    });

  const renameCurrentNote = () =>
    noteAction(async () => {
      const next = prompt(S.promptRenameNote(openNote.title), openNote.title);
      if (!next || next.trim() === openNote.title) return;
      const moved = await api.renameNote(view.folder, view.path, next.trim());
      // The tab follows the file rather than pointing at a name that is gone.
      tabs = Tabs.replaceView(tabs, Tabs.viewId(view), {
        kind: "note",
        folder: view.folder,
        path: moved,
      });
    });

  const deleteCurrentNote = () =>
    noteAction(async () => {
      if (!confirm(S.confirmDeleteNote(openNote.title))) return;
      await api.deleteNote(view.folder, view.path);
      closeTab(active);
    });

  function fail(e) {
    error = describeError(e);
    console.error("[memo]", e);
  }

  const reload = () => (reloadKey += 1);

  // One round trip instead of four: the auto-save calls this on every pause
  // in typing, so the fan-out was the hottest path in the app.
  async function refreshNotebook() {
    try {
      const snap = await api.notebookSnapshot();
      notebook = snap.info;
      clock = snap.clock;
      counts = snap.counts;
      conflicts = snap.conflicts;
      workspaces = snap.workspaces ?? [];
      noteFolders = await api.noteFolders(snap.info.layout.notesFolder);
    } catch {
      // No notebook open (or it just closed): back to onboarding.
      notebook = null;
    }
  }

  async function openAt(path) {
    busy = true;
    error = null;
    try {
      notebook = await api.openNotebook(path);
      await refreshNotebook();

      const restored = restoreView(await api.screenToRestore());
      if (restored) goTo(restored);

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
    const name = prompt(S.promptNewList);
    if (!name) return;
    try {
      await api.createList(layout.tasksFolder, name.trim());
      await refreshNotebook();
      openTab({ kind: "list", list: `${layout.tasksFolder}/${name.trim()}.md` });
    } catch (e) {
      fail(e);
    }
  }

  async function renameCurrentList() {
    if (view.kind !== "list") return;
    const from = view.list;
    const current = listName(from);
    const to = prompt(S.promptRenameList(current), current);
    if (!to || to.trim() === current) return;
    try {
      await api.renameList(from, to.trim());
      await refreshNotebook();
      // A rename never changes the folder: swap only the file name, and let
      // the tab follow the file instead of pointing at a name that is gone.
      const dir = from.slice(0, from.lastIndexOf("/"));
      const next = { kind: "list", list: `${dir}/${to.trim()}.md` };
      tabs = Tabs.replaceView(tabs, Tabs.viewId({ kind: "list", list: from }), next);
      reload();
    } catch (e) {
      fail(e);
    }
  }

  async function deleteCurrentList() {
    if (view.kind !== "list") return;
    const list = view.list;
    if (!confirm(S.confirmDeleteList(listName(list)))) return;
    try {
      const rescued = await api.deleteList(list);
      await refreshNotebook();
      goTo({ kind: "list", list: layout.inbox });
      reload();
      if (rescued > 0) error = S.tasksRescued(rescued, listName(list));
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

  /// Opening a document replaces what the tab shows, the way clicking a link
  /// does — a new tab is a deliberate gesture (middle click, or the option in
  /// the context menu), never the default.
  const showNote = (path, folder = layout.notesFolder, newTab = false) =>
    (newTab ? openTab : goTo)({ kind: "note", folder, path });

  const showList = (path, newTab = false) =>
    (newTab ? openTab : goTo)({ kind: "list", list: path });
</script>

<svelte:window onkeydown={onKeydown} />

<!-- The window is frameless: this bar draws the brand, the document tabs and
     the min/max/close controls itself, and is the only handle to move or close
     the window — so it renders even before a notebook is open. -->
<div class="window">
  <TitleBar>
    {#if notebook}
      <TabBar
        {tabs}
        {active}
        {titleOf}
        onSelect={(i) => (active = i)}
        onClose={closeTab}
        onOpenNew={openNewTab}
        onMove={(from, to) =>
          ({ tabs, active } = Tabs.move(tabs, active, from, to))}
      />
    {/if}
  </TitleBar>

  <main>
    {#if !notebook}
      <section class="shell__onboarding">
        <h1 class="shell__onboarding-title">Memo</h1>
        <p class="shell__onboarding-intro">{S.onboardingIntro}</p>
        <button
          class="shell__onboarding-action"
          onclick={chooseFolder}
          disabled={busy}>{S.chooseFolder}</button
        >
        {#if error}<p class="shell__error">{error}</p>{/if}
      </section>
  {:else}
    <div class="shell" class:shell--with-panel={selected}>
      <!-- LEFT: workspaces on top, notebook and settings pinned to the
           bottom, as the wireframe has them. -->
      <nav class="shell__sidebar">
        <div class="shell__sidebar-scroll">
          <button
            class="shell__nav-item"
            class:shell__nav-item--active={isOpen({ kind: "home" })}
            onclick={() => openTab({ kind: "home" })}>{S.home}</button
          >
          <button
            class="shell__nav-item"
            class:shell__nav-item--active={isOpen({ kind: "tasks" })}
            onclick={() => openTab({ kind: "tasks" })}>{S.tasks}</button
          >
          <button
            class="shell__nav-item"
            class:shell__nav-item--active={isOpen({ kind: "notes" })}
            onclick={() => openTab({ kind: "notes" })}>{S.notes}</button
          >

          <hr class="shell__divider" />
          {#each userLists as entry (entry.path)}
            <button
              class="shell__nav-item"
              class:shell__nav-item--active={isOpen({ kind: "list", list: entry.path })}
              onclick={() => showList(entry.path)}
              onauxclick={(e) =>
                e.button === 1 && (e.preventDefault(), showList(entry.path, true))}
              title={S.openInNewTab}
            >
              {entry.name}
              {#if counts[entry.path]}<span class="shell__count"
                  >{counts[entry.path]}</span
                >{/if}
            </button>
          {/each}

          {#if userWorkspaces.length > 0}
            <hr class="shell__divider" />
            <small class="shell__group-title">{S.workspacesTitle}</small>
            {#each userWorkspaces as ws (ws.folderName)}
              <button
                class="shell__nav-item"
                class:shell__nav-item--active={isOpen({ kind: "workspace", ws: ws.folderName })}
                onclick={() => openTab({ kind: "workspace", ws: ws.folderName })}
              >
                {ws.name}
              </button>
            {/each}
          {/if}

          <hr class="shell__divider" />
          <button
            class="shell__nav-item"
            class:shell__nav-item--active={isOpen({ kind: "completed" })}
            onclick={() => openTab({ kind: "completed" })}>{S.completed}</button
          >
          {#if !notebook.readOnly}
            <button
              class="shell__nav-item shell__nav-item--secondary"
              onclick={createList}>{S.newListButton}</button
            >
          {/if}
        </div>

        <div class="shell__sidebar-footer">
          <button
            class="shell__nav-item shell__nav-item--secondary"
            title={notebook.path}
            onclick={chooseFolder}
          >
            {notebook.name}
            {#if notebook.readOnly}<span class="shell__badge">{S.readOnly}</span>{/if}
          </button>
          <button
            class="shell__nav-item shell__nav-item--secondary"
            class:shell__nav-item--active={isOpen({ kind: "settings" })}
            onclick={() => openTab({ kind: "settings" })}>{S.settings}</button
          >
        </div>
      </nav>

      <!-- CENTRE: page header, then the screen itself. The tabs moved up into
           the title bar; the header keeps the back/forward, title and ••• menu. -->
      <section class="shell__centre">
        <PageHeader
          title={titleOf(view)}
          subtitle={view.kind === "home" ? (clock?.today ?? "") : ""}
          canBack={Tabs.canGoBack(tabs[active])}
          canForward={Tabs.canGoForward(tabs[active])}
          onBack={goBack}
          onForward={goForward}
          onRenameTitle={view.kind === "note" && !notebook.readOnly
            ? renameCurrentNote
            : null}
          menu={pageMenu}
        />

        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="shell__content" onclick={clickedAway}>
          {#if error}
            <p class="shell__error">
              {error}
              <button onclick={() => (error = null)}>{S.dismissError}</button>
            </p>
          {/if}

          {#if conflicts.length > 0}
            <!-- The one case where the user can silently lose work: two
                 devices edited the same file and the sync tool kept both. -->
            <div class="shell__conflict">
              <strong class="shell__conflict-title"
                >{S.conflictsTitle(conflicts.length)}</strong
              >
              <p>{S.conflictsBody}</p>
              <ul class="shell__conflict-list">
                {#each conflicts as conflict}
                  <li>
                    {#if conflict.list}<strong>{conflict.list}</strong>{/if}
                    <code class="shell__conflict-path">{conflict.path}</code>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}

          {#if view.kind === "home"}
            <HomeView
              dateFormat={layout.dateDisplayFormat}
              quickNoteFolder={layout.quickNoteFolder}
              notesFolder={layout.notesFolder}
              notesInbox={layout.notesInbox}
              folders={noteFolders}
              readOnly={notebook.readOnly}
              {reloadKey}
              onChanged={refreshNotebook}
              onError={fail}
              onOpenNote={(path) => showNote(path)}
              onSelectTask={select}
              selectedId={selected?.task?.id ?? null}
            />
          {:else if view.kind === "tasks"}
            <TasksView
              dateFormat={layout.dateDisplayFormat}
              inbox={layout.inbox}
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
              dateFormat={layout.dateDisplayFormat}
              list={view.list}
              readOnly={notebook.readOnly}
              onChanged={refreshNotebook}
              onError={fail}
              {reloadKey}
              onSelect={select}
              selectedId={selected?.task?.id ?? null}
            />
          {:else if view.kind === "notes"}
            <NotesWidget
              widget={{ kind: "notes", folder: layout.notesFolder }}
              readOnly={notebook.readOnly}
              notesInbox={layout.notesInbox}
              {reloadKey}
              onChanged={refreshNotebook}
              onError={fail}
              onOpenNote={showNote}
            />
          {:else if view.kind === "note"}
            <NoteEditor
              bind:this={noteEditor}
              folder={view.folder}
              path={view.path}
              readOnly={notebook.readOnly}
              onSaved={refreshNotebook}
              onError={fail}
              onLoaded={(state) => (openNote = state)}
            />
          {:else if view.kind === "settings"}
            <SettingsView
              {notebook}
              folders={noteFolders}
              notesInbox={layout.notesInbox}
              onChanged={refreshNotebook}
              onError={fail}
            />
          {:else if view.kind === "workspace"}
            {@const current = userWorkspaces.find((w) => w.folderName === view.ws)}
            {#if current}
              <WorkspaceView
                workspace={current}
                lists={notebook.lists}
                {counts}
                completedName={layout.completedName}
                notesInbox={layout.notesInbox}
                readOnly={notebook.readOnly}
                {reloadKey}
                onOpenList={(path) => showList(path)}
                onOpenNote={(path, folder) => showNote(path, folder)}
                onChanged={refreshNotebook}
                onError={fail}
              />
            {:else}
              <p class="shell__empty">{S.emptyWorkspace}</p>
            {/if}
          {:else}
            <CompletedView
              readOnly={notebook.readOnly}
              completedList={layout.completed}
              onChanged={refreshNotebook}
              onError={fail}
              {reloadKey}
            />
          {/if}
        </div>
      </section>

      <!-- RIGHT: the inspector, only while a task is open. -->
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
</div>

<style>
  /* Phase 10: the shell adopts the token layer. Components are rebuilt in
     later batches; this batch is the title bar + the window frame. */

  /* The frameless window's own frame: the OS gives no rounding or shadow to an
     undecorated window on GNOME/Wayland, so the app draws its own. `overflow:
     hidden` clips the content to the rounded corners; the transparent page
     behind (set in app.css) lets the corners show through. */
  .window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--theme-bg);
    border-radius: var(--theme-window-radius);
    overflow: hidden;
    box-shadow: var(--theme-window-shadow);
  }
  main {
    flex: 1;
    min-height: 0;
    font-family: var(--theme-font-sans);
  }
  .shell__onboarding {
    max-width: 32rem;
    margin: 4rem auto;
    padding: 1rem;
  }
  .shell {
    display: grid;
    grid-template-columns: var(--theme-sidebar-left) 1fr;
    height: 100%;
    min-height: 0;
  }
  /* Driven by a class, not `:has(aside)`: the panel is a child component and
     Svelte's scoped CSS would never match its element from here. The columns
     match the title bar's grid, so the tab strip aligns with the centre panel. */
  .shell--with-panel {
    grid-template-columns: var(--theme-sidebar-left) 1fr var(--theme-sidebar-right);
  }
  .shell__sidebar {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    border-right: 1px solid #ccc;
    min-height: 0;
  }
  .shell__sidebar-scroll {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.25rem;
    padding: 0.75rem;
    overflow-y: auto;
  }
  .shell__sidebar-footer {
    border-top: 1px solid #ddd;
    padding: 0.5rem 0.75rem;
  }
  .shell__nav-item {
    text-align: left;
    padding: 0.35rem 0.5rem;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    cursor: pointer;
  }
  .shell__nav-item:hover {
    background: #eee;
  }
  .shell__nav-item--active {
    background: #ddd;
    font-weight: 600;
  }
  .shell__nav-item--secondary {
    color: #555;
    font-size: 0.85rem;
  }
  .shell__sidebar-footer .shell__nav-item {
    width: 100%;
  }
  .shell__badge {
    display: block;
    color: #b00;
    font-size: 0.8rem;
  }
  .shell__centre {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .shell__content {
    flex: 1;
    padding: 1rem;
    overflow-y: auto;
  }
  .shell__error {
    background: #fee;
    border: 1px solid #f99;
    padding: 0.5rem;
    border-radius: 4px;
  }
  .shell__conflict {
    background: #fff8e1;
    border: 1px solid #e6c34a;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }
  .shell__conflict-list {
    margin: 0.5rem 0 0;
    padding-left: 1.2rem;
  }
  .shell__conflict-path {
    word-break: break-all;
    font-size: 0.8rem;
  }
  .shell__count {
    float: right;
    color: #666;
    font-size: 0.85rem;
    font-weight: normal;
  }
  .shell__group-title {
    color: #888;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.25rem 0.5rem 0;
  }
  .shell__empty {
    color: #666;
  }
  .shell__divider {
    border: none;
    border-top: 1px solid #ddd;
    width: 100%;
    margin: 0.25rem 0;
  }
</style>
