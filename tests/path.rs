use sdk::contract_test;
use sdk::storage::error::Result;
use sdk::storage::keypath::*;
use sdk::storage::load::*;
use sdk::storage::store::*;
use sdk::storage::nnint::*;
use sdk::SyscallErrorCode;
use xdrs::WriteXDR;

#[derive(Debug, WriteXDR)]
struct Data {
    num: i32,
    nnint: NNInt,
}

impl DefaultFromKeypath for Data {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        let mut kp = keypath.clone();

        kp.seq_begin();
        let num = DefaultFromKeypath::default_from_keypath(&kp);

        kp.seq_next();
        let nnint = DefaultFromKeypath::default_from_keypath(&kp);

        Data { num, nnint }
    }
}

const _: () = {
    use xdrs::ReadResult;
    use sdk::storage::read_xdr_path::*;

    impl ReadXDRPath for Data {
        fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
        where
            R: ReaderPath<'r, Keypath>
        {
            r.seq_begin();
            let num = ReadXDRPath::read_xdr_path(r)?;

            r.seq_next();
            let nnint = ReadXDRPath::read_xdr_path(r)?;

            r.seq_end();
            Ok(Data { num, nnint })
        }
    }
};

contract_test!(test_path {
    native_exec::test_environment::TestEnvironment::enable_syscall_logging();

    let data_keypath = Keypath::new();

    let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);

    data.store_keypath(&data_keypath).unwrap();

    let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

    let n = data.nnint.try_get()
        .unwrap_or_else(|e| {
            let code: Result<SyscallErrorCode, _> = e.0.try_into();
            panic!("try_get failed: {code:?}");
        })
        .unwrap_or(0);
    assert_eq!(0, n);

    data.nnint.add(72).unwrap();
    data.store_keypath(&data_keypath).unwrap();

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    assert_eq!(72, data.nnint.get().unwrap());
});
