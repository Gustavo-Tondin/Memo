<script>
  // The fixed Tasks workspace: the inbox, plus the day and the week.
  //
  // One entry in the sidebar and three buttons inside it, rather than three
  // entries — Today and This Week are *views of the same tasks*, not places
  // of their own, and the sidebar should say so.
  //
  // The choice is state of this screen, not a view of its own: switching
  // between the three is looking around inside one document, so it must not
  // open a tab or fill the back history.
  import { S } from "./strings.js";
  import ListView from "./ListView.svelte";
  import PeriodView from "./PeriodView.svelte";

  let {
    inbox,
    clock,
    readOnly = false,
    reloadKey = 0,
    onChanged,
    onError,
    onSelect,
    selectedId = null,
  } = $props();

  const SUBS = [
    { key: "inbox", label: S.inboxTab },
    { key: "day", label: S.today },
    { key: "week", label: S.week },
  ];

  let sub = $state("inbox");
</script>

<nav class="subs">
  {#each SUBS as item (item.key)}
    <button class:active={sub === item.key} onclick={() => (sub = item.key)}>
      {item.label}
    </button>
  {/each}
</nav>

{#if sub === "inbox"}
  <ListView
    list={inbox}
    {readOnly}
    {reloadKey}
    {onChanged}
    {onError}
    {onSelect}
    {selectedId}
  />
{:else}
  <PeriodView
    period={sub}
    {clock}
    {readOnly}
    {reloadKey}
    {onChanged}
    {onError}
    {onSelect}
    {selectedId}
  />
{/if}

<style>
  .subs {
    display: flex;
    gap: 0.35rem;
    margin-bottom: 0.75rem;
  }
  .subs button {
    background: none;
    border: 1px solid #ddd;
    border-radius: 14px;
    padding: 0.2rem 0.8rem;
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .subs button.active {
    background: #eef2ff;
    border-color: #b9c6f5;
    font-weight: 600;
  }
</style>
