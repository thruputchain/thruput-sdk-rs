use sdk::contract_test;

contract_test!(test_storable {
    use sdk::resource::*;
    use xdrs::{ReadXDR, WriteXDR};

    #[derive(Debug, ReadXDR, WriteXDR)]
    struct Boo;

    #[derive(Debug, ReadXDR, WriteXDR)]
    struct Foo;

    let kp = Keypath::root();

    let fr = Resource::new(&kp, Foo);

    let mut booh: Handle<Boo> = fr.get_handle(KeypathComponent::Index(0));

    booh.store(Boo);

    let boo = booh.load();

    println!("Boo: {boo:?}"); 
});
