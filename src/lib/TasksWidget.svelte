<script>
  // The `tasks` widget: the lists living in one folder, with their counts.
  //
  // It receives the notebook-wide lists and filters down to its own folder —
  // the shell already has them in the snapshot, and a widget fetching its own
  // data would mean N invokes per render once a workspace has several.
  import { S } from "./strings.js";
  import { listName } from "./paths.js";

  let {
    widget,
    lists = [],
    counts = {},
    completedName = "Completed",
    onOpenList,
  } = $props();

  // Direct children only: `Project A/Backlog/Inbox.md` belongs to the widget
  // at `Project A/Backlog`, but not to one at `Project A` — the nested widget
  // owns its subtree (spec 3.5).
  let own = $derived(
    !widget.folder
      ? []
      : lists.filter((entry) => {
          const prefix = `${widget.folder}/`;
          return (
            entry.path.startsWith(prefix) &&
            !entry.path.slice(prefix.length).includes("/") &&
            // Each tasks folder has its own Completed; it gets the dedicated
            // aggregated screen (phase 8.5), not a slot among the lists.
            entry.name !== completedName
          );
        }),
  );
</script>

<section class="tasks-widget">
  {#if widget.invalidFolder}
    <p class="tasks-widget__note tasks-widget__note--warn">
      {S.invalidWidgetFolder}
    </p>
  {:else if own.length === 0}
    <p class="tasks-widget__note">{S.widgetNoLists}</p>
  {:else}
    <ul class="tasks-widget__list">
      {#each own as entry (entry.path)}
        <li class="tasks-widget__item">
          <button
            class="tasks-widget__link"
            onclick={() => onOpenList?.(entry.path)}
          >
            {listName(entry.path)}
            {#if counts[entry.path]}<span class="tasks-widget__count"
                >{counts[entry.path]}</span
              >{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .tasks-widget__list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .tasks-widget__link {
    background: none;
    border: none;
    font: inherit;
    cursor: pointer;
    padding: 0.25rem 0.35rem;
    border-radius: 4px;
    width: 100%;
    text-align: left;
  }
  .tasks-widget__link:hover {
    background: #eee;
  }
  .tasks-widget__count {
    float: right;
    color: #666;
    font-size: 0.85rem;
  }
  .tasks-widget__note {
    color: #666;
    font-size: 0.9rem;
  }
  .tasks-widget__note--warn {
    color: #a00;
  }
</style>
