use sdk::contract_test;
use sdk::storage::error::Result;
use sdk::storage::keypath::*;
use sdk::storage::load::*;
use sdk::storage::store::*;
use sdk::storage::nnint::*;
use sdk::SyscallErrorCode;

#[derive(Debug)]
struct Data {
    num: i32,
    nnint: NNInt,
}

impl DefaultFromKeypath for Data {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        let mut kp = keypath.clone();

        kp.enter_seq();
        let num = DefaultFromKeypath::default_from_keypath(&kp);

        kp.seq_next();
        let nnint = DefaultFromKeypath::default_from_keypath(&kp);

        Data { num, nnint }
    }
}

impl LoadKeypath for Data {
    fn load_keypath(keypath: &Keypath) -> Result<Self> {
        let mut kp = keypath.clone();

        kp.enter_seq();
        let num = LoadKeypath::load_keypath(keypath)?;

        kp.seq_next();
        let nnint = LoadKeypath::load_keypath(keypath)?;

        Ok(Data { num, nnint })
    }
}

// derive
const _: () = {
    use xdrs::{WriteXDR, Writer, WriteResult, write_xdr};

    impl WriteXDR for Data {
        fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult {
            write_xdr(&self.num, w)?;
            write_xdr(&self.nnint, w)?;
            Ok(())
        }
    }
};

contract_test!(test_path {
    native_exec::test_environment::TestEnvironment::enable_syscall_logging();

    let data_keypath = Keypath::new();

    let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);

    data.store_keypath(&data_keypath).unwrap();

    let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    let v = data.nnint.try_get()
        .unwrap_or_else(|e| {
            let code: Result<SyscallErrorCode, _> = e.0.try_into();
            panic!("try_get failed: {code:?}");
        })
        .unwrap_or(0);
    assert_eq!(0, v);

    data.nnint.add(72).unwrap();
    data.store_keypath(&data_keypath).unwrap();

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    assert_eq!(72, data.nnint.get().unwrap());
});
