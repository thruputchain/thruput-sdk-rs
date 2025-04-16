use sdk_rs as sdk;
use sdk::contract_test;
use sdk::storage_prelude::*;

#[derive(Debug, Serialize, Storage)]
struct Data {
    num: i32,
    nums: Vector<u64>,
    nnints: Vector<NNInt>,
    bytes: Bytestring,
}

contract_test!(test_storable {
    use sdk::storage_prelude::*;

    let data_keypath = Keypath::new();
    let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
    data.store_keypath(&data_keypath).unwrap();

    let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();

    for i in 0..3 {
        let n = i;
        data.nums.push(n).unwrap();
    }

    data.store_keypath(&data_keypath).unwrap();

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    for i in 0..3 {
        let n = data.nums.get(i).unwrap();
        assert_eq!(n, i);
    }
});

contract_test!(test_storable_lazy {
    use sdk::storage_prelude::*;

    let data_keypath = Keypath::new();
    let mut handle: Handle<Data> = Handle::new(&data_keypath);

    let mut data = handle.load_mut().unwrap();
    for i in 0..3 {
        let n = i;
        data.nums.push(n).unwrap();
    }
    for i in 0..3 {
        let n = data.nums.get(i).unwrap();
        assert_eq!(n, i);
    }
    drop(data); // flush changes

    let data = handle.load().unwrap();
    for i in 0..3 {
        let n = data.nums.get(i).unwrap();
        assert_eq!(n, i);
    }

    assert!(data.nums.get(100).is_err());
});

contract_test!(test_load_default {
    use sdk::storage_prelude::*;

    let data_keypath = Keypath::new();
    let mut handle: Handle<Data> = Handle::new(&data_keypath);

    let mut data = handle.load_mut().unwrap();
    data.nnints.extend_len(3).unwrap();
    drop(data); // flush changes

    let data = handle.load().unwrap();
    for i in 0..3 {
        let mut nnint = data.nnints.get_default(i).unwrap();
        assert_eq!(nnint.try_get().unwrap(), None);

        let n = 72 + i;
        nnint.add_default_zero(n as i128).unwrap();
    }
    drop(data);

    let data = handle.load().unwrap();
    for i in 0..3 {
        let nnint = data.nnints.get_default(i).unwrap();
        let n = nnint.get().unwrap() as u64;
        let n_expected = 72 + i;
        assert_eq!(n, n_expected);
    }
});

contract_test!(test_bytestring {
    use sdk::storage_prelude::*;

    let mut handle: Handle<Data> = Handle::root();

    let data = handle.load().unwrap();
    let expected = b"";
    assert_eq!(expected.len(), data.bytes.len());
    assert_eq!(expected, data.bytes.get().as_ref());

    let mut data = handle.load_mut().unwrap();
    assert_eq!(0, data.bytes.len());
    let contents = b"Bytestring contents";
    data.bytes.set(contents);
    assert_eq!(contents.len(), data.bytes.len());
    assert_eq!(contents, data.bytes.get().as_ref());
    drop(data);

    let data = handle.load().unwrap();
    assert_eq!(contents.len(), data.bytes.len());
    assert_eq!(contents, data.bytes.get().as_ref());
});

contract_test!(test_vector {
    use sdk::storage_prelude::*;

    let mut vec: Vector<u32> = Vector::from(Keypath::root());

    assert_eq!(0, vec.len());
    for i in 0..3 {
        vec.push(i).unwrap();
    }

    assert_eq!(3, vec.len());
    for i in 0..3 {
        assert_eq!(i as u32, vec.get(i).unwrap());
    }
});

contract_test!(test_handled_vector {
    use sdk::storage_prelude::*;

    let mut handle: Handle<Vector<u32>> = Handle::root();

    let vec = handle.load().unwrap();
    assert_eq!(0, vec.len());
    drop(vec);

    let mut vec = handle.load_mut().unwrap();
    assert_eq!(0, vec.len());
    for i in 0..3 {
        vec.push(i).unwrap();
    }
    drop(vec);

    let vec = handle.load().unwrap();
    assert_eq!(3, vec.len());
    for i in 0..3 {
        assert_eq!(i as u32, vec.get(i).unwrap());
    }
    drop(vec);
});

contract_test!(test_vector_data {
    use sdk::storage_prelude::*;

    let mut handle: Handle<Vector<Data>> = Handle::root();

    let vec = handle.load().unwrap();
    assert_eq!(0, vec.len());
    drop(vec);

    let mut vec = handle.load_mut().unwrap();
    assert_eq!(0, vec.len());
    vec.extend_len(3).unwrap();
    for i in 0..3 {
        // let mut dh = vec.get_handle(i).unwrap();
        // let mut d = dh.load_mut().unwrap();
        let mut d = vec.load_mut(i).unwrap();
        d.num = i as _;
    }
    drop(vec);

    let vec = handle.load().unwrap();
    assert_eq!(3, vec.len());
    for i in 0..3 {
        let d = vec.get_default(i).unwrap();
        assert_eq!(i, d.num as _);
    }
    drop(vec);
});
