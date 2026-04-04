pub struct MultiImportedTraitsContract;

mod named_interface_impl {
    use super::MultiImportedTraitsContract;
    use named_interface_callee::NamedInterfaceCallee;
    use sdk::*;

    #[contract(interface = "ExplicitNamedInterface")]
    impl NamedInterfaceCallee for MultiImportedTraitsContract {
        fn add_one(x: u32) -> u32 {
            x + 1
        }
    }
}

mod versioned_interface_impl {
    use super::MultiImportedTraitsContract;
    use sdk::*;
    use versioned_callee::VersionedCallee;

    #[contract(version = "1.2.3")]
    impl VersionedCallee for MultiImportedTraitsContract {
        fn add_one(x: u32) -> u32 {
            x + 2
        }
    }
}
