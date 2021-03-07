//! The Rust ecosystem has no support for Mutex's that avoid deadlock by self-relocking apparently
//! (??) so I gotta implement it myself

use std::sync::atomic::{AtomicU64, Ordering};

pub struct AntiDeadlockMutex<T> {
	current_holder: AtomicU64,
	inner: std::sync::Mutex<T>,
}

impl<T> AntiDeadlockMutex<T> {
	pub fn new(value: T) -> Self {
		Self {
			current_holder: AtomicU64::from(0),
			inner: std::sync::Mutex::new(value),
		}
	}

	/// This will panic if the current thread is already holding a lock. This is done in order to
	/// prevent a deadlock, like it would happen with std's or parking_lot's Mutexes
	///
	/// If the mutex was poisened, the panic will be propagated
	pub fn lock(&self) -> AntiDeadlockMutexGuard<'_, T> {
		// UNSAFE: ThreadId is just a wrapper around u64, so it can be transmuted to u64
		let current_thread_id: u64 = unsafe { std::mem::transmute(std::thread::current().id()) };

		if self.current_holder.load(Ordering::Relaxed) == current_thread_id {
			// This mutex is already being held by this thread. Locking it again would cause a
			// deadlock
			panic!("Attempted to lock a mutex that was already locked on the same thread");
		}

		// UNWRAP: propagate panics
		let guard = self.inner.lock().unwrap();

		self.current_holder
			.store(current_thread_id, Ordering::Relaxed);

		AntiDeadlockMutexGuard {
			inner: guard,
			current_holder: &self.current_holder,
		}
	}
}

pub struct AntiDeadlockMutexGuard<'a, T> {
	inner: std::sync::MutexGuard<'a, T>,
	current_holder: &'a AtomicU64,
}

impl<T> std::ops::Deref for AntiDeadlockMutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&*self.inner
	}
}

impl<T> std::ops::DerefMut for AntiDeadlockMutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut *self.inner
	}
}

impl<T> Drop for AntiDeadlockMutexGuard<'_, T> {
	fn drop(&mut self) {
		self.current_holder.store(0, Ordering::Relaxed);
	}
}
