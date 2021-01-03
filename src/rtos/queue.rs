use alloc::{sync::Arc, vec::Vec};
use core::{marker::PhantomData, mem::size_of, ptr::null_mut, time::Duration};

use libc::c_void;

use crate::{
    bindings,
    error::{from_errno, Error},
};

/// Represents a FreeRTOS FIFO queue.
///
/// Clones of the object refer to the same underlying queue; they are shallow
/// clones.
pub struct Queue<T: Copy + Send>(Arc<QueueData>, PhantomData<T>);

impl<T: Copy + Send> Queue<T> {
    #[inline]
    /// Creates a new queue with the given length. Panics on failure; see
    /// [`Queue::try_new()`].
    pub fn new(length: u32) -> Self {
        Self::try_new(length).unwrap()
    }

    /// Creates a new queue with the given length.
    pub fn try_new(length: u32) -> Result<Self, Error> {
        let q = unsafe { bindings::queue_create(length, size_of::<T>() as u32) };
        if q == null_mut() {
            Err(from_errno())
        } else {
            Ok(Self(Arc::new(QueueData(q)), PhantomData))
        }
    }

    /// Posts an item to the front of the queue. If the queue still does not
    /// have an empty slot after `timeout` has passed, the item is returned
    /// via the [`Err`] constructor.
    pub fn prepend(&self, item: T, timeout: Duration) -> Result<(), T> {
        if unsafe {
            bindings::queue_prepend(
                self.queue(),
                &item as *const T as *const c_void,
                timeout.as_secs() as u32,
            )
        } {
            Ok(())
        } else {
            Err(item)
        }
    }

    /// Posts an item to the back of the queue. If the queue still does not have
    /// an empty slot after `timeout` has passed, the item is returned via
    /// the [`Err`] constructor.
    pub fn append(&self, item: T, timeout: Duration) -> Result<(), T> {
        if unsafe {
            bindings::queue_append(
                self.queue(),
                &item as *const T as *const c_void,
                timeout.as_secs() as u32,
            )
        } {
            Ok(())
        } else {
            Err(item)
        }
    }

    /// Obtains a copy of the element at the front of the queue, without
    /// removing it. If the queue is still empty after `timeout` has passed,
    /// [`None`] is returned.
    pub fn peek(&self, timeout: Duration) -> Option<T> {
        let mut buf = Vec::<T>::new();
        buf.reserve_exact(1);
        unsafe {
            if bindings::queue_peek(
                self.queue(),
                buf.as_mut_ptr() as *mut c_void,
                timeout.as_secs() as u32,
            ) {
                buf.set_len(1);
                Some(buf[0])
            } else {
                None
            }
        }
    }

    /// Receives an element from the front of the queue, removing it. If the
    /// queue is still empty after `timeout` has passed, [`None`] is returned.
    pub fn recv(&self, timeout: Duration) -> Option<T> {
        let mut buf = Vec::<T>::new();
        buf.reserve_exact(1);
        unsafe {
            if bindings::queue_recv(
                self.queue(),
                buf.as_mut_ptr() as *mut c_void,
                timeout.as_secs() as u32,
            ) {
                buf.set_len(1);
                Some(buf[0])
            } else {
                None
            }
        }
    }

    #[inline]
    /// Gets the number of elements currently in the queue.
    pub fn waiting(&self) -> u32 {
        unsafe { bindings::queue_get_waiting(self.queue()) }
    }

    #[inline]
    /// Gets the number of available slots in the queue (i.e., the number of
    /// elements which can be added to the queue).
    pub fn available(&self) -> u32 {
        unsafe { bindings::queue_get_available(self.queue()) }
    }

    #[inline]
    /// Clears the queue (i.e., deletes all elements).
    pub fn clear(&self) {
        unsafe { bindings::queue_reset(self.queue()) }
    }

    #[inline]
    unsafe fn queue(&self) -> bindings::queue_t {
        self.0 .0
    }
}

impl<T: Copy + Send> Clone for Queue<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

unsafe impl<T: Copy + Send> Send for Queue<T> {}
unsafe impl<T: Copy + Send> Sync for Queue<T> {}

struct QueueData(bindings::queue_t);

impl Drop for QueueData {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::queue_delete(self.0) }
    }
}
