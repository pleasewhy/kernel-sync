use core::sync::atomic::{AtomicU8, Ordering};
use core::task::Waker;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

const WAITER_INDEX: u8 = 1 << 0;
const WAITER_DISABLED: u8 = 1 << 1;

pub struct Waiter {
  state: AtomicU8,
  wakers: [UnsafeCell<MaybeUninit<Waker>>; 2],
}

impl Waiter {
  pub(crate) fn register(&self, waker: &Waker) {
      let state = self.state.load(Ordering::Acquire);
      let mut index = (state & WAITER_INDEX) as usize;
      if state & WAITER_DISABLED != 0
          || !waker
              .will_wake(unsafe { (*self.wakers.get_unchecked(index).get()).assume_init_ref() })
      {
          index = (index + 1) % 2;
          unsafe { (*self.wakers.get_unchecked(index).get()).write(waker.clone()) };
          self.state.store(index as u8, Ordering::Release);
      }
  }

  pub(crate) fn wake(&self) -> bool {
      let state = self.disable();
      if state & WAITER_DISABLED == 0 {
          let index = (state & WAITER_INDEX) as usize;
          unsafe { (*self.wakers.get_unchecked(index).get()).assume_init_read().wake() };
          true
      } else {
          false
      }
  }

  pub(crate) fn disable(&self) -> u8 {
      self.state.fetch_or(WAITER_DISABLED, Ordering::Relaxed)
  }

  pub(crate) fn is_disabled(&self) -> bool {
      self.state.load(Ordering::Relaxed) & WAITER_DISABLED != 0
  }
}

impl From<Waker> for Waiter {
    fn from(waker: Waker) -> Self {
        Self {
            state: AtomicU8::new(0),
            wakers: [
                UnsafeCell::new(MaybeUninit::new(waker)),
                UnsafeCell::new(MaybeUninit::uninit()),
            ],
        }
    }
}