//! One-shot wrapper so non-`Debug`/`Clone` types from `libguix` can
//! ride Iced's `Message: Debug + Clone + Send + 'static` bus.

use std::fmt;
use std::sync::{Arc, Mutex};

pub struct Carrier<T> {
    inner: Arc<Mutex<Option<T>>>,
}

impl<T> Carrier<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(value))),
        }
    }

    pub fn empty() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn take(&self) -> Option<T> {
        self.inner.lock().ok().and_then(|mut g| g.take())
    }
}

impl<T> Clone for Carrier<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for Carrier<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Carrier<{}>(present={})",
            std::any::type_name::<T>(),
            self.inner.lock().map(|g| g.is_some()).unwrap_or(false)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_yields_value_once() {
        let c = Carrier::new(String::from("hello"));
        assert_eq!(c.take().as_deref(), Some("hello"));
        assert!(c.take().is_none());
    }

    #[test]
    fn clone_shares_state() {
        let a = Carrier::new(42u32);
        let b = a.clone();
        assert_eq!(a.take(), Some(42));
        assert!(b.take().is_none(), "clones share the inner slot");
    }
}
