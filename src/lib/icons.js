// Raw SVG strings for the icons the UI renders today.
//
// Explicit imports (not a glob) keep the bundle tree-shaken: only the handful
// the interface actually draws is compiled in. The FULL Phosphor set lives on
// disk under `src/assets/icons/phosphor/` — vendored for the future per-space
// icon picker (roadmap, Fase 13), where it will be read lazily, not bundled.
//
// `?raw` gives the file's text; each SVG uses `fill="currentColor"`, so an icon
// takes the colour of whatever renders it.

import xBold from "../assets/icons/phosphor/bold/x.svg?raw";
import plus from "../assets/icons/phosphor/regular/plus.svg?raw";

export const ICONS = {
  "x-bold": xBold,
  plus,
};
