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
        data.store_keypath(&data_keypath);
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
        data.store_keypath(&data_keypath);
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

        entry.set(&value);
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
    data.store_keypath(&data_keypath);

    let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    for i in 0..3 {
        let n = i;
        data.nums.push(n);
    }
    data.store_keypath(&data_keypath);

    let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
    for i in 0..3 {
        let n = data.nums.get(i);
        assert_eq!(n, i);
    }
});

contract_test!(test_storable_lazy {
    let data_keypath = Keypath::new();
    let mut handle: Handle<Data> = Handle::new(&data_keypath);

    let mut data = handle.load_mut().unwrap();
    for i in 0..3 {
        let n = i;
        data.nums.push(n);
    }
    for i in 0..3 {
        let n = data.nums.get(i);
        assert_eq!(n, i);
    }
    drop(data); // flush changes

    let data = handle.load().unwrap();
    for i in 0..3 {
        let n = data.nums.get(i);
        assert_eq!(n, i);
    }

    assert!(data.nums.try_get(100).is_none());
});

contract_test!(test_load_default {
    let data_keypath = Keypath::new();
    let mut handle: Handle<Data> = Handle::new(&data_keypath);

    let mut data = handle.load_mut().unwrap();
    data.nnints.extend_len(3);
    drop(data); // flush changes

    let data = handle.load().unwrap();
    for i in 0..3 {
        let mut nnint = data.nnints.get_default(i);
        assert_eq!(nnint.try_get(), None);

        let n = 72 + i;
        nnint.strict_inc(n as i128);
    }
    drop(data);

    let data = handle.load().unwrap();
    for i in 0..3 {
        let nnint = data.nnints.get_default(i);
        let n = nnint.get() as u64;
        let n_expected = 72 + i;
        assert_eq!(n, n_expected);
    }
});

/* XXX
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
*/

contract_test!(test_vector {
    let mut vec: Vector<u32> = Vector::new_from(Keypath::root(), 0);

    assert_eq!(0, vec.len());
    for i in 0..3 {
        vec.push(i);
    }

    assert_eq!(3, vec.len());
    for i in 0..3 {
        assert_eq!(i as u32, vec.get(i));
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
        vec.push(i);
    }
    drop(vec);

    let vec = handle.load().unwrap();
    assert_eq!(3, vec.len());
    for i in 0..3 {
        assert_eq!(i as u32, vec.get(i));
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
    vec.extend_len(3);
    for i in 0..3 {
        // let mut dh = vec.get_handle(i).unwrap();
        // let mut d = dh.load_mut().unwrap();
        let mut d = vec.load_mut(i);
        d.num = i as _;
    }
    drop(vec);

    let vec = handle.load().unwrap();
    assert_eq!(3, vec.len());
    for i in 0..3 {
        let d = vec.get_default(i);
        assert_eq!(i, d.num as _);
    }
    drop(vec);
});

contract_test!(test_map_at {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 3;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut(&slot).strict_inc(7);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let v = data.map.at(&slot).get();
        assert_eq!(v, 7);
    }
});

contract_test!(test_map_at_nested {
    let data_keypath = Keypath::new();
    {
        let data: Data = DefaultFromKeypath::default_from_keypath(&data_keypath);
        data.store_keypath(&data_keypath);
    }

    let slot: u32 = 5;
    {
        let mut data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        data.map.at_mut(&slot).strict_inc(42);
    }

    {
        let data: Data = LoadKeypath::load_keypath(&data_keypath).unwrap();
        let val = data.map.at(&slot).try_get().unwrap_or(0);
        assert_eq!(val, 42);
    }
});

contract_test!(test_value_basic {
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

    // Initially empty
    assert_eq!(val.try_get(), None);

    // Set a value
    val.set(&42);
    assert_eq!(val.get(), 42);

    // Overwrite
    val.set(&99);
    assert_eq!(val.get(), 99);

    // Delete
    val.delete();
    assert_eq!(val.try_get(), None);
});

contract_test!(test_value_load_mut {
    let keypath = Keypath::new();
    let mut val: Value<i32> = Value::new(&keypath);

    // Set initial value
    val.set(&10);

    // Modify via load_mut — changes flush on drop
    {
        let mut data = val.load_mut();
        *data = 42;
    }

    assert_eq!(val.get(), 42);
});

contract_test!(test_map_at_value {
    let keypath = Keypath::new();
    let mut map: Map<u32, Value<i32>> = Map::new(&keypath);

    // Write via at_mut
    map.at_mut(&1u32).set(&100);

    // Read via at
    let v = map.at(&1u32).try_get();
    assert_eq!(v, Some(100));

    // Modify via load_mut through at_mut
    {
        let mut slot = map.at_mut(&1u32);
        let mut data = slot.load_mut();
        *data = 200;
    }

    let v = map.at(&1u32).get();
    assert_eq!(v, 200);
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
