
pub mod invoke_error;
pub mod host_fn_error;
pub mod bindings;
pub use host_fn_error::perform_mocked_syscall;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        let _ = types::TestEnvironment::new();
    }
}