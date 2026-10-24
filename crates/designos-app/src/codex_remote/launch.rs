//! The command that starts the remote Codex TUI (#930).
//!
//! The tab runs `codex --remote unix://<rendezvous>` once the socket exists. If it does not appear within five
//! seconds, the wrapper runs today's embedded launch instead, so a server that never came up leaves the tab as it was.
//! The wrapper starts with `exec ` so the agent shell adds no second one, and the outer login shell expands today's
//! command into the wrapper's `"$@"`: the fallback is byte for byte today's launch.

/// Environment names the wrapper reads. They are set for Ask-mode tabs on the card path and removed for every other tab.
pub const ENV_CODEX_BIN: &str = "DESIGNOS_CODEX_BIN";
pub const ENV_CODEX_SOCKET: &str = "DESIGNOS_CODEX_SOCKET";

/// 100 checks, 50 ms apart.
pub const SOCKET_WAIT_TICKS: u32 = 100;

/// The wrapper for production: waits five seconds for the socket.
pub fn tui_launch_command(fallback: &str) -> String {
    tui_launch_command_with_ticks(fallback, SOCKET_WAIT_TICKS)
}

/// The wrapper with a chosen wait of `ticks` checks. `fallback` is today's launch command, appended as the words the
/// wrapper falls back to.
pub fn tui_launch_command_with_ticks(fallback: &str, ticks: u32) -> String {
    format!(
        "exec /bin/sh -c 'i=0; while [ ! -S \"${ENV_CODEX_SOCKET}\" ] && [ \"$i\" -lt {ticks} ]; do sleep 0.05; i=$((i+1)); done; \
         [ -S \"${ENV_CODEX_SOCKET}\" ] && exec \"${ENV_CODEX_BIN}\" --remote \"unix://${ENV_CODEX_SOCKET}\"; exec \"$@\"' designos-codex-tui {fallback}"
    )
}
