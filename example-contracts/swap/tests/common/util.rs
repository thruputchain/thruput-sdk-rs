// General debugging utilities

use sdk::*;
use sdk::types::event::TxEmittedEvent;

pub type Result<T, E = TransactResult> = core::result::Result<T, E>;

pub fn transact_assert_success<F, T>(f: F) -> T
where
    F: Fn() -> Result<T>,
{
    f().unwrap_or_else(|r| {
        r.assert_success();
        unreachable!();
    })
}

pub fn exec_tx<F, T>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
) -> Result<T, TransactResult>
where
    F: Fn() -> T + std::panic::RefUnwindSafe,
{
    let (r, v) = _exec_tx(env, addr, f);

    if r.is_success() {
        Ok(v.unwrap())
    } else {
        Err(r)
    }
}

pub fn exec_tx2<F, T, E>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
    event_handler: E,
) -> Result<T, TransactResult>
where
    F: Fn() -> T + std::panic::RefUnwindSafe,
    E: Fn(&TxEmittedEvent),
{
    let (r, v) = _exec_tx(env, addr, f);

    if r.is_success() {
        for e in r.events() {
            event_handler(e);
        }
    }

    if r.is_success() {
        Ok(v.unwrap())
    } else {
        Err(r)
    }
}

pub fn _exec_tx<F, T>(
    env: &TestEnvironmentHandle,
    addr: &ContractAddress,
    f: F,
) -> (TransactResult, Option<T>)
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

    let ret: Mutex<Option<T>> = Arc::into_inner(ret).unwrap();
    let mut ret: Option<T> = Mutex::into_inner(ret).unwrap();
    let v = ret.take();

    (r, v)
}
