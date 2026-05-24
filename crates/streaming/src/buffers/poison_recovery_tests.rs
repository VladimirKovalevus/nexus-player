//! R4: a poisoned buffer mutex recovers its value, loses the poison flag so later try_lock
//! calls succeed, and does not panic.

use crate::buffers::sync::lock_recover;
use std::sync::Mutex;
use std::thread;

#[test]
fn lock_recover_restores_a_poisoned_mutex() {
    let lock = Mutex::new(1);
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

    let guard = lock_recover(&lock);
    assert_eq!(*guard, 42);
    drop(guard);

    assert!(!lock.is_poisoned());
    assert!(lock.try_lock().is_ok());
}

#[test]
fn lock_recover_works_on_a_healthy_mutex() {
    let lock = Mutex::new(1);
    *lock_recover(&lock) += 1;
    assert_eq!(*lock.try_lock().unwrap(), 2);
}
