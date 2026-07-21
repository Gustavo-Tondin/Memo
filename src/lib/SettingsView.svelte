<script>
  // The settings screen: every documented key of the notebook, editable.
  //
  // Two rules it follows, both inherited from the core:
  //
  // 1. **Every field is optional on the way in.** `set_notebook_settings`
  //    keeps whatever it is not told about, so this screen can send one key
  //    at a time and never has to hold — or risk overwriting — the rest.
  // 2. **The core normalises.** An offset it cannot parse falls back to the
  //    default rather than being stored wrong, so the UI never has to
  //    validate a second time. What it *does* do is stop a bad value from
  //    being offered at all: modes, week start and date shape are selects.
  import { api } from "./api.js";
  import { S } from "./strings.js";

  let { notebook, folders = [], notesInbox = "Inbox", onChanged, onError } =
    $props();

  let settings = $state(null);
  /// What the controls are bound to.
  ///
  /// Separate from `settings` on purpose: a control needs a *bound* variable
  /// to be pushed back by Svelte. With a plain `value=` attribute, a value
  /// the core rejected would stay on screen — the state never changed, only
  /// the DOM did, so nothing would put it back.
  let form = $state({});
  let saved = $state(false);
  let savedTimer = null;

  // Whatever the core says is what the controls show.
  $effect(() => {
    if (settings) form = { ...settings };
  });

  $effect(() => {
    load();
  });

  async function load() {
    try {
      settings = await api.notebookSettings();
    } catch (e) {
      onError?.(e);
    }
  }

  /// Sends one key. The core keeps everything it was not told about.
  async function put(patch) {
    try {
      await api.setNotebookSettings(patch);
      // Re-read rather than trusting what we sent: the core may have
      // normalised the value, and the screen should show what was stored.
      settings = await api.notebookSettings();
      onChanged?.();

      saved = true;
      clearTimeout(savedTimer);
      savedTimer = setTimeout(() => (saved = false), 1500);
    } catch (e) {
      onError?.(e);
    }
  }

  const DATE_SHAPES = ["dd-mm-yyyy", "dd/mm/yyyy", "mm/dd/yyyy", "yyyy-mm-dd"];

  let readOnly = $derived(!!notebook?.readOnly);
</script>

{#if settings && form}
  {#if readOnly}
    <p class="notice">{S.readOnlyNotice}</p>
  {/if}

  <section>
    <h2>{S.sectionDay}</h2>

    <label class="row">
      <span>{S.rolloverDaily}</span>
      <input
        bind:value={form.dailyAt}
        disabled={readOnly}
        aria-label={S.rolloverDaily}
        onchange={(e) => put({ dailyAt: e.currentTarget.value })}
      />
    </label>
    <p class="hint">{S.rolloverAtHint}</p>

    <label class="row">
      <span>{S.rolloverMode}</span>
      <select
        bind:value={form.dailyMode}
        disabled={readOnly}
        aria-label={`${S.rolloverDaily} — ${S.rolloverMode}`}
        onchange={(e) => put({ dailyMode: e.currentTarget.value })}
      >
        <option value="reset">{S.rolloverModeReset}</option>
        <option value="carry">{S.rolloverModeCarry}</option>
      </select>
    </label>

    <label class="row">
      <span>{S.rolloverWeekly}</span>
      <input
        bind:value={form.weeklyAt}
        disabled={readOnly}
        aria-label={S.rolloverWeekly}
        onchange={(e) => put({ weeklyAt: e.currentTarget.value })}
      />
    </label>

    <label class="row">
      <span>{S.rolloverMode}</span>
      <select
        bind:value={form.weeklyMode}
        disabled={readOnly}
        aria-label={`${S.rolloverWeekly} — ${S.rolloverMode}`}
        onchange={(e) => put({ weeklyMode: e.currentTarget.value })}
      >
        <option value="reset">{S.rolloverModeReset}</option>
        <option value="carry">{S.rolloverModeCarry}</option>
      </select>
    </label>

    <label class="row">
      <span>{S.weekStartsOn}</span>
      <select
        bind:value={form.weekStartsOn}
        disabled={readOnly}
        aria-label={S.weekStartsOn}
        onchange={(e) => put({ weekStartsOn: e.currentTarget.value })}
      >
        <option value="monday">{S.monday}</option>
        <option value="sunday">{S.sunday}</option>
      </select>
    </label>
  </section>

  <section>
    <h2>{S.sectionDisplay}</h2>

    <label class="row">
      <span>{S.dateFormat}</span>
      <select
        bind:value={form.dateDisplayFormat}
        disabled={readOnly}
        aria-label={S.dateFormat}
        onchange={(e) => put({ dateDisplayFormat: e.currentTarget.value })}
      >
        {#each DATE_SHAPES as shape (shape)}
          <option value={shape}>{shape}</option>
        {/each}
      </select>
    </label>

    <label class="row">
      <span>{S.showListCounts}</span>
      <input
        type="checkbox"
        bind:checked={form.showListCounts}
        disabled={readOnly}
        aria-label={S.showListCounts}
        onchange={(e) => put({ showListCounts: e.currentTarget.checked })}
      />
    </label>

    <label class="row">
      <span>{S.restoreLastScreen}</span>
      <input
        type="checkbox"
        bind:checked={form.restoreLastScreen}
        disabled={readOnly}
        aria-label={S.restoreLastScreen}
        onchange={(e) => put({ restoreLastScreen: e.currentTarget.checked })}
      />
    </label>
    <p class="hint">{S.restoreLastScreenHint}</p>

    <label class="row">
      <span>{S.autoUrgentByDate}</span>
      <input
        type="checkbox"
        bind:checked={form.autoUrgentByDate}
        disabled={readOnly}
        aria-label={S.autoUrgentByDate}
        onchange={(e) => put({ autoUrgentByDate: e.currentTarget.checked })}
      />
    </label>
    <p class="hint">{S.autoUrgentByDateHint}</p>

    <label class="row">
      <span>{S.closeOnClickAway}</span>
      <input
        type="checkbox"
        bind:checked={form.closeInspectorOnClickAway}
        disabled={readOnly}
        aria-label={S.closeOnClickAway}
        onchange={(e) =>
          put({ closeInspectorOnClickAway: e.currentTarget.checked })}
      />
    </label>
    <p class="hint">{S.closeOnClickAwayHint}</p>

    <label class="row">
      <span>{S.quickNoteFolder}</span>
      <select
        bind:value={form.quickNoteFolder}
        disabled={readOnly}
        aria-label={S.quickNoteFolder}
        onchange={(e) => put({ quickNoteFolder: e.currentTarget.value })}
      >
        <option value={notesInbox}>{notesInbox}</option>
        {#each folders.filter((f) => f !== notesInbox) as name (name)}
          <option value={name}>{name}</option>
        {/each}
      </select>
    </label>
  </section>

  <section>
    <h2>{S.sectionNotebook}</h2>
    <p class="row">
      <span>{S.notebookPath}</span>
      <code>{notebook?.path}</code>
    </p>
  </section>

  {#if saved}<p class="saved">{S.settingsSaved}</p>{/if}
{/if}

<style>
  section {
    margin-bottom: 1.5rem;
    max-width: 34rem;
  }
  h2 {
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #666;
    margin: 0 0 0.5rem;
    border-bottom: 1px solid #eee;
    padding-bottom: 0.25rem;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.3rem 0;
    margin: 0;
  }
  .row input:not([type="checkbox"]),
  .row select {
    font: inherit;
    min-width: 12rem;
  }
  .hint {
    margin: 0 0 0.6rem;
    font-size: 0.8rem;
    color: #888;
  }
  code {
    font-size: 0.85rem;
    color: #555;
    word-break: break-all;
  }
  .notice {
    background: #fff8e1;
    border: 1px solid #e6c34a;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
  }
  .saved {
    color: #2a7;
    font-size: 0.85rem;
  }
</style>
