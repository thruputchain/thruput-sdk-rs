//! Cross-crate storage-domain uniqueness — part of the bin-wrapper
//! pattern: every contract crate linked into the guest bin must be
//! `use`d here so its root registrations link into this test binary.

use counter as _;

sdk::storage_domain_uniqueness_test!();
