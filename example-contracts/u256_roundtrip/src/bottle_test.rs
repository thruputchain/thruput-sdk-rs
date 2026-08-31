//! Guest-to-guest coverage for the 256-bit ABI.
//!
//! Every other `U256`/`I256` test in this repo drives the contract from the
//! host, so the generated client's *encode* side runs as native x86 code and
//! only the callee half is ever compiled for rv32. Here the caller is itself
//! a contract: `U256RoundtripClient` is built for the guest, so the argument
//! encoding, the cross-contract call, and the return decode all run on the
//! target a deployed contract runs on. Eight 32-bit limbs, big-endian, on a
//! 32-bit machine -- the half nothing else exercises.
//!
//! `erc20`'s bottle test does not cover this: its amounts are `u128`/`i128`.
//!
//! One thing to know when this fails: contracts build with
//! `panic=immediate-abort`, so a tripped assertion traps rather than
//! reporting itself, and the harness surfaces only `child frame faulted`.
//! It tells you the guest-to-guest path broke, not which line. Bisect by
//! commenting legs out, or reach for the host-driven `tests/rv32.rs` first,
//! which names the values it compared.

use crate::U256RoundtripClient;
pub use bottle_test_if::*;
use sdk::storage::Value;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Value<Option<ContractAddress>>,
}

/// A negative value that is *not* its own negation, so the signed leg pins
/// the sign surviving the call rather than only the bit pattern.
/// [`I256::MIN`] cannot do that job -- negation is the identity on it.
const ASYMMETRIC: i128 = -0x0DEA_DBEE_F001_1223_3445_5667_7889_9AAB;

/// Every limb differs and the top bit is set, so a limb reordering or a
/// truncation in the guest-side encode shows up rather than cancelling.
const UNSIGNED: u128 = 0xDEAD_BEEF_0011_2233_4455_6677_8899_AABB;

#[contract(exports = "bottle_exports", version = "0.1.0")]
impl BottleTest for BT {
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        let nonce: SHA256Hash = [7; 32].into();
        let target_addr = deploy_contract(&nonce, target_contract_hash);

        self.target_addr.set(&Some(target_addr));
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.get().unwrap_or_else(||
            panic!("bottle-test: target_addr unset; please run setup() before run()"));

        let client = U256RoundtripClient::new(&target_addr);

        // `assert!(a == b)` rather than `assert_eq!`: contracts build with
        // `panic=immediate-abort`, so the message is discarded either way,
        // and comparing keeps `Debug` and its hex table out of the artifact.

        // Unsigned, so the guest-side encode is covered for both types.
        let u = U256::from(UNSIGNED);
        let plus_one = u.wrapping_add(U256::ONE);
        assert!(client.add_one_and_store(u) == plus_one);
        assert!(client.stored() == plus_one);

        // Signed and asymmetric: the sign has to survive the round trip, not
        // just the bits.
        let neg = I256::from(ASYMMETRIC);
        let pos = neg.wrapping_neg();
        assert!(neg.is_negative() && pos.is_positive());
        assert!(client.negate_and_store(neg) == pos);
        assert!(client.stored_signed() == pos);

        // And back, so a negative value survives the call in both directions.
        assert!(client.negate_and_store(pos) == neg);
        assert!(client.stored_signed() == neg);

        // `I256::MIN`: its own two's complement, all 32 bytes of it, and the
        // one value an encoder that mishandles the sign bit cannot fake.
        assert!(client.negate_and_store(I256::MIN) == I256::MIN);
        assert!(client.stored_signed() == I256::MIN);
    }
}
