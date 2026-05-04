// General debugging utilities

use sdk::*;

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
            r.assert_success();
            unreachable!();
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
