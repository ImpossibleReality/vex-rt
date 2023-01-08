#[macro_export]
/// Selects over a range of possible future events, processing exactly one.
/// Inspired by equivalent behaviours in other programming languages such as Go
/// and Kotlin, and ultimately the `select` system call from POSIX.
///
/// Which event gets processed is a case of bounded non-determinism: the
/// implementation makes no guarantee about which event gets processed if
/// multiple become possible around the same time, only that it will process one
/// of them if at least one can be processed.
///
/// # Examples
///
/// ```
/// fn foo(ctx: Context) {
///     let mut x = 0;
///     let mut l = Loop::new(Duration::from_secs(1));
///     loop {
///         println!("x = {}", x);
///         x += 1;
///         select! {
///             _ = l.next() => continue,
///             _ = ctx.done() => break,
///         }
///     }
/// }
/// ```
macro_rules! select {
    { $( $var:pat = $event:expr $(; $sub:pat = $dep:expr)* => $body:expr ),+ $(,)? } => {{
        let mut events = $crate::select_head!($($event $(; $sub = $dep)* ;;)+);
        $crate::select_body!{loop {
            $crate::rtos::GenericSleep::sleep($crate::select_sleep!(events; $($event,)+));
            events = $crate::select_match!{events; |r| r; $($event,)+};
        }; $($var => {$body},)+}
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_head {
    ($event:expr $(; $sub:pat = $dep:expr)* ;;) => {
        $crate::select_init!($event $(; $sub = $dep)*)
    };
    ($event:expr $(; $sub1:pat = $dep1:expr)* ;; $($rest:expr $(; $sub:pat = $dep:expr)* ;;)+) => {
        ($crate::select_init!($event $(; $sub1 = $dep1)*), $crate::select_head!($($rest $(; $sub = $dep)* ;;)*))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_init {
    ($(@NEXT)? $event:expr) => {
        $event
    };
    ($event:expr $(; $sub:pat = $dep:expr)+) => {
        $crate::rtos::select_option($crate::select_init!(@NEXT ::core::option::Option::Some($event) $(; $sub = $dep)+))
    };
    (@NEXT $event:expr; $sub1:pat = $dep1:expr $(; $sub:pat = $dep:expr)*) => {
        $crate::select_init!(@NEXT if let $sub1 = $dep1 { $event } else { ::core::option::Option::None } $(; $sub = $dep)*)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_match {
    { $event:expr; $cons:expr; $_:expr, } => {
        match $crate::rtos::Selectable::poll($event) {
            ::core::result::Result::Ok(r) => break $cons(r),
            ::core::result::Result::Err(s) => s,
        }
    };
    { $events:expr; $cons:expr; $_:expr, $($rest:expr,)+ } => {
        match $crate::rtos::Selectable::poll($events.0) {
            ::core::result::Result::Ok(r) => break $cons(::core::result::Result::Ok(r)),
            ::core::result::Result::Err(s) => (
                s,
                $crate::select_match!{$events.1; |r| $cons(::core::result::Result::Err(r)); $($rest,)*}
            ),
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_body {
    { $result:expr; $var:pat => $body:expr, } => {
        match $result {
            $var => $body,
        }
    };
    { $result:expr; $var:pat => $body:expr, $($vars:pat => $bodys:expr,)+ } => {
        match $result {
            ::core::result::Result::Ok($var) => $body,
            ::core::result::Result::Err(r) => $crate::select_body!{r; $($vars => $bodys,)*},
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_sleep {
    ($event:expr; $_:expr,) => {$crate::rtos::Selectable::sleep(&$event)};
    ($events:expr; $_:expr, $($rest:expr,)+) => {
        $crate::rtos::Selectable::sleep(&$events.0).combine($crate::select_sleep!($events.1; $($rest,)+))
    };
}

#[macro_export]
/// Generates a future event (i.e. one which implements
/// [`crate::rtos::Selectable`]) from a similar recipe as the [`select!`]
/// macro, combining the behaviour of [`select_map`](crate::rtos::select_map)
/// and [`select_any!`](crate::select_any!).
///
/// There is one important difference to note between this macro and
/// [`select!`]: since this macro needs to generate an object containing the
/// event processing recipe, the body expressions are placed inside lambdas, and
/// therefore contextual expressions such as `break`, `continue` and `return`
/// are not valid.
macro_rules! select_merge {
    { $( $var:pat = $event:expr $(; $sub:pat = $dep:expr)* => $body:expr ),+ $(,)? } => {{
        #[allow(clippy::redundant_closure)]
        let r = $crate::select_any!($($crate::rtos::select_map($event, |$var| $body) $(; $sub = $dep)*),+);
        r
    }};
}

#[macro_export]
/// Generates a future event (i.e. one which implements
/// [`crate::rtos::Selectable`]) from a set of events which all have the same
/// result type, by repeated application of [`crate::rtos::select_either`].
macro_rules! select_any {
    ($event:expr $(; $sub:pat = $dep:expr)* $(,)?) => {
        $crate::select_init!($event $(; $sub = $dep)*)
    };
    ($event:expr $(; $sub1:pat = $dep1:expr)*, $($rest:expr $(; $sub:pat = $dep:expr)*),+ $(,)?) => {
        $crate::rtos::select_either($crate::select_init!($event $(; $sub1 = $dep1)*), $crate::select_any!($($rest $(; $sub = $dep)*),+))
    };
}
