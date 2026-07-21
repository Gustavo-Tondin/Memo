// Showing a date.
//
// The file always stores ISO — that is the format decision, and it does not
// change. This is only how the date is *drawn*, following the notebook's
// `dateDisplayFormat`. Kept in one place so every screen shows the same date
// the same way.

/// Formats an ISO date (`2026-07-25`) in the notebook's chosen shape.
///
/// An unknown pattern falls back to the default rather than showing the date
/// wrong — the core validates the same way, so this is belt and braces.
export function formatDate(iso, pattern = "dd-mm-yyyy") {
  if (!iso) return "";
  const [y, m, d] = iso.split("-");
  if (!y || !m || !d) return iso;

  switch (pattern) {
    case "yyyy-mm-dd":
      return `${y}-${m}-${d}`;
    case "mm/dd/yyyy":
      return `${m}/${d}/${y}`;
    case "dd/mm/yyyy":
      return `${d}/${m}/${y}`;
    default:
      return `${d}-${m}-${y}`;
  }
}
