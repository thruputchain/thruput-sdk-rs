use core::any::Any;
use core::fmt::{self, Display, Write};
use core::panic::Location;
use core::ptr::NonNull;
use heapless::String;

pub trait Context: Any + Display {}

impl<T> Context for T where T: Any + Display {}

struct Frame {
    context: NonNull<dyn Context>,
    location: LocString,
    next: Option<NonNull<Frame>>,
}

type LocString = String<100>;

// On guest targets (single-threaded), use a plain static with ContractCell (which is Sync).
// On native (multi-threaded), use thread_local with core::cell::Cell.
// Both provide .get() and .set() for Copy types.
#[cfg(any(doc, target_family = "wasm", target_arch = "riscv32"))]
static FRAMES: crate::ContractCell<Option<NonNull<Frame>>> =
    crate::ContractCell::new(None);

#[cfg(not(any(doc, target_family = "wasm", target_arch = "riscv32")))]
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
    let location_str = write_location(Location::caller());

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

// Render the location to a fixed-length string, truncating the front of the
// filepath (and prepending an elipsis) if necessary.
//
fn write_location(loc: &Location) -> LocString {
    let mut out = LocString::new();
    let mut rem = out.capacity();

    let mut line_col = String::<16>::new();
    write!(line_col, ":{}:{}", loc.line(), loc.column()).unwrap();
    rem -= line_col.len();

    let file = loc.file();
    if file.len() > rem {
        // Truncate front of filepath
        let elipsis = "...";
        out.write_str(elipsis).unwrap();
        rem -= elipsis.len();
        out.write_str(&file[file.len() - rem..]).unwrap();
    } else {
        out.write_str(file).unwrap();
    }

    out.write_str(line_col.as_str()).unwrap();

    out
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

/// A snapshot of the `with_debug_context` frame stack.
///
/// The frames live on the stacks of the `with_debug_context` callers, so a
/// `Backtrace` is only valid to display while all of those calls are still
/// in progress. It is deliberately crate-private, non-`Copy`/`Clone`, and
/// only constructible via the unsafe [`Backtrace::capture`] so it cannot
/// escape the abort paths that uphold that invariant.
pub(crate) struct Backtrace {
    top: Option<NonNull<Frame>>,
}

impl Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut frames = self.top;
        let mut depth = 1; // depth 0 is the abort call itself

        while let Some(frame) = frames {
            // Safety: upheld by the caller of `Backtrace::capture`, which
            // promises the backtrace is only displayed while every captured
            // `with_debug_context` frame is still on the stack.
            let frame = unsafe { frame.as_ref() };

            writeln!(f, "  {}: {}", depth, frame)?;

            frames = frame.next;
            depth += 1;
        }

        Ok(())
    }
}

impl Backtrace {
    /// Capture the current debug-context frame stack.
    ///
    /// # Safety
    ///
    /// The returned value borrows the stack frames of every
    /// `with_debug_context` call in progress, but carries no lifetime.
    /// The caller must not let it outlive those calls: capture and display
    /// it within a context that never returns into them (i.e. an abort
    /// path), and do not store it.
    pub(crate) unsafe fn capture() -> Self {
        Backtrace { top: FRAMES.get() }
    }
}
