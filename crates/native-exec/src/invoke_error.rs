use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::sync::Once;

/// Panic payload classifying how an invocation frame terminated; downcast
/// at the frame's catch site and mapped to the frame outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct InvokeError {
    /// The code the catch site reports to the executor.
    pub error_type: InvokeErrorCode,
}

impl fmt::Display for InvokeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InvokeError {}", self.error_type)
    }
}

impl Error for InvokeError {}

/// Payload for a deterministic contract failure.
pub fn deterministic_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Deterministic,
    }
}

/// Payload for an out-of-gas termination.
pub fn out_of_gas_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::OutOfGas,
    }
}

/// Payload for an unrecoverable harness/FFI failure (not a contract fault).
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

/// Payload for a validation failure.
pub fn validation_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::ValidationFailure,
    }
}

/// Payload for abandoning the whole transaction.
pub fn abort_transaction_error() -> InvokeError {
    InvokeError {
        error_type: InvokeErrorCode::Abandon,
    }
}

thread_local! {
    /// Mock contract bodies currently running on this thread.
    ///
    /// Set by `trampoline` immediately around the body it invokes, which is
    /// what makes it right where a mark on the dispatching thread is wrong:
    /// execution is coroutinized, so a body runs on whichever pool thread
    /// resumed it -- but `trampoline` and the body it calls are always the
    /// *same* thread, so marking it there needs no guess about which thread
    /// that is. hogd's `MockRuntime` captures its owning handler for the same
    /// reason instead of reading the thread-local context slot.
    ///
    /// A count, not a flag: a mock body can invoke another contract, which
    /// re-enters `trampoline` on that thread.
    static PANIC_HOOK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Activates the classifying panic hook on the calling thread for as long as
/// it is held, installing it on first use.
///
/// Held by `trampoline` around the contract body it invokes. That is the whole
/// region the hook should cover: a panic anywhere else is a harness bug, and
/// should keep the default output and stay unrecoverable.
///
/// Decrements on drop so an unwinding body still leaves the count balanced.
pub struct PanicHookGuard;

impl PanicHookGuard {
    /// Enter a contract body on the calling thread.
    pub fn enter() -> Self {
        install_panic_hook();
        PANIC_HOOK_DEPTH.with(|c| c.set(c.get() + 1));
        PanicHookGuard
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        PANIC_HOOK_DEPTH.with(|c| c.set(c.get().saturating_sub(1)));
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

/// Whether this thread is inside a contract body, and so whether a panic on it
/// is contract code failing rather than a harness bug.
pub(crate) fn is_panic_hook_active() -> bool {
    PANIC_HOOK_DEPTH.with(|c| c.get()) > 0
}

/// Wrap a contract closure in the catch site that maps its panics to
/// groundhog invoke error codes.
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

    // A contract panic must classify the same way wherever the body runs:
    // execution is coroutinized, so a body resumes on whichever pool thread
    // picked it up rather than the one that dispatched to it. `trampoline`
    // marks that thread, which is what this covers.
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
            let _in_body = PanicHookGuard::enter();
            invoke_wrapper(|| panic!("contract panic"))().panic
        })
        .join()
        .expect("harness thread panicked");
        assert_eq!(in_body, InvokeErrorCode::Deterministic.0);
    }

    #[test]
    fn panic_hook_guard_balances_on_unwind() {
        assert!(!is_panic_hook_active());
        let caught = std::panic::catch_unwind(|| {
            let _in_body = PanicHookGuard::enter();
            assert!(is_panic_hook_active());
            panic!("body panicked");
        });
        assert!(caught.is_err());
        assert!(
            !is_panic_hook_active(),
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
        let _in_body = PanicHookGuard::enter();
        let err =
            panic::catch_unwind(|| panic!("contract panic")).unwrap_err();
        assert_eq!(
            err.downcast_ref::<&str>(),
            Some(&"contract panic"),
            "the hook must let the original payload unwind unchanged; \
             conversion is the catch site's job"
        );
    }

    #[test]
    fn invoke_wrapper_converts_panics_inside_a_body() {
        {
            // Inside a body: an ordinary contract panic becomes a
            // deterministic error at the catch site.
            let _in_body = PanicHookGuard::enter();
            let result = invoke_wrapper(|| panic!("contract panic"))();
            assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

            // InvokeError payloads pass through with their own code.
            let result =
                invoke_wrapper(|| std::panic::panic_any(validation_error()))();
            assert_eq!(result.panic, InvokeErrorCode::ValidationFailure.0);
        }

        // Outside one, an ordinary panic is a harness bug, not a contract
        // failure.
        assert!(!is_panic_hook_active());
        let result = invoke_wrapper(|| panic!("outside a body"))();
        assert_eq!(result.panic, InvokeErrorCode::Unrecoverable.0);
    }

    #[test]
    fn nested_body_stays_active_after_inner_guard_drops() {
        assert!(!is_panic_hook_active());

        let outer = PanicHookGuard::enter();
        {
            // A mock body invoking another contract re-enters `trampoline`.
            let _inner = PanicHookGuard::enter();
        }

        // The inner drop must not end the outer body: a panic here is still
        // a contract failure. This is why the marker counts rather than
        // flags.
        assert!(is_panic_hook_active());
        let result = invoke_wrapper(|| panic!("mid-nest panic"))();
        assert_eq!(result.panic, InvokeErrorCode::Deterministic.0);

        drop(outer);
        assert!(!is_panic_hook_active());
    }
}
