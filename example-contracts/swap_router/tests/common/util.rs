// General debugging utilities

use sdk::*;
use super::*;

pub type Result<T, E = TransactResult> = core::result::Result<T, E>;

#[track_caller]
pub fn transact_assert_success<F, T>(f: F) -> T
where
    F: Fn() -> Result<T>,
{
    // (Protect track_caller by avoiding a closure)
    match f() {
        Ok(v) => v,
        Err(r) => {
            print!("Transaction error: ");
            show_result(&r);
            panic!("Fatal: expected transaction success");
        }
    }
}

pub fn exec_tx<F, T>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
) -> Result<T, TransactResult>
where
    F: Fn() -> T + std::panic::RefUnwindSafe,
{
    use std::sync::Arc;
    use std::sync::Mutex;

    let ret: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));

    let r = env.transact(|tx| {
        tx.call(addr).run(|| {
            let ret = Arc::clone(&ret);
            let mut ret = ret.lock().unwrap();
            *ret = Some(f());
        })
    });

    if r.is_success() {
        let ret: Mutex<Option<T>> = Arc::into_inner(ret).unwrap();
        let mut ret: Option<T> = Mutex::into_inner(ret).unwrap();
        Ok(ret.take().unwrap())
    } else {
        Err(r)
    }
}

#[macro_export]
macro_rules! test_transaction {
    ( $( $(#[$attr:meta])* fn $test_name:ident($etx:ident: &Etx) -> Result<()> $body:tt )* ) => {

        $(
            #[test]
            $(#[$attr])*
            fn $test_name() {
                fn body($etx: &$crate::Etx) -> $crate::Result<()> {
                    $body
                }

                transact_assert_success(|| {
                    let etx = setup_etx()?;
                    body(&etx)
                });
            }
        )*
    };
}
