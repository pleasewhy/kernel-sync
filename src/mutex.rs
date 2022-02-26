extern crate alloc;
use alloc::collections::linked_list::LinkedList;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use core::fmt;

use crate::spinlock::SpinLock;
use crate::waiter::Waiter;

/// A mutual exclusion and asynchronous primitive which could work
/// in bare metal environments.
///
/// This mutex does not block coroutine, and it yield the CPU when
/// the lock is unavailable, and it wake up when the lock is available.
/// The mutex can also be statically initialized or created via a new
/// constructor.  Each mutex has a type parameter which represents the
/// data that it is protecting. The data can only be accessed through
/// the RAII guards returned from lock and try_lock, which guarantees
/// that the data is only ever accessed when the mutex is locked.
pub struct Mutex<T: ?Sized> {
    state: AtomicBool,
    waiters: SpinLock<LinkedList<Arc<Waiter>>>,
    data: UnsafeCell<T>,
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// [`Deref`] and [`DerefMut`] implementations.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods on
/// [`Mutex`].
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
}

/// A future which resolves when the target mutex has been successfully
/// acquired.
pub struct MutexLockFuture<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
    waiter: Option<Arc<Waiter>>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for MutexLockFuture<'_, T> {}
unsafe impl<T: ?Sized + Send> Sync for MutexLockFuture<'_, T> {}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(t: T) -> Self {
        Mutex {
            state: AtomicBool::new(false),
            waiters: SpinLock::new(LinkedList::new()),
            data: UnsafeCell::new(t),
        }
    }

    /// Returns a mutable pointer to the underlying data.
    ///
    /// This is mostly meant to be used for applications which require manual unlocking, but where
    /// storing both the lock and the pointer to the inner data gets inefficient.
    ///
    /// # Example
    /// ```
    /// let lock = spin::mutex::SpinMutex::<_>::new(42);
    ///
    /// unsafe {
    ///     core::mem::forget(lock.lock());
    ///     
    ///     assert_eq!(lock.as_mut_ptr().read(), 42);
    ///     lock.as_mut_ptr().write(58);
    ///
    ///     lock.force_unlock();
    /// }
    ///
    /// assert_eq!(*lock.lock(), 58);
    ///
    /// ```
    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn lock(&self) -> MutexLockFuture<'_, T> {
        return MutexLockFuture {
            mutex: self,
            waiter: None,
        };
    }

    /// Attempts to acquire this lock immedidately.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if !self.state.fetch_or(true, Ordering::Acquire) {
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }

    pub fn unlock(&self) {
        let mut waiters = self.waiters.lock();
        let disabled_waiters = waiters.drain_filter(|waiter| waiter.is_disabled());
        drop(disabled_waiters); // remove disabled waiters.

        for waiter in waiters.iter_mut() {
            if waiter.wake() {
                break;
            }
        }
        if !self.state.fetch_and(false, Ordering::Acquire) {
            panic!("unlock a unlocked mutex");
        } 
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T: ?Sized> MutexLockFuture<'_, T> {
    fn disable_waiter(&mut self) {
        if let Some(waiter) = self.waiter.take() {
            waiter.disable();
        }
    }
}

impl<'a, T: ?Sized> Future for MutexLockFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(guard) = self.mutex.try_lock() {
            self.disable_waiter();
            return Poll::Ready(guard);
        }
        if let Some(waiter) = &self.waiter {
            waiter.register(cx.waker());
        } else {
            let waiter = Arc::new(Waiter::from(cx.waker().clone()));
            self.waiter = Some(waiter.clone());
            let mut mutex_waiters = self.mutex.waiters.lock();
            mutex_waiters.push_back(waiter.clone());
        }
        if let Some(guard) = self.mutex.try_lock() {
            self.disable_waiter();
            return Poll::Ready(guard);
        }
        Poll::Pending
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
