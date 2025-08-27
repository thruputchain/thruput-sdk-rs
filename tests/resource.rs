use sdk::{bytestring_get, contract_test, WriteXDR};

contract_test!(test_storable {
    use sdk::resource::*;
    use xdrs::serialize_cxs::buf_reader::*;

    #[derive(Debug, PartialEq, WriteXDR)]
    struct Foo {
        n: u32,
        child: Option<Handle<Foo>>,
    }

    impl LoadPath for Foo {
        fn load_path(keypath: &Keypath) -> Result<Self> {
            let buf = bytestring_get(&keypath.generate_key())?;
            let mut reader = BufReader::new(buf.as_ref());
            let reader = &mut reader;

            let mut kp = keypath.clone();
            kp.enter_seq();
            let n = ReadPath::read_path(&kp, reader)?;

            kp.seq_next();
            let child = ReadPath::read_path(&kp, reader)?;

            let value = Foo { n, child };
            Ok(value)
        }
    }

    let foo = Foo { n: 12, child: None };

    // Allocate a handle and store the foo value into it
    let mut region = Region::new("objects");
    let mut foo_h: Handle<Foo> = region.alloc();
    foo_h.store(&foo);

    // Get the resource and modify it
    {
        let mut foo_r = foo_h.get();
        let foo: &mut Foo = &mut *foo_r;
        foo.n = 30;

        let mut child = region.alloc();
        child.store(&Foo { n: 1, child: None });
        foo.child = Some(child);

        foo_r.flush();
    }

    // Get the resource and check the modification
    let foo_r2 = foo_h.get();
    let foo2: &Foo = &*foo_r2;
    assert_eq!(foo2.n, 30);
    let child_h = foo2.child.as_ref().unwrap();
    let child_r = child_h.get();
    assert_eq!(child_r.n, 1);

    /*
    {
        #[derive(Debug, ReadXDR, WriteXDR, PartialEq)]
        struct Boo;

        let mut booh: Handle<Boo> = foo_r.get_handle(KeypathComponent::Index(0));
        booh.store(Boo);
        let boo = booh.load();
        println!("Boo: {boo:?}");
    }
    */
});
