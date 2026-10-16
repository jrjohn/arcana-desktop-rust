//! Runs page tasks off the UI thread and brings their results back as inputs.
//!
//! Tasks run on a tokio runtime (owned by the app); each finished task sends
//! `(tab, PageMsg)` over a channel that the shell drains every frame, then asks egui to
//! repaint. `in_flight` lets tests (and the screenshot tool) wait until the UI is idle.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;

use crate::pages::PageMsg;

/// Stable identity of a document tab — the address a task result is delivered to. Owned here
/// (not by the shell view) so the runtime does not depend on the view layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u64);

/// A finished task's result addressed to the tab that asked for it.
pub type Envelope = (TabId, PageMsg);

/// Task executor + result channel.
pub struct Executor {
    handle: Handle,
    tx: Sender<Envelope>,
    rx: Receiver<Envelope>,
    in_flight: Arc<AtomicUsize>,
    repaint: Arc<Mutex<Option<egui::Context>>>,
}

impl std::fmt::Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Executor")
            .field("in_flight", &self.in_flight())
            .finish_non_exhaustive()
    }
}

impl Executor {
    #[must_use]
    pub fn new(handle: Handle) -> Self {
        let (tx, rx) = channel();
        Self {
            handle,
            tx,
            rx,
            in_flight: Arc::new(AtomicUsize::new(0)),
            repaint: Arc::new(Mutex::new(None)),
        }
    }

    /// Remember the egui context so finished tasks can wake the UI.
    pub fn set_repaint_context(&self, ctx: &egui::Context) {
        let mut slot = self.repaint.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(ctx.clone());
        }
    }

    /// Run `fut` on the runtime; its output is delivered to `tab`.
    pub fn spawn(&self, tab: TabId, fut: impl Future<Output = PageMsg> + Send + 'static) {
        let tx = self.tx.clone();
        let in_flight = Arc::clone(&self.in_flight);
        let repaint = Arc::clone(&self.repaint);
        in_flight.fetch_add(1, Ordering::SeqCst);
        self.handle.spawn(async move {
            let msg = fut.await;
            // The receiver lives as long as the shell; a send error only means shutdown.
            let _ = tx.send((tab, msg));
            in_flight.fetch_sub(1, Ordering::SeqCst);
            if let Some(ctx) = repaint
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
            {
                ctx.request_repaint();
            }
        });
    }

    /// Tasks started but whose result has not been sent yet.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Results ready right now (non-blocking).
    #[must_use]
    pub fn drain(&self) -> Vec<Envelope> {
        self.rx.try_iter().collect()
    }

    /// Wait up to `timeout` for the next result.
    #[must_use]
    pub fn wait_next(&self, timeout: Duration) -> Option<Envelope> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Wait until nothing is in flight (or `timeout`), returning every result received.
    #[must_use]
    pub fn wait_idle(&self, timeout: Duration) -> Vec<Envelope> {
        let deadline = Instant::now() + timeout;
        let mut out = self.drain();
        while self.in_flight() > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            if let Some(e) = self.wait_next(left.min(Duration::from_millis(50))) {
                out.push(e);
            }
        }
        out.extend(self.drain());
        out
    }
}
