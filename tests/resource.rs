use sdk::contract_test;

contract_test!(test_storable {
    use sdk::resource::*;
    use xdrs::{ReadXDR, WriteXDR};

    #[derive(Debug, ReadXDR, WriteXDR, PartialEq)]
    struct Foo {
        n: u32,
        child: Option<Handle<Foo>>,
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
