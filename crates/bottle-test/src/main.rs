use bottle_test_if::*;

use sdk::load_rv32_contract;
use sdk::*;

pub fn main() {
    let stem = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("Please specify contract artifact to test"));

    bottle_test(&stem);
}

pub const TESTER_NONCE: SHA256Hash = SHA256Hash::new([7; 32]);
pub const OWNER: ContractAddress = ContractAddress::new([0xFF; 32]);

/// `stem` is the artifact stem `scripts/dev.sh` staged the pair under: the
/// rv32 bin name of the contract's `-rv32` wrapper package. The two builds
/// differ only in the wrapper's `bottle-test` feature — the target is the
/// contract under measurement, the tester adds the `BottleTest` entrypoints
/// that drive it.
fn bottle_test(stem: &str) {
    let target_contract =
        load_rv32_contract(format!("target/bottle-test/{stem}.target.rv32a"));

    let tester_contract =
        load_rv32_contract(format!("target/bottle-test/{stem}.tester.rv32a"));

    let env = TestEnvironment::no_cost();

    // Create the tester and target contracts
    let tester_hash = env.setup_create_contract(&tester_contract);
    let target_hash = env.setup_create_contract(&target_contract);

    // Something we can move into the test closure
    let stem = stem.to_string();

    let result = env.transact(|tx| {
        tx.call(&OWNER).run(move || {
            let tester_addr = deploy_contract(&TESTER_NONCE, &tester_hash);

            let tester = BottleTestClient::new(&tester_addr);
            tester.setup(&target_hash);

            let gas_start = stats::gas_remaining();
            tester.run();
            let gas_end = stats::gas_remaining();

            let gas_consumed = gas_start
                .checked_sub(gas_end)
                .unwrap_or_else(|| panic!("Negative gas consumed!"));

            eprintln!(
                "Bottle test succeeded (gas consumed: {})",
                gas_consumed
            );
            println!("{}\tgas_consumed\t{}", stem, gas_consumed);
        })
    });
    result.assert_success();

    // Diagnostic: per-syscall / guest-instruction gas breakdown of the
    // measured `tester.run()` invoke, from the execution trace.
    if std::env::var("BOTTLE_TRACE").is_ok() {
        if let Some(trace) = result.frame().trace() {
            trace_analysis::report(trace);
        } else {
            eprintln!("BOTTLE_TRACE set but no trace available");
        }
    }

    // Print out the transaction's event log
    {
        use groundhog_erc20_if::*;
        use sdk::EmittedEvent;

        for (i, event) in result.events().iter().enumerate() {
            let event: EmittedEvent = event.into();
            match ERC20Event::try_from(event) {
                Ok(event) => eprintln!("\tevent {:?}: {}", i, event),
                Err(_) => eprintln!("\tevent {:?}: <non-ERC20 event>", i),
            }
        }
    }
}

mod trace_analysis {
    use groundhog_trace::{InvocationFrameTrace, SyscallTraceBody};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Tally {
        // gas attributed to guest instructions, per frame depth
        guest_by_depth: BTreeMap<usize, u64>,
        // gas consumed inside non-invoke syscall windows, by syscall kind
        syscall_gas: BTreeMap<String, u64>,
        syscall_count: BTreeMap<String, u64>,
        sha256_lens: Vec<usize>,
    }

    fn body_name(body: &SyscallTraceBody) -> String {
        let dbg = format!("{body:?}");
        dbg.split([' ', '{']).next().unwrap_or("?").to_string()
    }

    // Returns the sum of this frame's syscall windows (each window includes
    // any child-frame execution nested in it).
    fn walk(frame: &InvocationFrameTrace, depth: usize, t: &mut Tally) -> u64 {
        let mut window_sum: u64 = 0;
        for sc in frame.system_calls.iter() {
            let consumed = sc
                .gas_remaining_at_start
                .saturating_sub(sc.gas_remaining_at_end);
            window_sum += consumed;
            let name = body_name(&sc.body);
            *t.syscall_count.entry(name.clone()).or_default() += 1;
            match &sc.body {
                SyscallTraceBody::kInvoke { invoke } => {
                    let child_windows = invoke
                        .outputs
                        .as_ref()
                        .and_then(|o| o.child_trace.as_ref())
                        .map(|child| walk(child, depth + 1, t))
                        .unwrap_or(0);
                    // Whatever the invoke window consumed beyond the child's
                    // own syscall windows is the child frame's guest
                    // (instruction) gas plus invoke host overhead.
                    *t.guest_by_depth.entry(depth + 1).or_default() +=
                        consumed.saturating_sub(child_windows);
                }
                SyscallTraceBody::kSHA256 { sha256 } => {
                    t.sha256_lens.push(sha256.inputs.input.len());
                    *t.syscall_gas.entry(name).or_default() += consumed;
                }
                _ => {
                    *t.syscall_gas.entry(name).or_default() += consumed;
                }
            }
        }
        window_sum
    }

    pub fn report(root: &InvocationFrameTrace) {
        // The measured section is bracketed by the two
        // kGetTransactionStatistics syscalls at root level; the single
        // kInvoke between them is `tester.run()`.
        let sys = &root.system_calls;
        let stat_idx: Vec<usize> = sys
            .iter()
            .enumerate()
            .filter(|(_, sc)| {
                matches!(
                    sc.body,
                    SyscallTraceBody::kGetTransactionStatistics { .. }
                )
            })
            .map(|(i, _)| i)
            .collect();
        let (lo, hi) = match stat_idx.as_slice() {
            [a, b, ..] => (*a, *b),
            _ => {
                eprintln!("trace: could not find stat brackets");
                return;
            }
        };

        let mut t = Tally::default();
        let mut measured_total = 0u64;
        for sc in &sys[lo + 1..hi] {
            let consumed = sc
                .gas_remaining_at_start
                .saturating_sub(sc.gas_remaining_at_end);
            measured_total += consumed;
            if let SyscallTraceBody::kInvoke { invoke } = &sc.body {
                let child_windows = invoke
                    .outputs
                    .as_ref()
                    .and_then(|o| o.child_trace.as_ref())
                    .map(|child| walk(child, 1, &mut t))
                    .unwrap_or(0);
                *t.guest_by_depth.entry(1).or_default() +=
                    consumed.saturating_sub(child_windows);
            }
        }

        eprintln!("== trace analysis of measured tester.run() section ==");
        eprintln!("measured total: {measured_total}");
        let guest_total: u64 = t.guest_by_depth.values().sum();
        eprintln!("guest (instruction) gas total: {guest_total}");
        for (depth, gas) in &t.guest_by_depth {
            eprintln!("  guest gas at frame depth {depth}: {gas}");
        }
        let syscall_total: u64 = t.syscall_gas.values().sum();
        eprintln!("syscall (host-charged) gas total: {syscall_total}");
        let mut rows: Vec<_> = t.syscall_count.iter().collect();
        rows.sort_by_key(|(name, _)| {
            std::cmp::Reverse(t.syscall_gas.get(*name).copied().unwrap_or(0))
        });
        for (name, count) in rows {
            let gas = t.syscall_gas.get(name).copied().unwrap_or(0);
            eprintln!("  {name}: count={count} host_gas={gas}");
        }
        if !t.sha256_lens.is_empty() {
            let total_bytes: usize = t.sha256_lens.iter().sum();
            eprintln!(
                "sha256 syscalls: {} calls, {} preimage bytes, lens={:?}",
                t.sha256_lens.len(),
                total_bytes,
                t.sha256_lens
            );
        }
    }
}
