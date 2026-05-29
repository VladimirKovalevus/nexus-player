//! Poison-tolerant mutex helper for the buffers.
//!
//! A panic while a lock is held poisons it permanently. The helper logs, takes the guard through
//! `into_inner` and clears the poison flag so later locks and `try_lock` calls keep working.

use std::sync::{Mutex, MutexGuard};
use tracing::error;

pub fn lock_recover<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| {
        error!("buffer mutex poisoned by a panic in another thread, recovering");
        lock.clear_poison();
        poisoned.into_inner()
    })
}
