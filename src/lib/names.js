// Names the core creates, mirrored for the screens.
//
// These are file names on disk, not labels: they are in English because the
// notebook layout is (phase 5), while what the user reads stays translated.
// Keeping them here means a rename in `core/src/lib.rs` has exactly one place
// to be followed on this side — the last time it did not, the completed
// screen silently read a file that no longer existed.

export const INBOX_LIST = "Inbox";
export const COMPLETED_LIST = "Completed";
