#![allow(dead_code)]
// Much of this debugging code is dead with the debugging feature disabled.

use core::any::Any;
use core::fmt::{self, Display, Write};
use core::panic::Location;
use core::ptr::NonNull;
use heapless::String;

pub trait Context: Any + Display {}

impl<T> Context for T where T: Any + Display {}

struct Frame {
    context: NonNull<dyn Context>,
    location: String<100>,
    next: Option<NonNull<Frame>>,
}

// On wasm (single-threaded), use a plain static with ContractCell (which is Sync).
// On native (multi-threaded), use thread_local with core::cell::Cell.
// Both provide .get() and .set() for Copy types.
#[cfg(any(doc, target_family = "wasm"))]
static FRAMES: crate::ContractCell<Option<NonNull<Frame>>> =
    crate::ContractCell::new(None);

#[cfg(not(any(doc, target_family = "wasm")))]
std::thread_local! {
    static FRAMES: core::cell::Cell<Option<NonNull<Frame>>> =
        const { core::cell::Cell::new(None) };
}

/// Add to the stack of debugging-context values.
#[track_caller]
pub fn with_debug_context<F, T>(context: &(impl Context + 'static), f: F) -> T
where
    F: FnOnce() -> T,
{
    let location = Location::caller();

    let mut location_str = String::new();
    write!(location_str, "{}", location)
        .unwrap_or_else(|e| panic!("Failed rendering location: {}", e));

    let frames = FRAMES.get();

    let frame = Frame {
        context: NonNull::from(context),
        location: location_str,
        next: frames,
    };

    FRAMES.set(Some(NonNull::from(&frame)));

    let _guard = FramesGuard { prev: frames };

    f()
}

struct FramesGuard {
    prev: Option<NonNull<Frame>>,
}

impl Drop for FramesGuard {
    fn drop(&mut self) {
        FRAMES.set(self.prev);
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Safety: Frame is only displayed via Backtrace::fmt, which runs in a
        // non-returning abort context. All `with_debug_context` callers are still
        // on the stack, so their `context` references remain live.
        let context = unsafe { self.context.as_ref() };
        write!(f, "{} at {}", context, &self.location)
    }
}

#[derive(Copy, Clone)]
pub struct Backtrace {
    top: Option<NonNull<Frame>>,
}

impl Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut frames = self.top;
        let mut depth = 1; // depth 0 is the abort call itself

        while let Some(frame) = frames {
            // Safety: After a backtrace is captured, we are in the non-returning context of
            // `abort`, so all reachable frames will be valid until the end of execution.
            let frame = unsafe { frame.as_ref() };

            writeln!(f, "  {}: {}", depth, frame)?;

            frames = frame.next;
            depth += 1;
        }

        Ok(())
    }
}

impl Backtrace {
    pub fn capture() -> Self {
        Backtrace { top: FRAMES.get() }
    }
}
