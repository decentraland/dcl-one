//! Poison-ignoring acquisition for `std::sync` locks.
//!
//! A guard dropped during a panic poisons its lock. These methods return the guard anyway, so a
//! panic on one thread never turns later acquisitions on other threads into panics. They do not
//! repair the data: a value the panicking thread left half-updated stays that way.

use std::sync::{
    Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError,
};

pub trait MutexExt<T: ?Sized> {
    /// Blocks until the lock is acquired, ignoring poison.
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T>;
    /// Acquires the lock without blocking, ignoring poison; `None` when another thread holds it.
    fn try_lock_ignore_poison(&self) -> Option<MutexGuard<'_, T>>;
}

impl<T: ?Sized> MutexExt<T> for Mutex<T> {
    fn lock_ignore_poison(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn try_lock_ignore_poison(&self) -> Option<MutexGuard<'_, T>> {
        match self.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

pub trait RwLockExt<T: ?Sized> {
    /// Blocks until shared access is acquired, ignoring poison.
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T>;
    /// Blocks until exclusive access is acquired, ignoring poison.
    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T>;
    /// Acquires shared access without blocking, ignoring poison; `None` when a writer holds it.
    fn try_read_ignore_poison(&self) -> Option<RwLockReadGuard<'_, T>>;
    /// Acquires exclusive access without blocking, ignoring poison; `None` when any guard is held.
    fn try_write_ignore_poison(&self) -> Option<RwLockWriteGuard<'_, T>>;
}

impl<T: ?Sized> RwLockExt<T> for RwLock<T> {
    fn read_ignore_poison(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write_ignore_poison(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(PoisonError::into_inner)
    }

    fn try_read_ignore_poison(&self) -> Option<RwLockReadGuard<'_, T>> {
        match self.try_read() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    fn try_write_ignore_poison(&self) -> Option<RwLockWriteGuard<'_, T>> {
        match self.try_write() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MutexExt, RwLockExt};
    use std::sync::{Mutex, RwLock};

    fn poison_mutex(mutex: &Mutex<u32>) {
        let result = std::panic::catch_unwind(|| {
            let mut guard = mutex.lock_ignore_poison();
            *guard = 1;
            panic!("poison the mutex");
        });
        assert!(result.is_err());
        assert!(mutex.is_poisoned());
    }

    fn poison_rwlock(lock: &RwLock<u32>) {
        let result = std::panic::catch_unwind(|| {
            let mut guard = lock.write_ignore_poison();
            *guard = 1;
            panic!("poison the rwlock");
        });
        assert!(result.is_err());
        assert!(lock.is_poisoned());
    }

    #[test]
    fn a_poisoned_mutex_keeps_serving_blocking_and_try_lock() {
        let mutex = Mutex::new(0);
        poison_mutex(&mutex);

        *mutex.lock_ignore_poison() += 1;
        assert_eq!(*mutex.lock_ignore_poison(), 2);
        let guard = mutex.try_lock_ignore_poison();
        assert_eq!(guard.as_deref(), Some(&2));
    }

    #[test]
    fn mutex_try_lock_is_none_while_held() {
        let mutex = Mutex::new(0);
        poison_mutex(&mutex);

        let _held = mutex.lock_ignore_poison();
        std::thread::scope(|scope| {
            scope.spawn(|| assert!(mutex.try_lock_ignore_poison().is_none()));
        });
    }

    #[test]
    fn a_poisoned_rwlock_keeps_serving_reads_and_writes() {
        let lock = RwLock::new(0);
        poison_rwlock(&lock);

        *lock.write_ignore_poison() += 1;
        assert_eq!(*lock.read_ignore_poison(), 2);
        assert_eq!(lock.try_read_ignore_poison().as_deref(), Some(&2));
        *lock.try_write_ignore_poison().expect("no guard is held") += 1;
        assert_eq!(*lock.read_ignore_poison(), 3);
    }

    #[test]
    fn rwlock_try_acquisition_is_none_while_held() {
        let lock = RwLock::new(0);
        poison_rwlock(&lock);

        {
            let _reader = lock.read_ignore_poison();
            assert!(lock.try_write_ignore_poison().is_none());
            assert!(lock.try_read_ignore_poison().is_some());
        }
        let _writer = lock.write_ignore_poison();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                assert!(lock.try_read_ignore_poison().is_none());
                assert!(lock.try_write_ignore_poison().is_none());
            });
        });
    }
}
