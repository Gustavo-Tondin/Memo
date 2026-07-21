// Since phase 7 a list is addressed by its root-relative path
// (`Tasks/Compras.md`). The address is what every command takes; the name is
// what the user reads. This is the one place that knows how to go from one
// to the other.

/// Display name of a list address: `Tasks/Compras.md` → `Compras`.
export function listName(path) {
  return (path ?? "").split("/").pop().replace(/\.md$/, "");
}
