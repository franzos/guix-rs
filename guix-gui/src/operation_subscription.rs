//! Wraps [`libguix::Operation`] as a stable-id `iced::Subscription`.
//! Iced dedupes by hash — the id must stay constant across `view()` calls.
//! Must hold the WHOLE `Operation`, not just the stream — see NOTES.md.

use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::stream::{self, StreamExt};
use iced::Subscription;
use libguix::{Operation, ProgressEvent};

/// `Hash` contributes to the subscription id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpKind {
    Install,
    Remove,
    Upgrade,
    Pull,
    SystemPull,
    Reconfigure,
}

impl OpKind {
    pub fn label(self) -> String {
        match self {
            OpKind::Install => crate::t!("op-install"),
            OpKind::Remove => crate::t!("op-remove"),
            OpKind::Upgrade => crate::t!("op-upgrade"),
            OpKind::Pull => crate::t!("op-pull"),
            OpKind::SystemPull => crate::t!("op-system-pull"),
            OpKind::Reconfigure => crate::t!("op-reconfigure"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpId(pub u64);

impl OpId {
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub enum OpEvent {
    Progress(Vec<ProgressEvent>),
    Finished,
}

/// Test mirror of Iced's `#[derive(Hash)]` recipe inside `operation_subscription`.
#[must_use]
#[allow(dead_code)]
pub fn subscription_hash(kind: OpKind, id: OpId) -> u64 {
    let mut h = DefaultHasher::new();
    "guix-gui::operation".hash(&mut h);
    kind.hash(&mut h);
    id.hash(&mut h);
    h.finish()
}

/// `Arc<Mutex<Option<Operation>>>` with manual `Debug` so it can ride
/// Iced's `Message` bus (requires `Debug + Clone + Send + 'static`).
pub struct SharedOp {
    inner: Arc<Mutex<Option<Operation>>>,
}

impl SharedOp {
    pub fn new(op: Operation) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(op))),
        }
    }

    pub fn take(&self) -> Option<Operation> {
        self.inner.lock().ok().and_then(|mut g| g.take())
    }

    #[cfg(test)]
    pub fn new_empty_for_tests() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

impl Clone for SharedOp {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl fmt::Debug for SharedOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let present = self.inner.lock().map(|g| g.is_some()).unwrap_or(false);
        write!(f, "SharedOp(present={present})")
    }
}

/// iced 0.14 replaced `run_with_id(id, stream)` with `run_with(data, fn(&data) -> stream)`.
/// `data` must be `Hash` — we hash only (kind, id) for stable dedupe and skip the slot.
struct SubData {
    kind: OpKind,
    id: OpId,
    slot: SharedOp,
}

impl Hash for SubData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.id.hash(state);
    }
}

fn build_stream(data: &SubData) -> impl futures_util::Stream<Item = OpEvent> {
    let slot = data.slot.clone();
    stream::unfold(
        // State: (taken-yet, op kept alive for its DropGuard, finished flag).
        (false, None::<libguix::Operation>, false),
        move |(taken, mut op, finished)| {
            let slot = slot.clone();
            async move {
                if finished {
                    return None;
                }
                if !taken {
                    op = slot.take();
                }
                let event = match op.as_mut() {
                    Some(o) => o.events_mut().next().await,
                    None => None,
                };
                match event {
                    Some(batch) => Some((OpEvent::Progress(batch), (true, op, false))),
                    None => Some((OpEvent::Finished, (true, op, true))),
                }
            }
        },
    )
}

pub fn operation_subscription(kind: OpKind, id: OpId, slot: SharedOp) -> Subscription<OpEvent> {
    Subscription::run_with(SubData { kind, id, slot }, build_stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_hash_is_stable_and_distinguishes_ids() {
        let id_a = OpId(1);
        let id_b = OpId(2);

        let h1 = subscription_hash(OpKind::Install, id_a);
        let h2 = subscription_hash(OpKind::Install, id_a);
        assert_eq!(h1, h2, "same (kind, id) hashes equal twice in a row");

        let h3 = subscription_hash(OpKind::Install, id_b);
        assert_ne!(h1, h3, "different ids produce different hashes");

        let h4 = subscription_hash(OpKind::Remove, id_a);
        assert_ne!(h1, h4, "different kinds produce different hashes");
    }

    #[test]
    fn op_id_is_monotonic() {
        let a = OpId::next();
        let b = OpId::next();
        let c = OpId::next();
        assert!(b.0 > a.0);
        assert!(c.0 > b.0);
    }
}
