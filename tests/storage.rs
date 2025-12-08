//! Test "storage" datatypes: keypaths, handles, vectors and maps

use sdk::Storage;
use sdk::contract_test;
use sdk::storage::*;

#[derive(Debug, xdrs::WriteXDR, Storage)]
struct Data {
    num: i32,
    nums: Vector<u64>,
    nnints: Vector<NNInt>,
    bytes: Bytestring,
    map: Map<u32, NNInt>,
    map2: Map<u32, i32>,
}

contract_test!(test_corner_case_paths {
    use hex_literal::hex;

    { // 0 bytes
        let path = Keypath::new();
        let key = path.generate_key();
        assert_eq!(&*key, &hex!("00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
    }
    { // 1 byte
        let mut path = Keypath::new();
        path.push_bytes(b"@");
        let key = path.generate_key();
        assert_eq!(&*key, &hex!("0140ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
    }
    { // 31 bytes
        let mut path = Keypath::new();
        path.push_bytes(b"@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
        let key = path.generate_key();
        assert_eq!(&*key, &hex!("1f40404040404040404040404040404040404040404040404040404040404040"));
    }
    {
        let mut path = Keypath::new();
        // 32 bytes
        path.push_bytes(b"@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
        let key = path.generate_key();
        assert_eq!(&*key, &hex!("92b7eb5290d8d6e3ac79215cb4bdb07fe89629ee720be4332b3daa842b7ec80a"));
    }
    {
        let mut path = Keypath::new();
        // 33 bytes
        path.push_bytes(b"@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@");
        let key = path.generate_key();
        assert_eq!(&*key, &hex!("fa80cb94fa63bcef8ef2076d2f754cc374927fb02e2316174494e013325560bd"));
    }
});

contract_test!(test_map {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath).unwrap();
    }

    let slot: u32 = 3;
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut cell: NNInt = data.map.get(&slot).unwrap();

        // Below results in `set_add(None, 7)` on this
        // previously-unset location
        cell.checked_inc(7).unwrap();
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let cell = data.map.get(&slot).unwrap();
        let v = cell.get().unwrap();
        assert_eq!(v, 7);
    }
});

contract_test!(test_map_entry {
    let data_keypath = Keypath::new();
    let slot: u32 = 3;

    // Check that the data is absent
    {
        let data: Option<Data> = LoadKeypath::try_load_keypath(&data_keypath).unwrap();
        assert!(data.is_none())
    }

    // Store the default value
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath).unwrap();
    }

    // Check that the entry starts empty
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let entry = data.map2.entry(&slot).unwrap_or_else(|e|
            panic!("entry(): {}", e));

        assert_eq!(*entry.get(), None);
    }

    // Check that the entry starts empty (another way)
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot).unwrap();

        assert_eq!(opt, None);
    }

    let mut value: i32 = -14;

    // Set the entry via `Map::set`
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map2.set(&slot, &value).unwrap();
    }

    // Check that the entry has the value
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot).unwrap();
        assert_eq!(opt, Some(value));
    }

    value += 1;

    // Set the entry via `Entry::set`
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut entry = data.map2.entry(&slot).unwrap();

        entry.set(&value);
    }

    // Check that the entry has the value
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot).unwrap();
        assert_eq!(opt, Some(value));
    }

    value += 1;

    // Set the entry via `Entry::get_mut`
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut entry = data.map2.entry(&slot).unwrap_or_else(|e|
            panic!("entry(): {}", e));

        *entry.get_mut() = Some(value);
    }

    // Check that the entry has the value
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot).unwrap();
        assert_eq!(opt, Some(value));
    }
});

contract_test!(test_storable {
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
        nnint.checked_inc(n as i128).unwrap();
    }
    drop(data);

    let data = handle.load().unwrap();
    for i in 0..3 {
        let nnint = data.nnints.get_default(i)
            .unwrap_or_else(|e| {
                panic!("try_get {i} failed: {e}");
            });
        let n = nnint.get().unwrap() as u64;
        let n_expected = 72 + i;
        assert_eq!(n, n_expected);
    }
});

contract_test!(test_bytestring {
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
    let mut vec: Vector<u32> = Vector::new_from(Keypath::root(), 0);

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
