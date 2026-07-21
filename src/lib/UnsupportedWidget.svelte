<script>
  // The polite fallback of the widget registry: a widget this build cannot
  // render is shown and named, never hidden and never touched. Erasing or
  // "fixing" it would destroy data written by a newer version or a community
  // template — the exact failure spec 3.5 forbids.
  import { S } from "./strings.js";

  let { widget } = $props();
</script>

<section class="unsupported">
  <strong>{S.unsupportedWidgetTitle(widget.kind)}</strong>
  <p>
    {#if widget.invalidFolder}
      {S.invalidWidgetFolder}
    {:else}
      {S.unsupportedWidgetBody}
    {/if}
  </p>
</section>

<style>
  .unsupported {
    border: 1px dashed #bbb;
    border-radius: 6px;
    padding: 0.6rem 0.8rem;
    color: #666;
    font-size: 0.9rem;
  }
  .unsupported p {
    margin: 0.25rem 0 0;
  }
</style>
