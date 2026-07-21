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
    onOpenList,
  } = $props();
</script>

<h2>
  {workspace.name}
  {#if workspace.readOnly}<small class="ro">{S.readOnlyWorkspace}</small>{/if}
</h2>

{#if workspace.widgets.length === 0}
  <p class="empty">{S.emptyWorkspace}</p>
{:else}
  {#each workspace.widgets as widget, i (i)}
    {@const Widget = widgetComponent(widget.kind)}
    <Widget {widget} {lists} {counts} {completedName} {onOpenList} />
  {/each}
{/if}

<style>
  .ro {
    color: #b00;
    font-weight: normal;
    font-size: 0.8rem;
    margin-left: 0.5rem;
  }
  .empty {
    color: #666;
  }
</style>
