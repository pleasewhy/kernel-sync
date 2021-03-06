use core::{
    cell::UnsafeCell,
    fmt,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::interrupt::{cpu_id, pop_off, push_off};

pub struct SpinLock<T: ?Sized> {
    // phantom: PhantomData<R>,
    pub(crate) locked: AtomicBool,
    cpuid: u8,
    data: UnsafeCell<T>,
}

/// An RAII implementation of a “scoped lock” of a mutex.
/// When this structure is dropped (falls out of scope),
/// the lock will be unlocked.
///
pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    spinlock: &'a SpinLock<T>,
    data: &'a mut T,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        SpinLock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            cpuid: 0,
        }
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let SpinLock { data, .. } = self;
        data.into_inner()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: ?Sized> SpinLock<T> {
    #[inline(always)]
    pub fn lock(&self) -> SpinLockGuard<T> {
        push_off();
        if self.holding() {
            panic!("a spinlock can only be locked once by a CPU");
        }

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Wait until the lock looks unlocked before retrying
            while self.is_locked() {
                // R::relax();
            }
        }

        SpinLockGuard {
            spinlock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    #[inline(always)]
    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        push_off();
        if self.holding() {
            panic!("a spinlock can only be locked once by a CPU");
        }
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinLockGuard {
                spinlock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            pop_off();
            None
        }
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // We know statically that there are no other references to `self`, so
        // there's no need to lock the inner mutex.
        unsafe { &mut *self.data.get() }
    }

    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    /// Check whether this cpu is holding the lock.
    /// Interrupts must be off.
    #[inline(always)]
    pub fn holding(&self) -> bool {
        return self.is_locked() && self.cpuid == cpu_id();
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SpinLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        if !self.spinlock.holding() {
            panic!("current cpu doesn't hold the lock{}", self.spinlock);
        }
        self.spinlock.locked.store(false, Ordering::Release);
    }
}

impl<T: ?Sized> fmt::Display for SpinLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Spinlock{{locked={}, cpuid={}}}",
            self.locked.load(Ordering::Relaxed),
            self.cpuid,
        )
    }
}
