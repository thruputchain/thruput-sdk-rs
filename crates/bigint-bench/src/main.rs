//! Measures rv32 runtime gas for the big-integer benchmark contract, joins it
//! with the codesize rows the build driver collected, and reports the diff.
//!
//! Run it through `scripts/bigint-bench.sh`, which builds the artifacts this
//! reads. Standalone:
//!
//! ```text
//! cargo run -p bigint-bench -- --dir target/bigint-bench [--iters 1000] \
//!     [--codesize target/bigint-bench/codesize.tsv] [--baseline u256]
//! ```
//!
//! Artifacts are discovered as `<backend>.<op>.rv32a` in `--dir`. Each is
//! deployed into its own [`TestEnvironment`] and invoked once; gas is read from
//! `stats::gas_remaining()` either side of the call, exactly as
//! `crates/bottle-test` does for the example contracts.
//!
//! Two rows per backend matter. `<backend>.none` is the calibration build: the
//! same loop with an identity operation, so it carries the entrypoint dispatch,
//! the operand round trips and the loop counter and nothing else. Every reported
//! per-operation number is a difference against it -- gas and `.text` alike --
//! which is what makes them comparable across implementations whose harness
//! overheads differ.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bigint_bench_if::BigIntBenchClient;
use sdk::*;

/// Distinct per artifact only in that it must be *some* nonce; the address it
/// derives is never referenced outside the transaction that deploys it.
const NONCE: SHA256Hash = SHA256Hash::new([0x2A; 32]);

/// The transaction's root caller.
const CALLER: ContractAddress = ContractAddress::new([0xEE; 32]);

/// Fixed operands, so a rerun on the same artifacts reports the same gas.
///
/// Chosen to keep every operation on its wide path: `A` fills all 256 bits with
/// its top bit set (so `sqrt` runs its full 128-step loop and `bit_len` cannot
/// return early), and `B` is smaller than `A` but still over 128 bits, so
/// `div`/`rem` take the multi-limb quotient path rather than a single-limb fast
/// path. Neither is zero, and the kernels force divisors odd anyway.
///
/// **What the `div` row is not.** `B` is 201 bits — seven of eight `u32` limbs,
/// four of four `u64` ones — so Knuth D runs its quotient loop twice on `u256`
/// and once on `uint`, against a possible seven for a two-limb divisor. That is
/// enough to exercise the loop and not enough for it to dominate, which is why
/// `div` lands near `div_small` rather than far above it. Read the division
/// rows as a near-floor case: a per-call overhead shows up as a larger
/// fraction here than it would on a deeper divide, and a change to the
/// quotient loop itself shows up as a smaller one. The two backends also run
/// different digit counts on the same operands, because their limb widths
/// differ — inherent to the comparison, not a flaw in it.
const A_HI: u128 = 0xF1E2_D3C4_B5A6_9788_796A_5B4C_3D2E_1F0B;
const A_LO: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3211;
const B_HI: u128 = 0x0000_0000_0000_0123_4567_89AB_CDEF_0123;
const B_LO: u128 = 0x89AB_CDEF_0123_4567_89AB_CDEF_0123_4567;

/// The calibration operation, subtracted from every other row.
const BASELINE_OP: &str = "none";

/// Section sizes for one artifact, as read from the ELF by the build driver.
///
/// `text` is the stored code image and `rv32a` the deployed artifact; `bss` is
/// VM-zeroed rather than stored and is reported for context only. Same split
/// `scripts/metrics/sdk_codesizes_rv32.sh` reports per contract.
#[derive(Clone, Copy, Default)]
struct Codesize {
    text: u64,
    rodata: u64,
    data: u64,
    bss: u64,
    rv32a: u64,
}

impl Codesize {
    fn data_image(&self) -> u64 {
        self.rodata + self.data
    }
}

/// One measured (backend, operation) build.
struct Row {
    backend: String,
    op: String,
    gas: u64,
    size: Codesize,
}

fn main() {
    let args = Args::parse(std::env::args().skip(1));

    let artifacts = discover(&args.dir);
    if artifacts.is_empty() {
        panic!(
            "no <backend>.<op>.rv32a artifacts under {} \
             (run scripts/bigint-bench.sh, which builds them)",
            args.dir.display()
        );
    }

    let sizes = args
        .codesize
        .as_deref()
        .map(read_codesize_tsv)
        .unwrap_or_default();

    let mut rows = Vec::new();
    for (backend, op, path) in artifacts {
        let gas = measure_gas(&path, args.iters);
        let size = sizes.get(&(backend.clone(), op.clone())).copied();
        if args.codesize.is_some() && size.is_none() {
            panic!(
                "codesize file has no row for {backend}.{op}; \
                 it and {} disagree about what was built",
                args.dir.display()
            );
        }
        rows.push(Row {
            backend,
            op,
            gas,
            size: size.unwrap_or_default(),
        });
    }

    rows.sort_by(|l, r| (&l.backend, &l.op).cmp(&(&r.backend, &r.op)));

    emit_metric_rows(&rows, args.iters);
    report(&rows, args.iters, args.baseline.as_deref());
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

/// Deploys `path` and returns the gas its `run` entrypoint consumed.
///
/// A fresh environment per artifact: the mock node caches an rv32 translation
/// per contract hash, and reusing one environment across 30-odd artifacts would
/// make each measurement depend on what ran before it.
fn measure_gas(path: &Path, iters: u32) -> u64 {
    let contract = load_rv32_contract(path);

    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let result = env.transact(|tx| {
        tx.call(&CALLER).run(move || -> (u64, u64) {
            let addr = deploy_contract(&NONCE, &contract_hash);
            let client = BigIntBenchClient::new(&addr);

            let before = stats::gas_remaining();
            let digest = client.run(iters, A_HI, A_LO, B_HI, B_LO);
            let after = stats::gas_remaining();

            let consumed = before.checked_sub(after).unwrap_or_else(|| {
                panic!("gas_remaining went up across the call")
            });

            (consumed, digest)
        })
    });

    result.assert_success();
    let (consumed, digest): (u64, u64) = result.assert_return_as();

    // The digest exists so the kernel's result stays live. A zero one is not
    // wrong for every operation (`cmp` on these operands is one), but it is the
    // signature of a kernel that got dead-coded, so say so rather than
    // reporting the gas of an empty loop as a result.
    if digest == 0 {
        eprintln!(
            "warning: {} returned a zero digest -- check the kernel was not \
             optimized away",
            path.display()
        );
    }

    consumed
}

// ---------------------------------------------------------------------------
// Artifact and codesize discovery
// ---------------------------------------------------------------------------

/// Finds `<backend>.<op>.rv32a` under `dir`, returning `(backend, op, path)`.
fn discover(dir: &Path) -> Vec<(String, String, PathBuf)> {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| {
        panic!("couldn't read artifact directory {}: {e}", dir.display())
    });

    let mut found = Vec::new();
    for entry in entries {
        let path = entry.expect("directory entry").path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".rv32a") else {
            continue;
        };
        // Backend names carry no dot; operation names carry no dot. Split on
        // the first one so an op like `mul_div_u128` stays intact.
        let Some((backend, op)) = stem.split_once('.') else {
            panic!("artifact {name} is not named <backend>.<op>.rv32a");
        };
        found.push((backend.to_string(), op.to_string(), path));
    }

    found.sort();
    found
}

/// Reads the driver's codesize TSV: a header line, then one row per artifact.
fn read_codesize_tsv(path: &Path) -> BTreeMap<(String, String), Codesize> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("couldn't read codesize file {}: {e}", path.display())
    });

    let mut lines = text.lines();
    let header = lines.next().unwrap_or_else(|| {
        panic!("codesize file {} is empty", path.display())
    });

    // Resolve columns by name rather than position, so adding a section to the
    // driver's output does not silently shift every number by one.
    let columns: Vec<&str> = header.split('\t').collect();
    let index = |want: &str| -> usize {
        columns.iter().position(|c| *c == want).unwrap_or_else(|| {
            panic!(
                "codesize file {} has no `{want}` column (header: {header})",
                path.display()
            )
        })
    };
    let (i_backend, i_op) = (index("backend"), index("op"));
    let (i_text, i_rodata) = (index("text"), index("rodata"));
    let (i_data, i_bss) = (index("data"), index("bss"));
    let i_rv32a = index("rv32a");

    let mut out = BTreeMap::new();
    for (n, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let number = |i: usize| -> u64 {
            fields
                .get(i)
                .unwrap_or_else(|| {
                    panic!(
                        "{}:{}: expected {} columns, got {}",
                        path.display(),
                        n + 2,
                        columns.len(),
                        fields.len()
                    )
                })
                .parse()
                .unwrap_or_else(|e| {
                    panic!("{}:{}: {e}", path.display(), n + 2)
                })
        };

        out.insert(
            (fields[i_backend].to_string(), fields[i_op].to_string()),
            Codesize {
                text: number(i_text),
                rodata: number(i_rodata),
                data: number(i_data),
                bss: number(i_bss),
                rv32a: number(i_rv32a),
            },
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

/// A backend's calibration figures, from its `none` build.
#[derive(Clone, Copy, Default)]
struct Baseline {
    gas: u64,
    text: u64,
}

/// What one operation costs over its backend's calibration build.
#[derive(Clone, Copy)]
struct Marginal {
    /// `.text` bytes the operation adds.
    text: i64,
    /// Gas per operation.
    gas: f64,
}

/// `name key=value ...` on stdout, one line per build.
///
/// Same shape as `scripts/metrics/sdk_codesizes_rv32.sh`, so this can be
/// appended to metric history as-is. The `*_net` fields are the marginals over
/// this backend's `none` row; the absolute values are kept alongside them so a
/// consumer can recompute or track either.
fn emit_metric_rows(rows: &[Row], iters: u32) {
    let baselines = baselines(rows);

    for row in rows {
        let base = baselines.get(&row.backend).copied().unwrap_or_default();
        let gas_net = row.gas as i64 - base.gas as i64;

        println!(
            "{}.{} text={} rodata={} data={} bss={} data_image={} \
             rv32a={} text_net={} gas_total={} gas_base={} gas_net={} \
             iters={} gas_per_op={:.3}",
            row.backend,
            row.op,
            row.size.text,
            row.size.rodata,
            row.size.data,
            row.size.bss,
            row.size.data_image(),
            row.size.rv32a,
            row.size.text as i64 - base.text as i64,
            row.gas,
            base.gas,
            gas_net,
            iters,
            gas_net as f64 / f64::from(iters),
        );
    }
}

/// Each backend's calibration figures, keyed by backend.
fn baselines(rows: &[Row]) -> BTreeMap<String, Baseline> {
    let mut out = BTreeMap::new();
    for row in rows {
        if row.op == BASELINE_OP {
            out.insert(
                row.backend.clone(),
                Baseline {
                    gas: row.gas,
                    text: row.size.text,
                },
            );
        }
    }
    out
}

/// The human-readable comparison, on stderr.
fn report(rows: &[Row], iters: u32, baseline_backend: Option<&str>) {
    let baselines = baselines(rows);

    let mut backends: Vec<&str> =
        rows.iter().map(|r| r.backend.as_str()).collect();
    backends.sort_unstable();
    backends.dedup();

    // The column everything else is compared against. An explicit `--baseline`
    // wins when it names a backend that is present; otherwise the first one.
    let base_backend = baseline_backend
        .filter(|b| backends.contains(b))
        .or_else(|| backends.first().copied());

    for backend in &backends {
        match baselines.get(*backend) {
            Some(base) => eprintln!(
                "calibration ({backend}.{BASELINE_OP}): {} gas over {iters} \
                 iterations, {} bytes .text",
                base.gas, base.text
            ),
            None => eprintln!(
                "warning: no {backend}.{BASELINE_OP} build; every {backend} \
                 number below is absolute, not per-operation"
            ),
        }
    }
    eprintln!();

    let ops = operations(rows);
    let index: BTreeMap<(&str, &str), &Row> = rows
        .iter()
        .map(|r| ((r.backend.as_str(), r.op.as_str()), r))
        .collect();

    let marginal = |backend: &str, op: &str| -> Option<Marginal> {
        let row = index.get(&(backend, op))?;
        let base = baselines.get(backend).copied().unwrap_or_default();
        Some(Marginal {
            text: row.size.text as i64 - base.text as i64,
            gas: (row.gas as i64 - base.gas as i64) as f64 / f64::from(iters),
        })
    };

    // A `.text` and a gas column per backend, then a delta pair for each
    // backend that is not the one being compared against.
    let mut header = format!("{:<15}", "operation");
    for backend in &backends {
        header += &format!(
            "{:>12}{:>12}",
            format!("{backend} B"),
            format!("{backend} gas")
        );
    }
    for backend in &backends {
        if Some(*backend) != base_backend {
            header += &format!("{:>16}{:>16}", "ΔB", "Δgas");
        }
    }
    eprintln!("{header}");
    eprintln!("{}", "-".repeat(header.chars().count()));

    for op in &ops {
        // The calibration build is the reference, not a row of its own.
        if *op == BASELINE_OP {
            continue;
        }

        let mut line = format!("{op:<15}");
        for backend in &backends {
            match marginal(backend, op) {
                Some(m) => line += &format!("{:>12}{:>12.2}", m.text, m.gas),
                None => line += &format!("{:>12}{:>12}", "-", "-"),
            }
        }

        let base = base_backend.and_then(|b| marginal(b, op));
        for backend in &backends {
            if Some(*backend) == base_backend {
                continue;
            }
            match (base, marginal(backend, op)) {
                (Some(b), Some(m)) => {
                    line += &format!(
                        "{:>16}{:>16}",
                        delta_i(m.text - b.text, b.text),
                        delta_f(m.gas - b.gas, b.gas),
                    );
                }
                _ => line += &format!("{:>16}{:>16}", "-", "-"),
            }
        }
        eprintln!("{line}");
    }

    eprintln!();
    match base_backend {
        Some(base) => eprintln!(
            "B and gas columns are per-operation marginals over \
             `{BASELINE_OP}`: `B` the .text the operation adds to the \
             artifact, `gas` its gas divided by {iters} iterations. \
             Δ columns are the other backend minus `{base}`, so negative is \
             cheaper than `{base}`."
        ),
        None => {
            eprintln!(
                "no backend had a calibration build; columns are absolute"
            )
        }
    }
}

/// Every operation present, in the order the kernel table declares them.
///
/// Sorting alphabetically would separate `div` from `div_small` and put `add`
/// next to `bit_len`; the declared order groups operations by kind, which is how
/// the table is meant to be read. Anything not in the list (a new op added to
/// the contract but not here) is appended, so it still shows up.
fn operations(rows: &[Row]) -> Vec<&str> {
    const DECLARED: &[&str] = &[
        "none",
        "add",
        "sub",
        "mul",
        "mul_small",
        "widening_mul",
        "div",
        "div_small",
        "rem",
        "mul_div_u128",
        "shl",
        "shr",
        "bitwise",
        "cmp",
        "sqrt",
        "bytes",
        "bit_len",
    ];

    let mut present: Vec<&str> = rows.iter().map(|r| r.op.as_str()).collect();
    present.sort_unstable();
    present.dedup();

    let mut ordered: Vec<&str> = DECLARED
        .iter()
        .copied()
        .filter(|d| present.contains(d))
        .collect();
    ordered.extend(present.into_iter().filter(|p| !DECLARED.contains(p)));
    ordered
}

/// `+120 (+53%)`. The percentage is dropped when the base is zero, where it
/// would be either infinite or a lie.
fn delta_i(delta: i64, base: i64) -> String {
    if base == 0 {
        return format!("{delta:+}");
    }
    format!("{delta:+} ({:+.0}%)", 100.0 * delta as f64 / base as f64)
}

/// [`delta_i`] for the fractional gas columns.
fn delta_f(delta: f64, base: f64) -> String {
    if base == 0.0 {
        return format!("{delta:+.2}");
    }
    format!("{delta:+.2} ({:+.0}%)", 100.0 * delta / base)
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

struct Args {
    dir: PathBuf,
    codesize: Option<PathBuf>,
    iters: u32,
    baseline: Option<String>,
}

impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut dir = None;
        let mut codesize = None;
        let mut iters = 1000u32;
        let mut baseline = None;

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = |flag: &str| -> String {
                args.next()
                    .unwrap_or_else(|| panic!("{flag} needs a value"))
            };
            match arg.as_str() {
                "--dir" => dir = Some(PathBuf::from(value("--dir"))),
                "--codesize" => {
                    codesize = Some(PathBuf::from(value("--codesize")))
                }
                "--baseline" => baseline = Some(value("--baseline")),
                "--iters" => {
                    let raw = value("--iters");
                    iters = raw
                        .parse()
                        .unwrap_or_else(|e| panic!("--iters {raw}: {e}"));
                }
                "-h" | "--help" => {
                    eprintln!(
                        "usage: bigint-bench --dir <artifact dir> \
                         [--codesize <tsv>] [--iters <n>] [--baseline <backend>]"
                    );
                    std::process::exit(0);
                }
                other => panic!("unrecognized argument {other}"),
            }
        }

        assert!(iters > 0, "--iters must be positive");

        Self {
            dir: dir.unwrap_or_else(|| PathBuf::from("target/bigint-bench")),
            codesize,
            iters,
            baseline,
        }
    }
}
