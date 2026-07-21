<script>
  // The CodeMirror instance, wrapped so the rest of the app never imports it.
  //
  // Everything above this file talks in `value` and `onChange`, the way it
  // did when this was a `<textarea>` — which is what let the note editor keep
  // its auto-save untouched when the engine changed underneath.
  import { onDestroy, onMount } from "svelte";
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap, placeholder as placeholderExt } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
  import { markdownPreview } from "./markdown.js";

  let { value = "", readOnly = false, placeholder = "", onChange } = $props();

  let host;
  let view = null;
  /// What the editor itself last produced, so an echo of our own change does
  /// not get pushed back in and move the cursor.
  let lastEmitted = null;

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          // `markdownLanguage` rather than the commonmark default: it is the
          // one that understands task lists and strikethrough, which a note
          // of the day uses constantly.
          //
          // No `codeLanguages`: highlighting a fenced block per language
          // would drag in ~110 parsers for an app whose notes are the small
          // ones of the day (spec 5, principle 4). A code block still reads
          // as code — monospace, set apart — it just is not colourised.
          markdown({ base: markdownLanguage }),
          markdownPreview,
          EditorView.lineWrapping,
          placeholderExt(placeholder),
          EditorState.readOnly.of(readOnly),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            lastEmitted = update.state.doc.toString();
            onChange?.(lastEmitted);
          }),
        ],
      }),
    });
  });

  onDestroy(() => view?.destroy());

  // A value that arrives from outside (another note opened, the file reloaded
  // from disk) replaces the document. Our own echo is ignored: re-setting it
  // would throw the cursor to the start mid-typing.
  $effect(() => {
    const incoming = value;
    if (!view || incoming === lastEmitted) return;
    if (incoming === view.state.doc.toString()) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: incoming },
    });
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
  }
  /* CodeMirror renders into our subtree, so these need :global. */
  .editor :global(.cm-editor) {
    height: 100%;
    font: inherit;
  }
  .editor :global(.cm-editor.cm-focused) {
    outline: none;
  }
  .editor :global(.cm-content) {
    padding: 0.5rem 0;
    line-height: 1.6;
    caret-color: #222;
  }
  .editor :global(.cm-line) {
    padding: 0 0.2rem;
  }
  .editor :global(.cm-task-checkbox) {
    margin-right: 0.35rem;
    vertical-align: middle;
  }
  .editor :global(.cm-placeholder) {
    color: #999;
  }
</style>
