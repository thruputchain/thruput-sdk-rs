use hogexec_internal::ffi_enums::InvokeErrorCode;
use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::panic;
use std::panic::UnwindSafe;
use std::sync::Once;

/// Panic payload classifying how an invocation frame terminated.
///
/// Thrown with `panic_any` and downcast at the frame's catch site
/// ([`invoke_wrapper`] or the mock frame executor), which maps the code to
/// the frame outcome. An ordinary (non-`InvokeError`) panic payload is
/// classified as a deterministic contract failure instead.
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

/// Panic payload for harness misuse: a test driving the mock-frame API
/// outside its contract (executing an already-closed frame, closing a frame
/// over an open child, ...). Unlike an ordinary message panic — which the
/// frame executor records as a deterministic fault of the enclosing frame —
/// this payload is re-raised by the catch sites, so the misuse always fails
/// the test instead of surfacing as contract behavior.
#[derive(Debug)]
pub struct HarnessMisuse(pub String);

impl fmt::Display for HarnessMisuse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "harness misuse: {}", self.0)
    }
}

/// Panic with a [`HarnessMisuse`] payload carrying `msg`.
///
/// Installs the panic hook first so the message is printed even when the
/// misuse fires outside any active frame (the payload itself is opaque to
/// the default hook).
#[track_caller]
pub(crate) fn harness_misuse_panic(msg: impl Into<String>) -> ! {
    install_panic_hook();
    panic::panic_any(HarnessMisuse(msg.into()));
}

thread_local! {
    static PANIC_HOOK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

static INSTALL_HOOK: Once = Once::new();

fn install_panic_hook() {
    INSTALL_HOOK.call_once(|| {
        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            // A HarnessMisuse payload is opaque to the default hook (it would
            // print only "Box<dyn Any>"), so print its message here whether
            // or not a frame is active — misuse must self-diagnose.
            if let Some(misuse) =
                panic_info.payload().downcast_ref::<HarnessMisuse>()
            {
                eprintln!("{misuse}");
                if let Some(location) = panic_info.location() {
                    eprintln!(
                        "Panic occurred at file '{}' line {}",
                        location.file(),
                        location.line()
                    );
                }
                return;
            }
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

pub(crate) fn is_panic_hook_active() -> bool {
    panic_hook_depth() > 0
}

pub(crate) fn panic_hook_depth() -> usize {
    PANIC_HOOK_DEPTH.with(|c| c.get())
}

/// Enter a contract-execution region: install the classifying panic hook
/// (once) and increment its activation depth. Paired with
/// [`deactivate_panic_hook`].
pub fn activate_panic_hook() {
    install_panic_hook();
    PANIC_HOOK_DEPTH.with(|c| c.set(c.get() + 1));
}

/// Leave a contract-execution region entered by [`activate_panic_hook`].
///
/// Panics if called without a matching activation.
pub fn deactivate_panic_hook() {
    PANIC_HOOK_DEPTH.with(|c| {
        let depth = c.get();
        assert!(
            depth > 0,
            "deactivate_panic_hook called without matching activate"
        );
        c.set(depth.saturating_sub(1));
    });
}

/// Wrap a contract closure in the catch site that maps its panics to
/// groundhog invoke error codes ([`InvokeError`] payloads keep their code;
/// ordinary panics become deterministic failures while the hook is active).
pub fn invoke_wrapper<'a, F>(
    wrapped: F,
) -> Box<dyn Fn() -> groundhog_sys::ForeignInvokeResult + 'a>
where
    F: Fn() + UnwindSafe + Clone + 'a,
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
