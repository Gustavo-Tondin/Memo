<script>
  // A user workspace: its widgets, in the order the config declares.
  //
  // This screen knows nothing about widget types — it asks the registry for
  // each component and renders whatever comes back. That ignorance is the
  // point: a new type reaches the screen by touching `widgets.js`, and a type
  // nobody implemented degrades to the unsupported card without this file
  // ever hearing about it.
  import { S } from "./strings.js";
  import { widgetComponent } from "./widgets.js";

  let {
    workspace,
    lists = [],
    counts = {},
    completedName = "Completed",
    notesInbox = "Inbox",
    readOnly = false,
    reloadKey = 0,
    onOpenList,
    onOpenNote,
    onChanged,
    onError,
  } = $props();
</script>

<h2 class="workspace-view__title">
  {workspace.name}
  {#if workspace.readOnly}<small class="workspace-view__badge"
      >{S.readOnlyWorkspace}</small
    >{/if}
</h2>

{#if workspace.widgets.length === 0}
  <p class="workspace-view__empty">{S.emptyWorkspace}</p>
{:else}
  {#each workspace.widgets as widget, i (i)}
    {@const Widget = widgetComponent(widget.kind)}
    <Widget
      {widget}
      {lists}
      {counts}
      {completedName}
      {notesInbox}
      {readOnly}
      {reloadKey}
      {onOpenList}
      {onOpenNote}
      {onChanged}
      {onError}
    />
  {/each}
{/if}

<style>
  .workspace-view__badge {
    color: #b00;
    font-weight: normal;
    font-size: 0.8rem;
    margin-left: 0.5rem;
  }
  .workspace-view__empty {
    color: #666;
  }
</style>
