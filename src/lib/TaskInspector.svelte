<script>
  // The right-hand panel: everything about one task that the row is too small
  // to show. The row keeps quiet markers; this is where the fields are edited.
  //
  // Two rules shape this file:
  //
  // 1. Opening a task never writes. A task with no id only gets one when the
  //    first save happens, so clicking around to look at things leaves the
  //    `.md` byte for byte as it was.
  // 2. The task is saved whole, in one call. `set_task_fields` exists for
  //    exactly this, because a half-applied edit is worse than none.
  import { api } from "./api.js";
  import { ensureTaskId } from "./taskId.js";

  let { task, list, readOnly = false, onSaved, onError, onClose } = $props();

  let draft = $state(fromTask(null));
  let saving = $state(false);
  let newTag = $state("");
  let newSubtask = $state("");
  /// The id this task is known by, once it has one.
  ///
  /// Needed because the screen above still holds the copy it selected, which
  /// has no id yet. Without remembering it here, saving a second time would
  /// look for an id-less task that no longer exists and fail.
  let knownId = $state(null);

  // A different task selected means a different draft. Without this, editing
  // one task and clicking another would show the first one's typing.
  $effect(() => {
    task;
    list;
    draft = fromTask(task);
    knownId = task?.id ?? null;
    newTag = "";
    newSubtask = "";
  });

  /// The id of the task being edited, assigning one on first need.
  async function taskId() {
    if (!knownId) knownId = await ensureTaskId(list, task);
    return knownId;
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

  async function act(fn) {
    if (readOnly || saving) return;
    saving = true;
    try {
      await fn();
      onSaved?.();
    } catch (e) {
      onError?.(e);
    } finally {
      saving = false;
    }
  }

  const save = () =>
    act(async () => {
      const id = await taskId();
      await api.setTaskFields(list, id, {
        // An emptied name would leave a checkbox with no text; keep the old one.
        text: draft.text.trim() || task.text,
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
      });
    });

  const complete = () =>
    act(async () => {
      const id = await taskId();
      await api.completeTask(list, id);
      // The task just left this list, so the panel is pointing at nothing.
      onClose?.();
    });
</script>

<aside>
  <header>
    <input
      type="checkbox"
      checked={task?.done ?? false}
      disabled={readOnly || saving}
      onchange={complete}
      aria-label="concluir"
    />
    <input
      class="title"
      bind:value={draft.text}
      disabled={readOnly}
      aria-label="nome da tarefa"
    />
    <button class="close" onclick={() => onClose?.()} aria-label="fechar">×</button>
  </header>

  <section>
    <h3>Subtarefas</h3>
    <ul>
      {#each draft.subtasks as subtask, i (i)}
        <li>
          <input
            type="checkbox"
            bind:checked={subtask.done}
            disabled={readOnly}
            aria-label={`subtarefa: ${subtask.text}`}
          />
          <input bind:value={subtask.text} disabled={readOnly} />
          {#if !readOnly}
            <button onclick={() => removeSubtask(i)} aria-label="remover subtarefa"
              >×</button
            >
          {/if}
        </li>
      {/each}
    </ul>
    {#if !readOnly}
      <form onsubmit={(e) => (e.preventDefault(), addSubtask())}>
        <input placeholder="Nova subtarefa…" bind:value={newSubtask} />
      </form>
    {/if}
  </section>

  <section>
    <h3>Tags</h3>
    <div class="tags">
      {#each draft.tags as tag (tag)}
        <span class="tag">
          #{tag}
          {#if !readOnly}
            <button onclick={() => removeTag(tag)} aria-label={`remover #${tag}`}
              >×</button
            >
          {/if}
        </span>
      {/each}
    </div>
    {#if !readOnly}
      <form onsubmit={(e) => (e.preventDefault(), addTag())}>
        <input placeholder="Nova tag…" bind:value={newTag} />
      </form>
    {/if}
  </section>

  <section>
    <h3>Descrição</h3>
    <textarea bind:value={draft.description} disabled={readOnly} rows="4"
    ></textarea>
  </section>

  <section class="fields">
    <label>
      Data
      <input type="date" bind:value={draft.due} disabled={readOnly} />
    </label>

    <label>
      Prioridade
      <select bind:value={draft.priority} disabled={readOnly}>
        <option value={0}>nenhuma</option>
        <option value={1}>!1 alta</option>
        <option value={2}>!2 média</option>
        <option value={3}>!3 baixa</option>
      </select>
    </label>

    <label>
      Repetir
      <span class="repeat">
        <input
          type="number"
          min="1"
          bind:value={draft.repeatEvery}
          disabled={readOnly || !draft.repeatUnit}
          aria-label="a cada"
        />
        <select bind:value={draft.repeatUnit} disabled={readOnly}>
          <option value="">não repete</option>
          <option value="day">dia(s)</option>
          <option value="week">semana(s)</option>
          <option value="month">mês(es)</option>
        </select>
      </span>
    </label>
  </section>

  {#if !readOnly}
    <footer>
      <button class="primary" onclick={save} disabled={saving}>Salvar</button>
      <button onclick={() => (draft = fromTask(task))} disabled={saving}>
        Descartar
      </button>
    </footer>
  {/if}
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
  .repeat {
    display: flex;
    gap: 0.25rem;
  }
  .repeat input {
    width: 3.5rem;
  }
  footer {
    display: flex;
    gap: 0.5rem;
    margin-top: auto;
    padding-top: 0.5rem;
  }
  .primary {
    font-weight: 600;
  }
  form input {
    width: 100%;
  }
</style>
