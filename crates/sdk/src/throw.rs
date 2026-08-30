/// Extension trait for replacing an error with a contract exception.
pub trait OrThrow<T> {
    /// On `Err`, throw `exn` as a contract exception. On `Ok`, return the value.
    fn or_throw<E: crate::Exn>(self, exn: E) -> T;
}

impl<T, Err> OrThrow<T> for Result<T, Err> {
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn or_throw<E: crate::Exn>(self, exn: E) -> T {
        match self {
            Ok(v) => v,
            Err(_) => throw(exn),
        }
    }
}

#[cfg_attr(not(target_arch = "riscv32"), track_caller)]
pub fn throw<E>(exn: E) -> !
where
    E: crate::Exn,
{
    let exn = crate::exn::cross_exn::ContractExn::new(exn);
    crate::contract_abort_encoded(&exn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exn::Exn;
    use crate::xdr::*;

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
    #[repr(i32)]
    enum TestExn {
        Boom = 1,
    }
    impl Exn for TestExn {
        const SELECTOR: u64 = 0xDEAD;
    }

    #[test]
    fn or_throw_ok_returns_value() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.or_throw(TestExn::Boom), 42);
    }
}
