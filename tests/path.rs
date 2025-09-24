use sdk::contract_test;
use sdk::*;
use sdk::storage::*;
use sdk::storage::error::Result;

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
        // kp.leave_seq();

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
        // kp.leave_seq();

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
    let v = data.nnint.get().unwrap();
    assert_eq!(0, v);

    data.nnint.add_default_zero(72).unwrap();
    data.store_keypath(&data_keypath).unwrap();

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    assert_eq!(72, data.nnint.get().unwrap());

    println!("test_path OK");

    return ();
});
