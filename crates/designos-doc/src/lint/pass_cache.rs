//! Scratch space that lives for one lint pass over one document.
//!
//! A [`super::LintEnv`] that needs the same expensive structure for many nodes (a spatial index of
//! the page, say) keeps it here. The pass owns the cache and drops it when the pass ends, so the
//! structure can never outlive the document revision it was built from, and the environment itself
//! stays stateless and shareable.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// One value per type, created on first use.
#[derive(Default)]
pub struct PassCache {
    slots: RefCell<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl PassCache {
    /// The cached `T`, built with `init` on first use. `init` may use this cache itself.
    pub fn get_or_insert_with<T: Any + Send + Sync>(&self, init: impl FnOnce() -> T) -> Arc<T> {
        let key = TypeId::of::<T>();
        if let Some(found) = self.slots.borrow().get(&key) {
            if let Ok(found) = Arc::clone(found).downcast::<T>() {
                return found;
            }
        }
        let made = Arc::new(init());
        self.slots.borrow_mut().insert(key, Arc::clone(&made) as Arc<dyn Any + Send + Sync>);
        made
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_each_type_once() {
        let cache = PassCache::default();
        let mut built = 0;
        let a = cache.get_or_insert_with(|| {
            built += 1;
            vec![1u32, 2]
        });
        let b = cache.get_or_insert_with(|| {
            built += 1;
            vec![9u32]
        });
        assert_eq!(built, 1);
        assert!(Arc::ptr_eq(&a, &b));
        let other = cache.get_or_insert_with(|| "text".to_string());
        assert_eq!(other.as_str(), "text");
    }
}
