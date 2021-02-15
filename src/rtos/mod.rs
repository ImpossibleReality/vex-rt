//! Multitasking primitives.

use alloc::{boxed::Box, format, string::String};
use core::{
    cmp::min,
    convert::TryInto,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
    ops::{Add, Div, Mul, Sub},
    time::Duration,
};

use crate::{
    bindings,
    error::{Error, SentinelError},
    util::cstring::{as_cstring, from_cstring_raw},
};

const TIMEOUT_MAX: u32 = 0xffffffff;

/// Represents a time on a monotonically increasing clock (i.e., time since
/// program start).
///
/// This type has a precision of 1 millisecond.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u32);

impl Instant {
    #[inline]
    /// Creates a new `Instant` from the specified number of whole milliseconds
    /// since program start.
    pub fn from_millis(millis: u32) -> Self {
        Self(millis)
    }

    /// Creates a new `Instant` from the specified number of whole seconds since
    /// program start.
    pub fn from_secs(secs: u32) -> Self {
        Self(
            secs.checked_mul(1000)
                .expect("overflow when creating instant from seconds"),
        )
    }

    #[inline]
    /// Returns the number of *whole* seconds since program start contained by
    /// this `Instant`.
    ///
    /// The returned value does not include the fractional (milliseconds) part
    /// of the time value.
    pub fn as_millis(&self) -> u32 {
        self.0
    }

    #[inline]
    /// Returns the number of whole milliseconds since program start contained
    /// by this `Instant`.
    pub fn as_secs(&self) -> u32 {
        self.0 / 1000
    }

    #[inline]
    /// Returns the fractional part of this `Instant`, in whole milliseconds.
    ///
    /// This method does **not** return the time value in milliseconds. The
    /// returned number always represents a fractional portion of a second
    /// (i.e., it is less than one thousand).
    pub fn subsec_millis(&self) -> u32 {
        self.0 % 1000
    }

    #[inline]
    /// Checked addition of a [`Duration`] to an `Instant`. Computes `self +
    /// rhs`, returning [`None`] if overflow occured.
    pub fn checked_add(self, rhs: Duration) -> Option<Self> {
        Some(Self(self.0.checked_add(rhs.as_millis().try_into().ok()?)?))
    }

    #[inline]
    /// Checked subtraction of a [`Duration`] from an `Instant`. Computes
    /// `self - rhs`, returning [`None`] if the result would be negative or
    /// overflow occured.
    pub fn checked_sub(self, rhs: Duration) -> Option<Instant> {
        Some(Self(self.0.checked_sub(rhs.as_millis().try_into().ok()?)?))
    }

    #[inline]
    /// Checked subtraction of two `Instant`s. Computes `self - rhs`, returning
    /// [`None`] if the result would be negative or overflow occured.
    pub fn checked_sub_instant(self, rhs: Self) -> Option<Duration> {
        Some(Duration::from_millis(self.0.checked_sub(rhs.0)?.into()))
    }

    #[inline]
    /// Checked multiplication of an `Instant` by a scalar. Computes `self *
    /// rhs`, returning [`None`] if an overflow occured.
    pub fn checked_mul(self, rhs: u32) -> Option<Instant> {
        Some(Self(self.0.checked_mul(rhs)?))
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Self::Output {
        self.checked_add(rhs)
            .expect("overflow when adding duration to instant")
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Self::Output {
        self.checked_sub(rhs)
            .expect("overflow when subtracting duration from instant")
    }
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub_instant(rhs)
            .expect("overflow when subtracting instants")
    }
}

impl Mul<u32> for Instant {
    type Output = Instant;

    fn mul(self, rhs: u32) -> Self::Output {
        self.checked_mul(rhs)
            .expect("overflow when multiplying instant by scalar")
    }
}

impl Div<u32> for Instant {
    type Output = Instant;

    #[inline]
    fn div(self, rhs: u32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Debug for Instant {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:03} s", self.0 / 1000, self.0 % 1000)
    }
}

impl Display for Instant {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:03} s", self.0 / 1000, self.0 % 1000)
    }
}

#[inline]
/// Gets the current timestamp (i.e., the time which has passed since program
/// start).
pub fn time_since_start() -> Instant {
    unsafe { Instant::from_millis(bindings::millis()) }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
/// Represents a FreeRTOS task.
pub struct Task(bindings::task_t);

impl Task {
    /// The default priority for new tasks.
    pub const DEFAULT_PRIORITY: u32 = bindings::TASK_PRIORITY_DEFAULT;

    /// The default stack depth for new tasks.
    pub const DEFAULT_STACK_DEPTH: u16 = bindings::TASK_STACK_DEPTH_DEFAULT as u16;

    #[inline]
    /// Delays the current task by the specified duration.
    pub fn delay(dur: Duration) {
        unsafe {
            bindings::task_delay(dur.as_millis() as u32);
        }
    }

    #[inline]
    /// Gets the current task.
    pub fn current() -> Task {
        Task(unsafe { bindings::task_get_current() })
    }

    /// Finds a task by its name.
    pub fn find_by_name(name: &str) -> Result<Task, Error> {
        let ptr = as_cstring(name, |cname| unsafe {
            Ok(bindings::task_get_by_name(cname.into_raw()))
        })?;
        if ptr.is_null() {
            Err(Error::Custom(format!("task not found: {}", name)))
        } else {
            Ok(Task(ptr))
        }
    }

    #[inline]
    /// Spawns a new task with no name and the default priority and stack depth.
    pub fn spawn<F>(f: F) -> Result<Task, Error>
    where
        F: FnOnce() + Send + 'static,
    {
        Task::spawn_ext("", Self::DEFAULT_PRIORITY, Self::DEFAULT_STACK_DEPTH, f)
    }

    /// Spawns a new task with the specified name, priority and stack depth.
    pub fn spawn_ext<F>(name: &str, priority: u32, stack_depth: u16, f: F) -> Result<Task, Error>
    where
        F: FnOnce() + Send + 'static,
    {
        extern "C" fn run<F: FnOnce()>(arg: *mut libc::c_void) {
            let cb_box: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            cb_box()
        }

        let cb = Box::new(f);
        unsafe {
            let arg = Box::into_raw(cb);
            let r = Task::spawn_raw(name, priority, stack_depth, run::<F>, arg as *mut _);
            if r.is_err() {
                // We need to re-box the pointer if the task could not be created, to avoid a
                // memory leak.
                Box::from_raw(arg);
            }
            r
        }
    }

    #[inline]
    /// Spawns a new task from a C function pointer and an arbitrary data
    /// pointer.
    pub fn spawn_raw(
        name: &str,
        priority: u32,
        stack_depth: u16,
        f: unsafe extern "C" fn(arg1: *mut libc::c_void),
        arg: *mut libc::c_void,
    ) -> Result<Task, Error> {
        as_cstring(name, |cname| {
            Ok(Task(
                unsafe {
                    bindings::task_create(Some(f), arg, priority, stack_depth, cname.into_raw())
                }
                .check()?,
            ))
        })
    }

    #[inline]
    /// Gets the name of the task.
    pub fn name(&self) -> String {
        unsafe { from_cstring_raw(bindings::task_get_name(self.0)) }
    }

    #[inline]
    /// Gets the priority of the task.
    pub fn priority(&self) -> u32 {
        unsafe { bindings::task_get_priority(self.0) }
    }

    #[inline]
    /// Gets the state of the task.
    pub fn state(&self) -> TaskState {
        match unsafe { bindings::task_get_state(self.0) } {
            bindings::task_state_e_t_E_TASK_STATE_RUNNING => TaskState::Running,
            bindings::task_state_e_t_E_TASK_STATE_READY => TaskState::Ready,
            bindings::task_state_e_t_E_TASK_STATE_BLOCKED => TaskState::Blocked,
            bindings::task_state_e_t_E_TASK_STATE_SUSPENDED => TaskState::Suspended,
            bindings::task_state_e_t_E_TASK_STATE_DELETED => TaskState::Deleted,
            bindings::task_state_e_t_E_TASK_STATE_INVALID => {
                panic!("invalid task handle: {:#010x}", self.0 as usize)
            }
            x => panic!("bindings::task_get_state returned unexpected value: {}", x),
        }
    }

    #[inline]
    /// Unsafely deletes the task.
    ///
    /// # Safety
    /// This is unsafe because it does not guarantee that the task's code safely
    /// unwinds (i.e., that destructors are called, memory is freed and other
    /// resources are released).
    pub unsafe fn delete(&self) {
        bindings::task_delete(self.0)
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("name", &self.name())
            .field("priority", &self.priority())
            .finish()
    }
}

unsafe impl Send for Task {}

unsafe impl Sync for Task {}

/// Represents the state of a [`Task`].
pub enum TaskState {
    /// The task is actively executing.
    Running,
    /// The task exists and is available to run, but is not currently running.
    Ready,
    /// The task is delayed or blocked by a mutex, semaphore or I/O operation.
    Blocked,
    /// The task is suspended.
    Suspended,
    /// The task has been deleted.
    Deleted,
}

#[derive(Copy, Clone, Debug)]
/// Represents a future time to sleep until.
pub enum GenericSleep {
    /// Represents a future time when a notification occurs. If a timestamp is
    /// present, then it represents whichever is earlier.
    NotifyTake(Option<Instant>),
    /// Represents an explicit future timestamp.
    Timestamp(Instant),
}

impl GenericSleep {
    /// Sleeps until the future time respresented by `self`. The result is the
    /// number of notifications which were present, if the sleep ended due to
    /// notification.
    pub fn sleep(self) -> u32 {
        match self {
            GenericSleep::NotifyTake(timeout) => {
                let timeout = timeout.map_or(TIMEOUT_MAX, |v| {
                    v.checked_sub_instant(time_since_start())
                        .map_or(0, |d| d.as_millis() as u32)
                });
                unsafe { bindings::task_notify_take(true, timeout) }
            }
            GenericSleep::Timestamp(v) => {
                if let Some(d) = v.checked_sub_instant(time_since_start()) {
                    Task::delay(d)
                }
                0
            }
        }
    }

    #[inline]
    /// Get the timestamp represented by `self`, if it is present.
    pub fn timeout(self) -> Option<Instant> {
        match self {
            GenericSleep::NotifyTake(v) => v,
            GenericSleep::Timestamp(v) => Some(v),
        }
    }

    /// Combine two `GenericSleep` objects to one which represents the earliest
    /// possible time of the two.
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (GenericSleep::Timestamp(a), GenericSleep::Timestamp(b)) => {
                GenericSleep::Timestamp(core::cmp::min(a, b))
            }
            (a, b) => GenericSleep::NotifyTake(
                a.timeout()
                    .map_or(b.timeout(), |a| Some(b.timeout().map_or(a, |b| min(a, b)))),
            ),
        }
    }
}

/// Represents a future event which can be used with the [`select!`] macro.
pub trait Selectable<T = ()>: Sized {
    /// Processes the event if it is ready, consuming the event object;
    /// otherwise, it provides a replacement event object.
    fn poll(self) -> Result<T, Self>;
    /// Gets the earliest time that the event could be ready.
    fn sleep(&self) -> GenericSleep;
}

/// Creates a new [`Selectable`] event by mapping the result of a given one.
#[inline]
pub fn select_map<'a, T: 'a, U: 'a, F: 'a + FnOnce(T) -> U>(
    event: impl Selectable<T> + 'a,
    f: F,
) -> impl Selectable<U> + 'a {
    struct MapSelect<T, U, E: Selectable<T>, F: FnOnce(T) -> U> {
        event: E,
        f: F,
        _t: PhantomData<T>,
    }

    impl<T, U, E: Selectable<T>, F: FnOnce(T) -> U> Selectable<U> for MapSelect<T, U, E, F> {
        fn poll(self) -> Result<U, Self> {
            match self.event.poll() {
                Ok(r) => Ok((self.f)(r)),
                Err(event) => Err(Self {
                    event,
                    f: self.f,
                    _t: PhantomData,
                }),
            }
        }
        fn sleep(&self) -> GenericSleep {
            self.event.sleep()
        }
    }

    MapSelect {
        event,
        f,
        _t: PhantomData,
    }
}

/// Creates a new [`Selectable`] event which processes exactly one of the given
/// events.
#[inline]
pub fn select_either<'a, T: 'a>(
    fst: impl Selectable<T> + 'a,
    snd: impl Selectable<T> + 'a,
) -> impl Selectable<T> + 'a {
    struct EitherSelect<T, E1: Selectable<T>, E2: Selectable<T>>(E1, E2, PhantomData<T>);

    impl<T, E1: Selectable<T>, E2: Selectable<T>> Selectable<T> for EitherSelect<T, E1, E2> {
        fn poll(self) -> Result<T, Self> {
            Err(Self(
                match self.0.poll() {
                    Ok(r) => return Ok(r),
                    Err(e) => e,
                },
                match self.1.poll() {
                    Ok(r) => return Ok(r),
                    Err(e) => e,
                },
                PhantomData,
            ))
        }
        fn sleep(&self) -> GenericSleep {
            self.0.sleep().combine(self.1.sleep())
        }
    }

    EitherSelect(fst, snd, PhantomData)
}

mod broadcast;
mod channel;
mod context;
mod event;
mod r#loop;
mod mutex;
mod promise;
mod semaphore;

pub use broadcast::*;
pub use channel::*;
pub use context::*;
pub use event::*;
pub use mutex::*;
pub use promise::*;
pub use r#loop::*;
pub use semaphore::*;
