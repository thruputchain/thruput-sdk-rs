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
    map3: Map<u32, Thing>,
}

#[derive(Debug, xdrs::WriteXDR, Storage)]
struct Thing {
    num: i32,
    bytes: Bytestring,
}

contract_test!(test_map {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath).unwrap();
    }

    let slot: u32 = 3;
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut cell: NNInt = data.map.get(&slot);

        // Below results in `set_add(None, 7)` on this
        // previously-unset location
        cell.strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let cell = data.map.get(&slot);
        let v = cell.get();
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
        let opt = data.map2.try_get(&slot);

        assert_eq!(opt, None);
    }

    let mut value: i32 = -14;

    // Set the entry via `Map::set`
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map2.set(&slot, &value);
    }

    // Check that the entry has the value
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot);
        assert_eq!(opt, Some(value));
    }

    value += 1;

    // Set the entry via `Entry::set`
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut entry = data.map2.entry(&slot).unwrap();

        entry.set(&value).unwrap();
    }

    // Check that the entry has the value
    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let opt = data.map2.try_get(&slot);
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
        let opt = data.map2.try_get(&slot);
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
        assert_eq!(nnint.try_get(), None);

        let n = 72 + i;
        nnint.strict_inc(n as i128);
    }
    drop(data);

    let data = handle.load().unwrap();
    for i in 0..3 {
        let nnint = data.nnints.get_default(i)
            .unwrap_or_else(|e| {
                panic!("try_get {i} failed: {e}");
            });
        let n = nnint.get() as u64;
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

/* TODO:
contract_test!(test_thing_map {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath).unwrap();
    }

    let slot: u32 = 3;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let mut thing_entry = data.map3.entry(&slot).unwrap();
        let maybe_thing = thing_entry.get_mut();

        if let Some(thing) = maybe_thing.as_mut() {
            thing.num += 1;
        } else {
            *maybe_thing = Some(Thing
    }
});
*/
