//! Cross-crate storage-domain uniqueness: every contract crate linked into
//! this package's guest bin must be `use`d here, so its root registrations
//! link into this test binary and the macro can compare them.

use counter as _;

thruput_sdk::storage_domain_uniqueness_test!();
