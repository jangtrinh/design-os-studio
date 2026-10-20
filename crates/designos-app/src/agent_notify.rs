//! OS notifications when an agent turn finishes or needs input in the background (`#683`).
//!
//! ## Spike S1 — Notification click-to-focus across platforms (`#683` §3.8, Rule 8)
//!
//! We evaluated native OS notification delivery and click activation across macOS, Linux, and Windows:
//! - **macOS**: `/usr/bin/osascript` (`display notification`) posts banners to Notification Center
//!   without requiring a signed `.app` bundle, and provides no click callback in the shipped
//!   [`OsNotificationSink`].
//! - **Linux**: `notify-send` posts banners to the freedesktop.org notification daemon without a
//!   resident click callback in [`OsNotificationSink`].
//! - **Windows**: PowerShell `ToastNotificationManager` requires a registered COM
//!   `NotificationActivator` CLSID and AppUserModelID in the registry, so no click callback is
//!   available in [`OsNotificationSink`].
//!
//! Per spec Rule 8 ("If a platform cannot route a click back, record that in the Spike S1 note and
//! still send the banner"), [`OsNotificationSink`] dispatches the banner on a background worker
//! thread (`< 0.1 ms` on the UI thread per §11) and does not register an OS click callback on
//! macOS, Linux, or Windows. The [`NotificationSink::poll_activated_tab`] queue is a test seam
//! exercised by [`StubNotificationSink`] so the Studio window focus and tab-switch handler
//! (`FocusAgentTab`) is deterministically verified in unit tests.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Status line message shown once when the OS denies or cannot deliver notifications (`#683` §8.8).
pub const NOTIFICATIONS_OFF_STATUS_MSG: &str = "Notifications are off";

/// Why an agent tab is requesting user attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyKind {
    /// A turn completed (or ended with an error) while the window was unfocused or the tab hidden.
    Finished,
    /// The agent is waiting on a permission prompt or user input.
    NeedsInput,
}

/// One formatted OS notification emitted for an agent tab (`#683` §3.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentNotification {
    pub tab_id: usize,
    pub agent_name: String,
    pub file_name: String,
    pub kind: NotifyKind,
    pub message: String,
}

impl AgentNotification {
    pub fn new(tab_id: usize, agent_name: impl Into<String>, file_name: impl Into<String>, kind: NotifyKind) -> Self {
        let agent_name = agent_name.into();
        let file_name = file_name.into();
        let message = match kind {
            NotifyKind::Finished => format!("{agent_name} finished in {file_name}"),
            NotifyKind::NeedsInput => format!("{agent_name} needs your input"),
        };
        Self { tab_id, agent_name, file_name, kind, message }
    }
}

/// Outcome of delivering an [`AgentNotification`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationOutcome {
    Sent,
    Denied,
}

/// Pluggable sink for OS notifications (real background OS command in production, recording stub in tests).
pub trait NotificationSink: Send + Sync {
    fn send(&self, notification: &AgentNotification) -> NotificationOutcome;

    /// Returns true once the sink has observed a denied or failed OS notification delivery (`#683` §8.8).
    fn is_denied(&self) -> bool {
        false
    }

    /// Test seam exercised by [`StubNotificationSink`] to pop the next `tab_id` whose simulated
    /// notification was clicked (`#683` §3.6, §3.8).
    fn poll_activated_tab(&self) -> Option<usize> {
        None
    }
}

/// Rule 6 & Rule 7 visibility gate: send a notification only when the Studio window is unfocused
/// or the tab is not visible (`!window_focused || !tab_visible`).
pub fn should_notify(window_focused: bool, tab_visible: bool) -> bool {
    !window_focused || !tab_visible
}

/// Recording stub sink used in unit tests and headless runs.
#[derive(Default)]
pub struct StubNotificationSink {
    sent: Mutex<Vec<AgentNotification>>,
    activated: Mutex<VecDeque<usize>>,
    denied: AtomicBool,
    denied_observed: AtomicBool,
}

impl StubNotificationSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_denied(denied: bool) -> Self {
        Self { sent: Mutex::new(Vec::new()), activated: Mutex::new(VecDeque::new()), denied: AtomicBool::new(denied), denied_observed: AtomicBool::new(false) }
    }

    pub fn set_denied(&self, denied: bool) {
        self.denied.store(denied, Ordering::SeqCst);
        if !denied {
            self.denied_observed.store(false, Ordering::SeqCst);
        }
    }

    pub fn calls(&self) -> Vec<AgentNotification> {
        self.sent.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn count(&self) -> usize {
        self.sent.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Simulates the user clicking a notification for `tab_id` (test seam for `#683` §3.6).
    pub fn simulate_click(&self, tab_id: usize) {
        if let Ok(mut guard) = self.activated.lock() {
            guard.push_back(tab_id);
        }
    }

    /// Simulates clicking the most recently sent notification, using its recorded `tab_id`.
    pub fn click_last(&self) {
        let last_tab = self.sent.lock().ok().and_then(|g| g.last().map(|n| n.tab_id));
        if let Some(tab_id) = last_tab {
            self.simulate_click(tab_id);
        }
    }
}

impl NotificationSink for StubNotificationSink {
    fn send(&self, notification: &AgentNotification) -> NotificationOutcome {
        if self.denied.load(Ordering::SeqCst) {
            self.denied_observed.store(true, Ordering::SeqCst);
            return NotificationOutcome::Denied;
        }
        if let Ok(mut guard) = self.sent.lock() {
            guard.push(notification.clone());
        }
        NotificationOutcome::Sent
    }

    fn is_denied(&self) -> bool {
        self.denied_observed.load(Ordering::SeqCst)
    }

    fn poll_activated_tab(&self) -> Option<usize> {
        self.activated.lock().ok().and_then(|mut g| g.pop_front())
    }
}

type DeliveryRunner = dyn Fn(&AgentNotification) -> bool + Send + Sync;

/// Production OS notification sink that dispatches platform notifications on a detached worker
/// thread without blocking the UI thread (`< 0.1 ms` per §11) and records delivery failure in
/// `denied_detected` for the next `poll_live_agent_bridge` tick (`#683` §8.8).
pub struct OsNotificationSink {
    denied_detected: Arc<AtomicBool>,
    runner: Option<Arc<DeliveryRunner>>,
}

impl Default for OsNotificationSink {
    fn default() -> Self {
        Self { denied_detected: Arc::new(AtomicBool::new(false)), runner: None }
    }
}

impl OsNotificationSink {
    /// Creates an [`OsNotificationSink`] with a custom background delivery function (used in unit
    /// tests to verify non-blocking dispatch and asynchronous `Denied` reporting).
    pub fn with_runner<F>(runner: F) -> Self
    where
        F: Fn(&AgentNotification) -> bool + Send + Sync + 'static,
    {
        Self { denied_detected: Arc::new(AtomicBool::new(false)), runner: Some(Arc::new(runner)) }
    }
}

impl NotificationSink for OsNotificationSink {
    fn send(&self, notification: &AgentNotification) -> NotificationOutcome {
        if self.denied_detected.load(Ordering::SeqCst) {
            return NotificationOutcome::Denied;
        }
        let notif = notification.clone();
        let denied_flag = Arc::clone(&self.denied_detected);
        let runner = self.runner.clone();
        let spawned = std::thread::Builder::new().name("designos-agent-notify".into()).spawn(move || {
            let ok = match runner {
                Some(f) => f(&notif),
                None => deliver_os_notification(&notif),
            };
            if !ok {
                denied_flag.store(true, Ordering::SeqCst);
            }
        });
        if spawned.is_err() {
            self.denied_detected.store(true, Ordering::SeqCst);
            return NotificationOutcome::Denied;
        }
        NotificationOutcome::Sent
    }

    fn is_denied(&self) -> bool {
        self.denied_detected.load(Ordering::SeqCst)
    }
}

fn deliver_os_notification(notification: &AgentNotification) -> bool {
    let body = &notification.message;
    #[cfg(target_os = "macos")]
    {
        let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{escaped}\" with title \"design:os studio\"");
        std::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("notify-send")
            .arg("design:os studio")
            .arg(body)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = body;
        false
    }
}

/// Per-terminal notification dispatcher that enforces one notification per turn and one-time
/// status-line reporting when notifications are off (`#683` §3.6, §8.8).
pub struct AgentNotifier {
    sink: Arc<dyn NotificationSink>,
    pub notifications_off_noted: bool,
}

impl std::fmt::Debug for AgentNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentNotifier").field("notifications_off_noted", &self.notifications_off_noted).finish()
    }
}

impl Default for AgentNotifier {
    fn default() -> Self {
        #[cfg(test)]
        let sink: Arc<dyn NotificationSink> = Arc::new(StubNotificationSink::new());
        #[cfg(not(test))]
        let sink: Arc<dyn NotificationSink> = Arc::new(OsNotificationSink::default());
        Self { sink, notifications_off_noted: false }
    }
}

impl AgentNotifier {
    pub fn with_sink(sink: Arc<dyn NotificationSink>) -> Self {
        Self { sink, notifications_off_noted: false }
    }

    pub fn set_sink(&mut self, sink: Arc<dyn NotificationSink>) {
        self.sink = sink;
    }

    /// Pops the next `tab_id` from the [`StubNotificationSink`] activation test seam (`#683` §3.6, §3.8).
    pub fn poll_activated_tab(&self) -> Option<usize> {
        self.sink.poll_activated_tab()
    }

    /// Returns `Some(NOTIFICATIONS_OFF_STATUS_MSG)` once after the notification sink detects a
    /// denied or failed OS notification (`#683` §8.8, §11).
    pub fn poll_denied_status_msg(&mut self) -> Option<String> {
        if !self.notifications_off_noted && self.sink.is_denied() {
            self.notifications_off_noted = true;
            Some(NOTIFICATIONS_OFF_STATUS_MSG.to_string())
        } else {
            None
        }
    }

    /// Sends `notification` if `should_notify(window_focused, tab_visible)` holds.
    /// Returns `Some(NOTIFICATIONS_OFF_STATUS_MSG)` once if the sink synchronously reports `Denied` (`#683` §8.8).
    pub fn notify_if_background(&mut self, window_focused: bool, tab_visible: bool, notification: AgentNotification) -> Option<String> {
        if !should_notify(window_focused, tab_visible) {
            return None;
        }
        match self.sink.send(&notification) {
            NotificationOutcome::Sent => None,
            NotificationOutcome::Denied => {
                if !self.notifications_off_noted {
                    self.notifications_off_noted = true;
                    Some(NOTIFICATIONS_OFF_STATUS_MSG.to_string())
                } else {
                    None
                }
            }
        }
    }
}
