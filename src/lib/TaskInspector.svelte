<script>
  // The right-hand panel: everything about one task that the row is too small
  // to show. The row keeps quiet markers; this is where the fields are edited.
  //
  // Three rules shape this file:
  //
  // 1. Opening a task never writes. A task with no id only gets one when the
  //    user actually changes something, so clicking around to look at things
  //    leaves the `.md` byte for byte as it was.
  // 2. There is no save button. An edit that the user has to remember to
  //    confirm is an edit they will lose. Changes are written on their own,
  //    shortly after the typing stops.
  // 3. The task is saved whole, in one call. `set_task_fields` exists for
  //    exactly this, because a half-applied edit is worse than none.
  import { onDestroy } from "svelte";
  import { api } from "./api.js";
  import { ensureTaskId } from "./taskId.js";
  import { S } from "./strings.js";

  let {
    task,
    list,
    readOnly = false,
    onSaved,
    onError,
    onClose,
    // How long to wait after the last keystroke. A prop so tests can drop it
    // to zero instead of sleeping — a real user never sets this.
    saveDelay = 500,
  } = $props();

  let draft = $state(fromTask(null));
  let newTag = $state("");
  let newSubtask = $state("");

  // Plain variables, not state: none of this should re-render anything, and a
  // reactive `pending` would make the auto-save effect trigger itself.
  //
  // `slot` is the task the current draft belongs to, together with its id once
  // it has one. It is captured instead of read from the props at write time,
  // because the user can click another task while a write is still on its way
  // — and that write has to land on the task it was typed into.
  let slot = null;
  let baseline = "";
  let pending = null;
  let timer = null;

  // A different task selected means a different draft. Without this, editing
  // one task and clicking another would show the first one's typing.
  $effect(() => {
    task;
    list;
    // Whatever was typed into the previous task goes out now, addressed to
    // that task, before the draft is replaced.
    flush();
    slot = { list, task, id: task?.id ?? null };
    // Stringify the plain object before it becomes the reactive draft.
    // Reading `draft` here would make this effect depend on the state it
    // assigns, and Svelte would loop until it gave up.
    const fresh = fromTask(task);
    baseline = JSON.stringify(fresh);
    draft = fresh;
    newTag = "";
    newSubtask = "";
  });

  // The auto-save itself. Stringifying the draft subscribes to every field in
  // it; comparing against the baseline is what tells a real edit apart from
  // the effect above having just reloaded the same values.
  $effect(() => {
    const snapshot = JSON.stringify(draft);
    if (readOnly || snapshot === baseline) return;

    // The snapshot rides along with the fields it produced, so that marking
    // the write as done later cannot swallow something typed in between.
    pending = { fields: fields(), snapshot };
    if (timer) clearTimeout(timer);
    timer = setTimeout(write, saveDelay);
  });

  // A pending edit must not die with the panel — closing it is the most
  // natural moment to stop typing.
  onDestroy(() => flush());

  /// Sends whatever is waiting right now, without waiting for the timer.
  function flush() {
    if (!pending) return Promise.resolve();
    if (timer) clearTimeout(timer);
    timer = null;
    return write();
  }

  async function write() {
    // Read synchronously: by the time the first await returns, the user may
    // already have selected another task and replaced both of these.
    const target = slot;
    const job = pending;
    pending = null;
    timer = null;
    if (!target || !job) return;

    try {
      // The id is earned here, on a real change — never on opening the task.
      if (!target.id) target.id = await ensureTaskId(target.list, target.task);
      await api.setTaskFields(target.list, target.id, job.fields);
      // Only after it lands, and only if the panel is still on the same task:
      // a failed write has to be retried by the next edit, not quietly
      // counted as saved.
      if (target === slot) baseline = job.snapshot;
      onSaved?.();
    } catch (e) {
      onError?.(e);
    }
  }

  function fields() {
    return {
      // An emptied name would leave a checkbox with no text; keep the old one.
      text: draft.text.trim() || slot?.task?.text || "",
      // Present-but-null is how a field is cleared — absent would mean
      // "leave alone", and there would be no way to remove a date.
      due: draft.due || null,
      priority: Number(draft.priority) || null,
      tags: draft.tags,
      // Blank lines are dropped: an empty indented line ends the task block
      // in the file, which would cut the description in half on re-read.
      description: draft.description
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean),
      repeat: repeatText(),
      subtasks: draft.subtasks.map((s) => ({ text: s.text, done: s.done })),
    };
  }

  function fromTask(t) {
    return {
      text: t?.text ?? "",
      // `<input type="date">` speaks ISO, which is what the file stores too.
      due: t?.due ?? "",
      priority: t?.priority ?? 0,
      tags: [...(t?.tags ?? [])],
      description: (t?.description ?? []).join("\n"),
      repeatEvery: t?.repeat?.every ?? 1,
      repeatUnit: t?.repeat?.unit ?? "",
      subtasks: (t?.subtasks ?? []).map((s) => ({ ...s })),
    };
  }

  /// The bridge hands back `{ every, unit }`, but `set_task_fields` takes the
  /// written form. Building it here keeps an invalid value impossible: the
  /// core silently drops a `repeat:` it cannot parse.
  function repeatText() {
    if (!draft.repeatUnit) return null;
    const every = Math.max(1, Number(draft.repeatEvery) || 1);
    if (every === 1) return `every-${draft.repeatUnit}`;
    return `every-${every}-${draft.repeatUnit}s`;
  }

  /// Takes the chosen date and gets the calendar out of the way.
  ///
  /// The native picker in WebKitGTK has no confirm button and does not close
  /// itself once a day is clicked — the only way out was clicking outside the
  /// whole window. Dropping focus dismisses it.
  function pickDate(event) {
    draft.due = event.currentTarget.value;
    event.currentTarget.blur();
  }

  /// Removing a date needs its own control: the field shows a whole date or
  /// nothing, and there is no keystroke in the picker that means "none".
  function clearDate() {
    draft.due = "";
  }

  /// A tag with a space in it would break the metadata line on the next read:
  /// the loose word stops the line from being all-tokens, and the whole thing
  /// turns into a description. Cheaper to fix the tag than to lose the fields.
  function cleanTag(raw) {
    return raw.trim().replace(/^#+/, "").replace(/\s+/g, "-");
  }

  function addTag() {
    const tag = cleanTag(newTag);
    newTag = "";
    if (tag && !draft.tags.includes(tag)) draft.tags = [...draft.tags, tag];
  }

  const removeTag = (tag) => (draft.tags = draft.tags.filter((t) => t !== tag));

  function addSubtask() {
    const text = newSubtask.trim();
    newSubtask = "";
    if (text) draft.subtasks = [...draft.subtasks, { text, done: false }];
  }

  const removeSubtask = (i) =>
    (draft.subtasks = draft.subtasks.filter((_, at) => at !== i));

  async function complete() {
    if (readOnly) return;
    // Any typing goes out first, and is waited for: completing moves the task
    // to another file, and two writes racing would both try to hand out the
    // id — the loser failing on a task that no longer has one to claim.
    await flush();
    const target = slot;
    try {
      if (!target.id) target.id = await ensureTaskId(target.list, target.task);
      await api.completeTask(target.list, target.id);
      onSaved?.();
      // The task just left this list, so the panel is pointing at nothing.
      onClose?.();
    } catch (e) {
      onError?.(e);
    }
  }
</script>

<aside>
  <header>
    <input
      type="checkbox"
      checked={task?.done ?? false}
      disabled={readOnly}
      onchange={complete}
      aria-label={S.complete}
    />
    <input
      class="title"
      bind:value={draft.text}
      disabled={readOnly}
      aria-label={S.taskName}
    />
    <button class="close" onclick={() => onClose?.()} aria-label={S.closePanel}>×</button>
  </header>

  <section>
    <h3>{S.subtasksTitle}</h3>
    <ul>
      {#each draft.subtasks as subtask, i (i)}
        <li>
          <input
            type="checkbox"
            bind:checked={subtask.done}
            disabled={readOnly}
            aria-label={S.subtaskLabel(subtask.text)}
          />
          <input bind:value={subtask.text} disabled={readOnly} />
          {#if !readOnly}
            <button onclick={() => removeSubtask(i)} aria-label={S.removeSubtask}
              >×</button
            >
          {/if}
        </li>
      {/each}
    </ul>
    {#if !readOnly}
      <form onsubmit={(e) => (e.preventDefault(), addSubtask())}>
        <input placeholder={S.newSubtaskPlaceholder} bind:value={newSubtask} />
      </form>
    {/if}
  </section>

  <section>
    <h3>{S.tagsTitle}</h3>
    <div class="tags">
      {#each draft.tags as tag (tag)}
        <span class="tag">
          #{tag}
          {#if !readOnly}
            <button onclick={() => removeTag(tag)} aria-label={S.removeTag(tag)}
              >×</button
            >
          {/if}
        </span>
      {/each}
    </div>
    {#if !readOnly}
      <form onsubmit={(e) => (e.preventDefault(), addTag())}>
        <input placeholder={S.newTagPlaceholder} bind:value={newTag} />
      </form>
    {/if}
  </section>

  <section>
    <h3>{S.descriptionTitle}</h3>
    <textarea bind:value={draft.description} disabled={readOnly} rows="4"
    ></textarea>
  </section>

  <section class="fields">
    <label>
      {S.dueDateLabel}
      <span class="date">
        <!-- `onchange`, not `bind:value`: a date field emits `input` for every
             half-typed value while the picker is open, so binding it would
             save an empty date between choosing the month and choosing the
             day. `change` only fires once the value is a whole date, or
             cleared. -->
        <input
          type="date"
          value={draft.due}
          onchange={pickDate}
          disabled={readOnly}
        />
        {#if !readOnly && draft.due}
          <button onclick={clearDate} aria-label={S.clearDate} title={S.clearDateHint}
            >×</button
          >
        {/if}
      </span>
    </label>

    <label>
      {S.priorityLabel}
      <select bind:value={draft.priority} disabled={readOnly}>
        <option value={0}>{S.priorityNone}</option>
        <option value={1}>{S.priorityHigh}</option>
        <option value={2}>{S.priorityMedium}</option>
        <option value={3}>{S.priorityLow}</option>
      </select>
    </label>

    <label>
      {S.repeatLabel}
      <span class="repeat">
        <input
          type="number"
          min="1"
          bind:value={draft.repeatEvery}
          disabled={readOnly || !draft.repeatUnit}
          aria-label={S.repeatEvery}
        />
        <select bind:value={draft.repeatUnit} disabled={readOnly}>
          <option value="">{S.noRepeat}</option>
          <option value="day">{S.repeatDays}</option>
          <option value="week">{S.repeatWeeks}</option>
          <option value="month">{S.repeatMonths}</option>
        </select>
      </span>
    </label>
  </section>

</aside>

<style>
  /* Structural only, like the rest of the app until phase 10. */
  aside {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 0.75rem;
    border-left: 1px solid #ccc;
    overflow-y: auto;
    font-size: 0.9rem;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .title {
    flex: 1;
    font: inherit;
    font-weight: 600;
  }
  .close {
    background: none;
    border: none;
    font-size: 1.1rem;
    cursor: pointer;
    color: #666;
  }
  h3 {
    margin: 0 0 0.25rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #666;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.1rem 0;
  }
  li input:not([type]) {
    flex: 1;
    font: inherit;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .tag {
    background: #e3ecff;
    color: #24468a;
    border-radius: 10px;
    padding: 0.05rem 0.4rem;
    font-size: 0.8rem;
  }
  .tag button {
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    padding: 0;
  }
  textarea {
    width: 100%;
    font: inherit;
    resize: vertical;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .date,
  .repeat {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .date button {
    background: none;
    border: none;
    cursor: pointer;
    color: #666;
    font-size: 1rem;
    padding: 0 0.15rem;
  }
  .repeat input {
    width: 3.5rem;
  }
  form input {
    width: 100%;
  }
</style>
