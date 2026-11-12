use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT, ristretto::RistrettoPoint,
    scalar::Scalar,
};
use ristretto_msm::RistrettoMsmClient;
use thruput_sdk::*;

pub fn run(contract_hash: &SHA256Hash, check: fn(&RistrettoMsmClient)) {
    let address =
        deploy_contract(&sha256_static!("ristretto-msm"), contract_hash);
    check(&RistrettoMsmClient::new(&address));
}

fn packed(scalar: &Scalar, point: &RistrettoPoint) -> [u8; 64] {
    let mut term = [0; 64];
    term[..32].copy_from_slice(scalar.as_bytes());
    term[32..].copy_from_slice(point.compress().as_bytes());
    term
}

pub fn arithmetic(client: &RistrettoMsmClient) {
    let g = RISTRETTO_BASEPOINT_POINT;
    let h = RistrettoPoint::from_uniform_bytes(&[42; 64]);
    let value = Scalar::from(17u64);
    let blind = Scalar::from(43u64);
    let commitment = (value * g + blind * h).compress().to_bytes();
    assert!(client.check_opening(
        value.as_bytes(),
        blind.as_bytes(),
        g.compress().as_bytes(),
        h.compress().as_bytes(),
        &commitment,
    ));
    assert!(!client.check_opening(
        (value + Scalar::ONE).as_bytes(),
        blind.as_bytes(),
        g.compress().as_bytes(),
        h.compress().as_bytes(),
        &commitment,
    ));
    assert_eq!(client.msm(&[]), [0; 32]);
    assert_eq!(client.msm(&packed(&Scalar::ZERO, &g)), [0; 32]);
    let identity = RistrettoPoint::default();
    assert_eq!(client.msm(&packed(&value, &identity)), [0; 32]);

    // Includes duplicate points and exact cancellation.
    let cancellation = [packed(&value, &g), packed(&(-value), &g)].concat();
    assert_eq!(client.msm(&cancellation), [0; 32]);

    // Independent software reference across all supported input sizes used here.
    for count in [1, 2, 8, 32] {
        let mut terms = Vec::new();
        let mut expected = RistrettoPoint::default();
        for index in 0..count {
            let scalar = Scalar::from((index + 1) as u64);
            let point = if index % 2 == 0 { g } else { h };
            terms.extend_from_slice(&packed(&scalar, &point));
            expected += scalar * point;
        }
        let before = thruput_sdk::stats::gas_remaining();
        assert_eq!(client.msm(&terms), expected.compress().to_bytes());
        let consumed = before - thruput_sdk::stats::gas_remaining();
        println!("gas: msm terms={count}, call plus measurement={consumed}");
    }
    let before = thruput_sdk::stats::gas_remaining();
    assert_eq!(
        client.repeated_msm(&packed(&Scalar::ONE, &g), 256),
        (Scalar::from(256u64) * g).compress().to_bytes(),
    );
    let consumed = before - thruput_sdk::stats::gas_remaining();
    println!(
        "gas: repeated msm terms=256, guest construction plus call plus measurement={consumed}"
    );
}

fn assert_syscall_error<T: core::fmt::Debug>(
    result: Result<T, CallError>,
    expected: sys::SyscallErrorCode,
) {
    let error = result.unwrap_err();
    let library = error.try_as::<LibraryExn>().expect("syscall exception");
    let syscall = library.as_syscall_exn().expect("syscall error");
    assert!(syscall.is(sys::SyscallNo::kRistretto255Msm, expected));
}

pub fn malformed(client: &RistrettoMsmClient) {
    let invalid = sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput;
    for bytes in [vec![0; 1], vec![0; 63], vec![0; 65]] {
        assert_syscall_error(client.try_msm(&bytes), invalid);
    }
    assert_syscall_error(client.try_repeated_msm(&[0; 64], 257), invalid);
    let mut scalar = [0xff; 32];
    let point = RISTRETTO_BASEPOINT_POINT.compress().to_bytes();
    // A bad opening is an exception, never `false`.
    assert_syscall_error(
        client.try_check_opening(&scalar, &[0; 32], &point, &point, &[0; 32]),
        invalid,
    );
    // Even a zero scalar must not hide a malformed point.
    scalar.fill(0);
    assert_syscall_error(
        client.try_check_opening(
            &scalar,
            &[0; 32],
            &[0xff; 32],
            &point,
            &[0; 32],
        ),
        invalid,
    );
}

pub fn insufficient_gas(client: &RistrettoMsmClient) {
    // Two terms alone cost 56,000 gas.
    // Native and guest callers must observe a fault, never a false opening.
    let result = client
        .with_gas_limit(50_000)
        .try_check_opening(&[0; 32], &[0; 32], &[0; 32], &[0; 32], &[0; 32]);
    assert!(matches!(result, Err(CallError::Fault(_))), "{result:?}");
}
