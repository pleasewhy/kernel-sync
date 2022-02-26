// use crate::spinlock::SpinLock;
// use alloc::collections::linked_list::LinkedList;
// use core::cell::UnsafeCell;
// use core::sync::atomic::AtomicUsize;
// use core::task::{Context, Poll, Waker};

// /// A asynchronous reader-writer lock.
// pub struct RwLock<T: ?Sized> {
//     state: AtomicUsize,
//     wakers: SpinLock<LinkedList<Waker>>,
//     data: UnsafeCell<T>,
// }

// const READER: usize = 1 << 2;
// const UPGRADED: usize = 1 << 1;
// const WRITER: usize = 1;

// /// A guard that provides immutable data access.
// ///
// /// When the guard falls out of scope it will decrement the read count,
// /// potentially releasing the lock.
// pub struct RwLockReadGuard<'a, T: 'a + ?Sized> {
//     lock: &'a AtomicUsize,
//     data: &'a T,
// }

// /// A guard that provides mutable data access.
// ///
// /// When the guard falls out of scope it will release the lock.
// pub struct RwLockWriteGuard<'a, T: 'a + ?Sized> {
//     inner: &'a RwLock<T>,
//     data: &'a mut T,
// }

// // Same unsafe impls as `std::sync::RwLock`
// unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
// unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

// impl<T> RwLock<T> {
//     /// create a new rwlock wrapping the supplied data.
//     pub const fn new(data: T) -> Self {
//         RwLock {
//             state: AtomicUsize::new(0),
//             wakers: SpinLock::new(LinkedList::new()),
//             data: UnsafeCell::new(data),
//         }
//     }

//     /// Consumes this `RwLock`, returning the underlying data.
//     pub fn into_inner(self) -> T {
//         // We know statically that there are no outstanding references to
//         // `self` so there's no need to lock.
//         let RwLock { data, .. } = self;
//         data.into_inner()
//     }

//     /// Returns a mutable pointer to the underying data.
//     ///
//     /// This is mostly meant to be used for applications which require manual unlocking, but where
//     /// storing both the lock and the pointer to the inner data gets inefficient.
//     ///
//     /// While this is safe, writing to the data is undefined behavior unless the current thread has
//     /// acquired a write lock, and reading requires either a read or write lock.
//     ///
//     /// # Example
//     /// ```
//     /// let lock = spin::RwLock::new(42);
//     ///
//     /// unsafe {
//     ///     core::mem::forget(lock.write());
//     ///     
//     ///     assert_eq!(lock.as_mut_ptr().read(), 42);
//     ///     lock.as_mut_ptr().write(58);
//     ///
//     ///     lock.force_write_unlock();
//     /// }
//     ///
//     /// assert_eq!(*lock.read(), 58);
//     ///
//     /// ```
//     #[inline(always)]
//     pub fn as_mut_ptr(&self) -> *mut T {
//         self.data.get()
//     }
// }

// impl<T: ?Sized> RwLock<T> {
//     /// Locks this rwlock with shared read access, yield the current future
//     /// if it can't be acquired.
//     ///
//     /// The calling thread will be blocked until there are no more writers which
//     /// hold the lock. There may be other readers currently inside the lock when
//     /// this method returns. This method does not provide any guarantees with
//     /// respect to the ordering of whether contentious readers or writers will
//     /// acquire the lock first.
//     ///
//     /// Returns an RAII guard which will release this thread's shared access
//     /// once it is dropped.
//     ///
//     /// ```
//     /// let mylock = RwLock::new(0);
//     /// {
//     ///     let mut data = mylock.read().await;
//     ///     // The lock is now locked and the data can be read
//     ///     println!("{}", *data);
//     ///     // The lock is dropped
//     /// }
//     /// ```
//     #[inline]
//     pub fn read(&self) -> RwLockReadGuard<T> {
//         loop {
//             match self.try_read() {
//                 Some(guard) => return guard,
//                 None => R::relax(),
//             }
//         }
//     }
// }
