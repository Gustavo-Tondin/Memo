// Screen tests with the bridge mocked.
//
// What these catch: a button wired to the wrong command, arguments in the
// wrong shape, a screen that never reloads after acting, an action offered
// on a read-only notebook. What they deliberately do NOT check is whether
// the core does the right thing with those calls — that lives in Rust, and
// duplicating it here would just be a slower copy.

import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

const { default: ListView } = await import("./ListView.svelte");
const { default: PeriodView } = await import("./PeriodView.svelte");
const { default: CompletedView } = await import("./CompletedView.svelte");
const { default: TaskInspector } = await import("./TaskInspector.svelte");

const task = (id, text, extra = {}) => ({
  id,
  text,
  done: false,
  origin: null,
  meta: null,
  indent: "",
  ...extra,
});

/// Answers each command with whatever `responses` says.
function bridge(responses) {
  invoke.mockImplementation((cmd, args) => {
    if (!(cmd in responses)) return Promise.resolve(null);
    const value = responses[cmd];
    return Promise.resolve(typeof value === "function" ? value(args) : value);
  });
}

const noop = () => {};

beforeEach(() => invoke.mockReset());

describe("ListView", () => {
  test("shows the tasks of the list it was given", async () => {
    bridge({ list_tasks: [task("a1", "Comprar leite")] });

    render(ListView, {
      props: { list: "Compras", readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    expect(await screen.findByText("Comprar leite")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("list_tasks", { list: "Compras" });
  });

  test("adding a task calls create_task and reloads", async () => {
    bridge({ list_tasks: [], create_task: "novo" });

    render(ListView, {
      props: { list: "Inbox", readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await userEvent.type(await screen.findByPlaceholderText("Nova tarefa…"), "Ligar pro dentista");
    await userEvent.click(screen.getByText("Adicionar"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_task", {
        list: "Inbox",
        text: "Ligar pro dentista",
      }),
    );
    // Reloaded after acting, so the new task actually shows up.
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "list_tasks").length).toBeGreaterThan(1);
  });

  test("checking a task completes it", async () => {
    bridge({ list_tasks: [task("a1", "Comprar leite")], complete_task: {} });

    render(ListView, {
      props: { list: "Compras", readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await userEvent.click(await screen.findByLabelText("concluir"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("complete_task", { list: "Compras", id: "a1" }),
    );
  });

  test("pulling sends the task to the right period", async () => {
    bridge({ list_tasks: [task("a1", "Comprar leite")], pull_into_period: true });

    render(ListView, {
      props: { list: "Compras", readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await userEvent.click(await screen.findByText("→ Semana"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("pull_into_period", {
        period: "week",
        list: "Compras",
        id: "a1",
      }),
    );
  });

  test("duplicated ids still render every line", async () => {
    // The core de-duplicates ids on read, but not on a read-only notebook —
    // and a duplicate key makes Svelte abort the whole list, which showed up
    // as an empty Inbox while the same task still appeared under Today.
    bridge({
      list_tasks: [task("abc123", "Comprar leite"), task("abc123", "Comprar leite")],
    });

    render(ListView, {
      props: { list: "Inbox", readOnly: true, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await waitFor(() =>
      expect(screen.getAllByText("Comprar leite").length).toBe(2),
    );
  });

  test("pulling a task with no id gives it one first", async () => {
    // Written by hand in another editor: there is nothing to reference in the
    // day state until the core hands out an id.
    bridge({
      list_tasks: [task(null, "Escrita à mão")],
      ensure_task_id: "new1",
      pull_into_period: true,
    });

    render(ListView, {
      props: { list: "Compras", readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await userEvent.click(await screen.findByText("→ Hoje"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("pull_into_period", {
        period: "day",
        list: "Compras",
        id: "new1",
      }),
    );
  });

  test("clicking a task opens it instead of renaming it", async () => {
    bridge({ list_tasks: [task("a1", "Comprar leite")] });
    const opened = [];

    render(ListView, {
      props: {
        list: "Compras",
        readOnly: false,
        onChanged: noop,
        onError: noop,
        reloadKey: 0,
        onSelect: (list, t) => opened.push([list, t.id]),
      },
    });

    await userEvent.click(await screen.findByText("Comprar leite"));

    expect(opened).toEqual([["Compras", "a1"]]);
  });

  test("a read-only notebook offers no way to add tasks", async () => {
    bridge({ list_tasks: [task("a1", "Comprar leite")] });

    render(ListView, {
      props: { list: "Compras", readOnly: true, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await screen.findByText("Comprar leite");
    expect(screen.queryByPlaceholderText("Nova tarefa…")).toBeNull();
  });

  test("a failing command is reported instead of swallowed", async () => {
    const errors = [];
    bridge({
      list_tasks: [],
      create_task: () => Promise.reject({ kind: "io", message: "disco cheio" }),
    });

    render(ListView, {
      props: {
        list: "Compras",
        readOnly: false,
        onChanged: noop,
        onError: (e) => errors.push(e),
        reloadKey: 0,
      },
    });

    await userEvent.type(await screen.findByPlaceholderText("Nova tarefa…"), "qualquer");
    await userEvent.click(screen.getByText("Adicionar"));

    await waitFor(() => expect(errors.length).toBeGreaterThan(0));
    expect(errors[0].kind).toBe("io");
  });
});

describe("PeriodView", () => {
  const props = {
    period: "day",
    clock: { today: "2026-07-20", weekStart: "2026-07-20" },
    readOnly: false,
    onChanged: noop,
    onError: noop,
    reloadKey: 0,
  };

  test("separates what is pulled from what is suggested", async () => {
    bridge({
      period_tasks: [{ list: "Compras", task: task("a1", "Puxada") }],
      grouped_suggestions: [
        { list: "Inbox", task: task("b2", "Sugerida"), group: "lists" },
      ],
    });

    render(PeriodView, { props });

    expect(await screen.findByText("Puxada")).toBeTruthy();
    expect(await screen.findByText("Sugerida")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("period_tasks", { period: "day" });
    expect(invoke).toHaveBeenCalledWith("grouped_suggestions", { period: "day" });
  });

  test("suggestions are shown under the group that explains them", async () => {
    bridge({
      period_tasks: [],
      grouped_suggestions: [
        { list: "Inbox", task: task("a1", "Vencida"), group: "urgent" },
        { list: "Inbox", task: task("b2", "Tranquila"), group: "lists" },
      ],
    });

    render(PeriodView, { props });

    expect(await screen.findByText("Urgente (1)")).toBeTruthy();
    expect(await screen.findByText("Das listas (1)")).toBeTruthy();
    expect(screen.queryByText("Em breve (0)")).toBeNull();
  });

  test("a task created here goes through add_task_in_period", async () => {
    // The core writes it to the Inbox; the screen must not pick a list itself.
    bridge({ period_tasks: [], grouped_suggestions: [], add_task_in_period: "novo" });

    render(PeriodView, { props });

    await userEvent.type(
      await screen.findByPlaceholderText("Nova tarefa (vai para a Inbox)…"),
      "Responder e-mail",
    );
    await userEvent.click(screen.getByText("Adicionar"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("add_task_in_period", {
        period: "day",
        text: "Responder e-mail",
      }),
    );
  });

  test("removing a pulled task only touches the period", async () => {
    bridge({
      period_tasks: [{ list: "Compras", task: task("a1", "Puxada") }],
      grouped_suggestions: [],
      remove_from_period: true,
    });

    render(PeriodView, { props });
    await userEvent.click(await screen.findByText("tirar"));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("remove_from_period", {
        period: "day",
        list: "Compras",
        id: "a1",
      }),
    );
    expect(invoke.mock.calls.some(([cmd]) => cmd === "complete_task")).toBe(false);
  });

  test("the week screen asks for week data", async () => {
    // `grouped_suggestions`, not `period_suggestions`: mocking the command the
    // screen stopped calling in phase 6 left `suggestions` null and threw
    // while rendering, which vitest reported as an unhandled error instead of
    // a failing test.
    bridge({ period_tasks: [], grouped_suggestions: [] });

    render(PeriodView, { props: { ...props, period: "week" } });

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("period_tasks", { period: "week" }),
    );
  });
});

describe("CompletedView", () => {
  test("unchecking sends the task back through uncomplete_task", async () => {
    bridge({
      list_tasks: [task("a1", "Pagar internet", { done: true, origin: "Compras" })],
      uncomplete_task: {},
    });

    render(CompletedView, {
      props: { readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    expect(await screen.findByText("volta para Compras")).toBeTruthy();
    await userEvent.click(screen.getByLabelText("desmarcar"));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("uncomplete_task", { id: "a1" }));
  });

  test("a task with no id cannot be unchecked", async () => {
    // Hand-written in another editor and not yet adopted: acting on it would
    // have nothing to address.
    bridge({ list_tasks: [task(null, "Escrita à mão", { done: true })] });

    render(CompletedView, {
      props: { readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await screen.findByText("Escrita à mão");
    expect(screen.getByLabelText("desmarcar").disabled).toBe(true);
  });

  test("reads the list by the name the core actually writes", async () => {
    // The core renamed this list to English in phase 5 and this screen kept
    // asking for "Completas". A missing file reads as empty, so the screen
    // showed "nothing completed yet" forever instead of failing.
    bridge({ list_tasks: [] });

    render(CompletedView, {
      props: { readOnly: false, onChanged: noop, onError: noop, reloadKey: 0 },
    });

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("list_tasks", { list: "Completed" }),
    );
  });
});


describe("TaskInspector", () => {
  // saveDelay 0 so the debounce resolves on the next tick instead of the test
  // sitting through half a second of nothing.
  const props = (task, extra = {}) => ({
    task,
    list: "Compras",
    readOnly: false,
    saveDelay: 0,
    onSaved: noop,
    onError: noop,
    onClose: noop,
    ...extra,
  });

  /// The fields of the last `set_task_fields` call.
  const lastSave = () =>
    invoke.mock.calls.filter(([cmd]) => cmd === "set_task_fields").at(-1)[1];

  const saveCount = () =>
    invoke.mock.calls.filter(([cmd]) => cmd === "set_task_fields").length;

  test("opening a task writes nothing at all", async () => {
    // The whole point of the lazy id, and now of the auto-save too: looking at
    // a task must leave the `.md` untouched. Saving on open would give every
    // hand-written task an id just for having been clicked.
    bridge({ ensure_task_id: "new1", set_task_fields: null });

    render(TaskInspector, { props: props(task(null, "Escrita à mão")) });

    await screen.findByDisplayValue("Escrita à mão");
    await new Promise((r) => setTimeout(r, 20));
    expect(invoke).not.toHaveBeenCalled();
  });

  test("there is no save button to forget", async () => {
    bridge({ set_task_fields: null });

    render(TaskInspector, { props: props(task("a1", "Comprar leite")) });

    await screen.findByDisplayValue("Comprar leite");
    expect(screen.queryByText("Salvar")).toBeNull();
  });

  test("editing saves on its own", async () => {
    bridge({ set_task_fields: null });

    render(TaskInspector, { props: props(task("a1", "Comprar leite")) });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    await waitFor(() => expect(lastSave().fields.tags).toEqual(["casa"]));
    expect(lastSave().id).toBe("a1");
  });

  test("the first edit is what earns the id", async () => {
    bridge({
      list_tasks: [task(null, "Escrita à mão")],
      ensure_task_id: "new1",
      set_task_fields: null,
    });

    render(TaskInspector, { props: props(task(null, "Escrita à mão")) });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("ensure_task_id", {
        list: "Compras",
        position: 0,
      }),
    );
    expect(lastSave().id).toBe("new1");
  });

  test("a second edit does not ask for a second id", async () => {
    // The screen above still holds the id-less copy it selected. Looking the
    // position up again would find nothing — the task has an id by now — and
    // the second write would fail with taskNotFound.
    bridge({
      list_tasks: [task(null, "Escrita à mão")],
      ensure_task_id: "new1",
      set_task_fields: null,
    });

    render(TaskInspector, { props: props(task(null, "Escrita à mão")) });
    const tags = await screen.findByPlaceholderText("Nova tag…");

    await userEvent.type(tags, "casa{enter}");
    await waitFor(() => expect(saveCount()).toBe(1));
    await userEvent.type(tags, "obra{enter}");
    await waitFor(() => expect(saveCount()).toBe(2));

    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ensure_task_id").length).toBe(1);
  });

  test("clearing the date sends null, not nothing", async () => {
    // Absent means "leave alone" on the Rust side, so an emptied field has to
    // travel as an explicit null or the date could never be removed.
    bridge({ set_task_fields: null });

    render(TaskInspector, {
      props: props(task("a1", "Comprar leite", { due: "2026-07-25" })),
    });

    await userEvent.clear(await screen.findByLabelText("Data"));

    await waitFor(() => expect(lastSave().fields.due).toBe(null));
  });

  test("a tag with spaces is stored as a single token", async () => {
    // A loose word on the metadata line stops it from being all-tokens, and
    // the next read turns the whole line into a description — losing the
    // date, the priority and the other tags with it.
    bridge({ set_task_fields: null });

    render(TaskInspector, { props: props(task("a1", "Comprar leite")) });
    await userEvent.type(
      await screen.findByPlaceholderText("Nova tag…"),
      "casa nova{enter}",
    );

    await waitFor(() => expect(lastSave().fields.tags).toEqual(["casa-nova"]));
  });

  test("repeat travels in the written form, never as an object", async () => {
    // The bridge hands back { every, unit } but only parses `every-2-weeks`.
    bridge({ set_task_fields: null });

    render(TaskInspector, {
      props: props(
        task("a1", "Regar plantas", { repeat: { every: 2, unit: "week" } }),
      ),
    });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    await waitFor(() => expect(lastSave().fields.repeat).toBe("every-2-weeks"));
  });

  test("a single repetition drops the count", async () => {
    bridge({ set_task_fields: null });

    render(TaskInspector, {
      props: props(
        task("a1", "Regar plantas", { repeat: { every: 1, unit: "day" } }),
      ),
    });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    await waitFor(() => expect(lastSave().fields.repeat).toBe("every-day"));
  });

  test("subtasks survive the round trip", async () => {
    bridge({ set_task_fields: null });

    render(TaskInspector, {
      props: props(
        task("a1", "Obra", { subtasks: [{ text: "Cimento", done: true }] }),
      ),
    });
    await userEvent.type(
      await screen.findByPlaceholderText("Nova subtarefa…"),
      "Areia{enter}",
    );

    await waitFor(() =>
      expect(lastSave().fields.subtasks).toEqual([
        { text: "Cimento", done: true },
        { text: "Areia", done: false },
      ]),
    );
  });

  test("selecting another task replaces the draft", async () => {
    bridge({ set_task_fields: null });

    const { rerender } = render(TaskInspector, {
      props: props(task("a1", "Comprar leite")),
    });
    await screen.findByDisplayValue("Comprar leite");

    await rerender(props(task("b2", "Pagar boleto")));

    expect(await screen.findByDisplayValue("Pagar boleto")).toBeTruthy();
    expect(screen.queryByDisplayValue("Comprar leite")).toBeNull();
  });

  test("switching task mid-edit still saves what was typed, to the right task", async () => {
    // The dangerous moment of an auto-save: a pending write belongs to the
    // task it was typed into, not to whatever is on screen when it lands.
    bridge({ set_task_fields: null });

    const { rerender } = render(TaskInspector, {
      props: props(task("a1", "Comprar leite"), { saveDelay: 10_000 }),
    });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    await rerender(props(task("b2", "Pagar boleto"), { saveDelay: 10_000 }));

    await waitFor(() => expect(saveCount()).toBe(1));
    expect(lastSave().id).toBe("a1");
    expect(lastSave().fields.tags).toEqual(["casa"]);
  });

  test("closing with an edit pending still saves it", async () => {
    bridge({ set_task_fields: null });

    const { unmount } = render(TaskInspector, {
      props: props(task("a1", "Comprar leite"), { saveDelay: 10_000 }),
    });
    await userEvent.type(await screen.findByPlaceholderText("Nova tag…"), "casa{enter}");

    unmount();

    await waitFor(() => expect(saveCount()).toBe(1));
    expect(lastSave().fields.tags).toEqual(["casa"]);
  });

  test("a failed write is retried by the next edit, not counted as saved", async () => {
    let attempts = 0;
    const errors = [];
    bridge({
      set_task_fields: () => {
        attempts += 1;
        return attempts === 1
          ? Promise.reject({ kind: "io", message: "disco cheio" })
          : Promise.resolve(null);
      },
    });

    render(TaskInspector, {
      props: props(task("a1", "Comprar leite"), {
        onError: (e) => errors.push(e),
      }),
    });
    const tags = await screen.findByPlaceholderText("Nova tag…");

    await userEvent.type(tags, "casa{enter}");
    await waitFor(() => expect(errors.length).toBe(1));

    await userEvent.type(tags, "obra{enter}");
    // The retry carries the tag the failed write was meant to persist.
    await waitFor(() => expect(lastSave().fields.tags).toEqual(["casa", "obra"]));
  });

  test("a read-only notebook cannot be edited at all", async () => {
    bridge({ set_task_fields: null });

    render(TaskInspector, {
      props: props(task("a1", "Comprar leite"), { readOnly: true }),
    });

    await screen.findByDisplayValue("Comprar leite");
    expect(screen.queryByPlaceholderText("Nova tag…")).toBeNull();
    expect(screen.getByLabelText("nome da tarefa").disabled).toBe(true);
    expect(saveCount()).toBe(0);
  });
});
