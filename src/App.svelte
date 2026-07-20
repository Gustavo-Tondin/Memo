<script>
  import { invoke } from "@tauri-apps/api/core";

  // Phase 0 smoke test: proves the frontend can reach the Rust core.
  // Replaced by real screens in Phase 4.
  let coreVersion = $state("...");

  invoke("core_version")
    .then((v) => {
      console.log(`[memo] core_version ok: ${v}`);
      coreVersion = v;
    })
    .catch((err) => {
      console.error("[memo] core_version failed", err);
      coreVersion = "unavailable";
    });
</script>

<main>
  <h1>Memo</h1>
  <p>core {coreVersion}</p>
</main>
