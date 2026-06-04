//! A poisoned lock on the audio path recovers its value, loses the poison flag so later
//! try_lock calls succeed, and does not panic.

use crate::audio::sync::{lock_recover, read_recover, write_recover};
use std::sync::{Mutex, RwLock};
use std::thread;

fn poison_mutex(lock: &Mutex<i32>) {
    let outcome = thread::scope(|scope| {
        scope
            .spawn(|| {
                let mut guard = lock.lock().unwrap();
                *guard = 42;
                panic!("poisoning the mutex on purpose");
            })
            .join()
    });
    assert!(outcome.is_err());
    assert!(lock.is_poisoned());
}

fn poison_rwlock(lock: &RwLock<i32>) {
    let outcome = thread::scope(|scope| {
        scope
            .spawn(|| {
                let mut guard = lock.write().unwrap();
                *guard = 42;
                panic!("poisoning the rwlock on purpose");
            })
            .join()
    });
    assert!(outcome.is_err());
    assert!(lock.is_poisoned());
}

#[test]
fn lock_recover_restores_a_poisoned_mutex() {
    let lock = Mutex::new(1);
    poison_mutex(&lock);

    let guard = lock_recover(&lock);
    assert_eq!(*guard, 42);
    drop(guard);

    assert!(!lock.is_poisoned());
    assert!(lock.try_lock().is_ok());
}

#[test]
fn read_recover_restores_a_poisoned_rwlock() {
    let lock = RwLock::new(1);
    poison_rwlock(&lock);

    let guard = read_recover(&lock);
    assert_eq!(*guard, 42);
    drop(guard);

    assert!(!lock.is_poisoned());
    assert!(lock.try_read().is_ok());
    assert!(lock.try_write().is_ok());
}

#[test]
fn write_recover_restores_a_poisoned_rwlock() {
    let lock = RwLock::new(1);
    poison_rwlock(&lock);

    let mut guard = write_recover(&lock);
    assert_eq!(*guard, 42);
    *guard = 43;
    drop(guard);

    assert!(!lock.is_poisoned());
    assert!(lock.try_write().is_ok());
    assert_eq!(*lock.try_read().unwrap(), 43);
}

#[test]
fn helpers_work_on_healthy_locks() {
    let mutex = Mutex::new(1);
    *lock_recover(&mutex) += 1;
    assert_eq!(*mutex.try_lock().unwrap(), 2);

    let rwlock = RwLock::new(1);
    *write_recover(&rwlock) += 1;
    assert_eq!(*read_recover(&rwlock), 2);
    assert!(!rwlock.is_poisoned());
}
