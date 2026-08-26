use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::sync::Once;

#[derive(Debug, Clone, PartialEq)]
pub struct InvokeError {
    pub error_type: InvokeErrorCode,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.error_type)
    }
}

impl Error for InvokeError {}

pub fn deterministic_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Deterministic,
    }
}

pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::OutOfGas,
    }
}

pub fn unrecoverable_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Unrecoverable,
    }
}

/// Panic payload for the non-error control-flow unwinds (`raw_Return` /
/// `raw_Exit` terminating an invocation frame). Despite traveling as an
/// [`InvokeError`], the code is [`InvokeErrorCode::None`]: the frame
/// completed; the panic only unwinds the contract's Rust stack.
pub fn control_flow_unwind() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::None,
    }
}

pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::ValidationFailure,
    }
}

pub fn abort_transaction_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Abandon,
    }
}

// Two counters, not one: the panic hook is active when *either* is non-zero,
// but they are raised by different owners with independently nesting lifetimes
// (`activate`/`deactivate` pairs vs. a guard's `Drop`). Kept apart, each stays
// a count of one thing -- so `panic_hook_depth()` still answers "how many
// frames did this thread activate", and a stray `deactivate_panic_hook` cannot
// decrement a count a live `MockInvocationGuard` is relying on.
thread_local! {
    /// Frames opened on this thread by `activate_panic_hook`.
    ///
    /// Set by the transaction handler around a frame, on the thread that
    /// entered the FFI call.
    static PANIC_HOOK_DEPTH: Cell<usize> = const { Cell::new(0) };

    /// Mock contract bodies currently running on this thread.
    ///
    /// Set by `trampoline` immediately around the body it invokes, which is
    /// what makes it right where `PANIC_HOOK_DEPTH` alone is wrong: execution
    /// is coroutinized, so a body runs on whichever pool thread resumed it
    /// rather than the one that activated -- but `trampoline` and the body it
    /// calls are always the *same* thread, so marking it there needs no
    /// guess about which thread that is. hogd's `MockRuntime` captures its
    /// owning handler for the same reason instead of reading the thread-local
    /// context slot.
    ///
    /// A count, not a flag: a mock body can invoke another contract, which
    /// re-enters `trampoline` on that thread.
    static MOCK_INVOCATION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Marks the calling thread as running a mock contract body for as long as it
/// is held.
///
/// Decrements on drop so an unwinding body still leaves the count balanced.
pub(crate) struct MockInvocationGuard;

impl MockInvocationGuard {
    pub(crate) fn enter() -> Self {
        MOCK_INVOCATION_DEPTH.with(|c| c.set(c.get() + 1));
        MockInvocationGuard
    }
}

impl Drop for MockInvocationGuard {
    fn drop(&mut self) {
        MOCK_INVOCATION_DEPTH.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

static INSTALL_HOOK: Once = Once::new();

fn install_panic_hook() {
    INSTALL_HOOK.call_once(|| {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            if !is_panic_hook_active() {
                prev_hook(panic_info);
                return;
            }

            // While an invocation is active the hook only controls printing;
            // it must not panic or resume_unwind — since rust-lang/rust#158369
            // (nightly-2026-06-26, Rust 1.98) that aborts the process.
            // [`InvokeError`] payloads are expected control flow (aborts,
            // frame unwinds), so they stay silent; anything else is a
            // contract panic, printed compactly here in place of the default
            // hook's output. Either way the original payload keeps unwinding
            // to the enclosing catch site ([`invoke_wrapper`] or the mock
            // frame executor), which maps non-[`InvokeError`] payloads to a
            // deterministic error.
            if panic_info.payload().downcast_ref::<InvokeError>().is_none() {
                _print_panic_info(panic_info);
            }
        }));
    });
}

fn _print_panic_info(panic_info: &std::panic::PanicHookInfo<'_>) {
    if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
        eprintln!("Panic message: {}", payload);
    } else if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
        eprintln!("Panic message: {}", payload);
    } else {
        eprintln!("Panic: {:?}", panic_info.payload());
    }

    if let Some(location) = panic_info.location() {
        eprintln!(
            "Panic occurred at file '{}' line {}",
            location.file(),
            location.line()
        );
    }
}

/// Whether a panic on this thread is expected control flow rather than a
/// harness bug.
///
/// Either a frame was opened on this thread, or this thread is inside a mock
/// body -- the second covers the resumed-coroutine case the first cannot see.
/// Whether a panic on this thread should be treated as coming from contract
/// code, and so routed to the compact hook output rather than the default one.
///
/// True inside an activated frame *or* inside a mock body, since a body can be
/// resumed on a thread that never activated.
pub(crate) fn is_panic_hook_active() -> bool {
    panic_hook_depth() > 0 || in_mock_invocation()
}

/// Whether this thread is currently executing a mock contract body.
pub(crate) fn in_mock_invocation() -> bool {
    MOCK_INVOCATION_DEPTH.with(|c| c.get()) > 0
}

/// How many invocation frames this thread has activated and not yet
/// deactivated. Counts activations only -- a mock body running here does not
/// raise it.
pub(crate) fn panic_hook_depth() -> usize {
    PANIC_HOOK_DEPTH.with(|c| c.get())
}

pub fn activate_panic_hook() {
    install_panic_hook();
    PANIC_HOOK_DEPTH.with(|c| c.set(c.get() + 1));
}

pub fn deactivate_panic_hook() {
    PANIC_HOOK_DEPTH.with(|c| {
        let depth = c.get();
        assert!(
            depth > 0,
            "deactivate_panic_hook called without matching activate"
        );
        c.set(depth - 1);
    });
}

/// Wrap a function (i.e. part of a contract) within a try-catch block that
/// appropriately handles groundhog error codes.
///
/// `Sync` is the load-bearing bound: the wrapper is stored on the contract and
/// invoked through a shared reference from whichever thread resumed the
/// coroutine, so `&F` has to be usable there. Cloning an `Rc` capture from two
/// threads at once would race its refcount, and `Sync` rejects that at the
/// registration site instead. `Send` goes with it because the captures travel
/// with the contract's heap allocation.
pub fn invoke_wrapper<'a, F>(
    wrapped: F,
) -> Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult + Send + Sync + 'a>
where
    F: Fn() + UnwindSafe + Clone + Send + Sync + 'a,
{
    Box::new(move || {
        let w = wrapped.clone();
        let result = panic::catch_unwind(w);

        match result {
            Ok(_) => groundhog_sys::ForeignInvokeResult {
                panic: InvokeErrorCode::None.0,
            },
            Err(err) => {
                if let Some(my_error) = err.downcast_ref::<InvokeError>() {
                    return groundhog_sys::ForeignInvokeResult {
                        panic: my_error.error_type.0,
                    };
                }
                if is_panic_hook_active() {
                    // An ordinary panic out of contract code; the hook
                    // already printed it. Deterministic failure of this
                    // invocation.
                    return groundhog_sys::ForeignInvokeResult {
                        panic: InvokeErrorCode::Deterministic.0,
                    };
                }
                eprintln!("unrecoverable error: {:?}", err);
                groundhog_sys::ForeignInvokeResult {
                    panic: InvokeErrorCode::Unrecoverable.0,
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A contract panic must classify the same way wherever the body runs.
    // `activate_panic_hook` marks the thread that entered the FFI call, but
    // execution is coroutinized and the body resumes on whichever pool thread
    // picked it up; `trampoline` marks that one, which is what this covers.
    #[test]
    fn mock_body_panic_is_deterministic_on_any_thread() {
        // Off the activating thread and outside a body, an ordinary panic is
        // a harness bug -- nothing says a frame is running.
        let unmarked = std::thread::spawn(|| {
            invoke_wrapper(|| panic!("contract panic"))().panic
        })
        .join()
        .expect("harness thread panicked");
        assert_eq!(unmarked, InvokeErrorCode::Unrecoverable.0);

        // Inside a body, on that same foreign thread, it is a deterministic
        // failure of the invocation -- which is the classification the
        // executor needs and a thread-local activation depth cannot give.
        let in_body = std::thread::spawn(|| {
            let _in_body = MockInvocationGuard::enter();
            invoke_wrapper(|| panic!("contract panic"))().panic
        })
        .join()
        .expect("harness thread panicked");
        assert_eq!(in_body, InvokeErrorCode::Deterministic.0);
    }

    #[test]
    fn mock_invocation_guard_balances_on_unwind() {
        assert!(!in_mock_invocation());
        let caught = std::panic::catch_unwind(|| {
            let _in_body = MockInvocationGuard::enter();
            assert!(in_mock_invocation());
            panic!("body panicked");
        });
        assert!(caught.is_err());
        assert!(
            !in_mock_invocation(),
            "guard must decrement when the body unwinds through it"
        );
    }

    #[test]
    fn display_terminates_and_names_the_error_code() {
        // Regression: Display used to format `self.clone()` with `{}`,
        // recursing infinitely and crashing on any to_string().
        let msg = deterministic_error().to_string();
        assert!(msg.contains("InvokeError"), "got: {msg}");
        assert!(msg.contains("Deterministic"), "got: {msg}");
    }

    #[test]
    fn hook_never_panics_and_leaves_payload_untouched() {
        // Regression for rust-lang/rust#158369 (first in nightly-2026-06-26):
        // the hook used to resume_unwind a converted payload from inside the
        // hook, which now aborts the whole process. Merely completing this
        // test proves the hook no longer panics while active.
        install_panic_hook();

        activate_panic_hook();
        let err =
            panic::catch_unwind(|| panic!("contract panic")).unwrap_err();
        assert_eq!(
            err.downcast_ref::<&str>(),
            Some(&"contract panic"),
            "the hook must let the original payload unwind unchanged; \
             conversion is the catch site's job"
        );
        deactivate_panic_hook();
    }

    #[test]
    fn invoke_wrapper_converts_panics_when_active() {
        // With activation: an ordinary contract panic becomes a
        // deterministic error at the catch site.
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);
        let result = invoke_wrapper(|| panic!("contract panic"))();
        assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

        // InvokeError payloads pass through with their own code.
        let result =
            invoke_wrapper(|| std::panic::panic_any(validation_error()))();
        assert_eq!(result.panic, InvokeErrorCode::ValidationFailure.0);

        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 0);

        // Without activation: an ordinary panic is a harness bug, not a
        // contract failure.
        let result = invoke_wrapper(|| panic!("after deactivate"))();
        assert_eq!(result.panic, InvokeErrorCode::Unrecoverable.0);
    }

    #[test]
    fn nested_activation_stays_active_after_inner_deactivate() {
        install_panic_hook();
        assert_eq!(panic_hook_depth(), 0);

        // Outer activate
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);

        // Inner activate (simulating nested contract call)
        activate_panic_hook();
        assert_eq!(panic_hook_depth(), 2);

        // Inner deactivate — hook must still be active
        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 1);
        assert!(is_panic_hook_active());

        // A panic here must still count as a contract failure (this is the
        // bug scenario: before the fix, the inner deactivate would have
        // turned off the hook)
        let result = invoke_wrapper(|| panic!("mid-nest panic"))();
        assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

        // Outer deactivate
        deactivate_panic_hook();
        assert_eq!(panic_hook_depth(), 0);
        assert!(!is_panic_hook_active());
    }

    #[test]
    #[should_panic(
        expected = "deactivate_panic_hook called without matching activate"
    )]
    fn deactivate_without_activate_panics() {
        assert_eq!(panic_hook_depth(), 0);
        deactivate_panic_hook();
    }
}
