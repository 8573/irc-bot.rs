use core::ErrorKind;
use core::Result;
use std::borrow::Cow;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

pub(crate) trait ReadLockExt<T> {
    /// Acquires the lock for reading if it is clean (i.e., not poisoned).
    ///
    /// If the lock is poisoned, returns an error saying that a lock of the given `description` was
    /// poisoned.
    fn read_clean<Desc>(&self, description: Desc) -> Result<RwLockReadGuard<T>>
    where
        Desc: Into<Cow<'static, str>>;
}

pub(crate) trait WriteLockExt<T> {
    /// Acquires the lock for writing if it is clean (i.e., not poisoned).
    ///
    /// If the lock is poisoned, returns an error saying that a lock of the given `description` was
    /// poisoned.
    fn write_clean<Desc>(&self, description: Desc) -> Result<RwLockWriteGuard<T>>
    where
        Desc: Into<Cow<'static, str>>;
}

impl<T> ReadLockExt<T> for RwLock<T> {
    fn read_clean<Desc>(&self, description: Desc) -> Result<RwLockReadGuard<T>>
    where
        Desc: Into<Cow<'static, str>>,
    {
        self.read()
            .map_err(|PoisonError { .. }| ErrorKind::LockPoisoned(description.into().into()).into())
    }
}

impl<T> WriteLockExt<T> for RwLock<T> {
    fn write_clean<Desc>(&self, description: Desc) -> Result<RwLockWriteGuard<T>>
    where
        Desc: Into<Cow<'static, str>>,
    {
        self.write()
            .map_err(|PoisonError { .. }| ErrorKind::LockPoisoned(description.into().into()).into())
    }
}

pub(crate) trait MutexExt<T> {
    /// Acquires the lock if it is clean (i.e., not poisoned).
    ///
    /// If the lock is poisoned, returns an error saying that a lock of the given `description` was
    /// poisoned.
    fn lock_clean<Desc>(&self, description: Desc) -> Result<MutexGuard<T>>
    where
        Desc: Into<Cow<'static, str>>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_clean<Desc>(&self, description: Desc) -> Result<MutexGuard<T>>
    where
        Desc: Into<Cow<'static, str>>,
    {
        self.lock()
            .map_err(|PoisonError { .. }| ErrorKind::LockPoisoned(description.into().into()).into())
    }
}
