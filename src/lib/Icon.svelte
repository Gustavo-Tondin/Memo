<script>
  // Draws a vendored Phosphor SVG by name. The single door to icons in the UI —
  // the rest of the app says `<Icon name="..." />` and never touches raw SVG.
  import { ICONS } from "./icons.js";

  let { name, size = "1em", label = null } = $props();

  let svg = $derived(ICONS[name] ?? "");
</script>

<span
  class="memo-icon"
  style="--icon-size:{size}"
  role={label ? "img" : "presentation"}
  aria-label={label}
  aria-hidden={label ? null : "true"}
>
  {@html svg}
</span>

<style>
  .memo-icon {
    display: inline-flex;
    width: var(--icon-size);
    height: var(--icon-size);
    line-height: 0;
    flex-shrink: 0;
  }
  .memo-icon :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
