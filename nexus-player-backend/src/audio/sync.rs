//! Poison-tolerant lock helpers for the audio path.
//!
//! A panic while a lock is held poisons it permanently. On the realtime audio path that would
//! either flood the log or make every later `try_lock` fail, so the helpers log once, take the
//! guard through `into_inner` and clear the poison flag.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::error;

pub fn lock_recover<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| {
        error!("mutex poisoned by a panic in another thread, recovering");
        lock.clear_poison();
        poisoned.into_inner()
    })
}

pub fn read_recover<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| {
        error!("rwlock poisoned by a panic in another thread, recovering");
        lock.clear_poison();
        poisoned.into_inner()
    })
}

pub fn write_recover<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|poisoned| {
        error!("rwlock poisoned by a panic in another thread, recovering");
        lock.clear_poison();
        poisoned.into_inner()
    })
}
