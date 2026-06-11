use std::{env, fs, io, path::Path, path::PathBuf};

use groundhog_spec_tool::{
    SpecToolError, render_spec_json, rewrite::rewrite_wasm_with_spec,
    spec_to_json,
};
use thiserror::Error;

const USAGE: &str = "\
Usage: groundhog-spec-tool <SUBCOMMAND>

Subcommands:
  rewrite --wasm <PATH> --json-out <PATH> [OPTIONS]
      Canonicalize the contract-spec custom sections of a wasm binary:
      all raw spec sections are stripped and a single final spec section
      is emitted.

      By default the wasm is rewritten IN PLACE (atomically, via a
      temporary file and rename). The JSON spec sidecar is written to
      --json-out; if the wasm contains no spec sections at all, any
      existing file at --json-out is DELETED so a stale sidecar never
      outlives its spec.

      Options:
        --out <PATH>           write the rewritten wasm here instead of
                               rewriting --wasm in place
        --json-format <FMT>    sidecar format: 'debug' (default;
                               human-friendly, not parseable back) or
                               'round-trip' (lossless; parseable with
                               groundhog_spec_tool::parse_spec_json)

  describe [--list | <TYPE> [--json | --xdr] [-o <PATH>]]
      Describe a registered describable type, or list them all.
";

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}\n\n{USAGE}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tool(#[from] SpecToolError),
}

fn usage(msg: impl Into<String>) -> CliError {
    CliError::Usage(msg.into())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("rewrite") => run_rewrite(args),
        Some("describe") => run_describe(args),
        Some("--help") | Some("-h") | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => {
            Err(usage(format!("unexpected subcommand '{other}'")))
        }
        None => Err(usage("expected subcommand 'rewrite' or 'describe'")),
    }
}

enum JsonFormat {
    Debug,
    RoundTrip,
}

fn run_rewrite(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    let mut wasm_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut json_format = JsonFormat::Debug;

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wasm" => {
                let value =
                    args.next().ok_or(usage("missing value for --wasm"))?;
                wasm_path = Some(PathBuf::from(value));
            }
            "--out" => {
                let value =
                    args.next().ok_or(usage("missing value for --out"))?;
                out_path = Some(PathBuf::from(value));
            }
            "--json-out" => {
                let value = args
                    .next()
                    .ok_or(usage("missing value for --json-out"))?;
                json_out = Some(PathBuf::from(value));
            }
            "--json-format" => {
                let value = args
                    .next()
                    .ok_or(usage("missing value for --json-format"))?;
                json_format = match value.as_str() {
                    "debug" => JsonFormat::Debug,
                    "round-trip" => JsonFormat::RoundTrip,
                    other => {
                        return Err(usage(format!(
                            "--json-format must be 'debug' or \
                             'round-trip', got '{other}'"
                        )));
                    }
                };
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => {
                return Err(usage(format!("unexpected argument '{other}'")));
            }
        }
    }

    let wasm_path = wasm_path.ok_or(usage("missing --wasm"))?;
    let json_out = json_out.ok_or(usage("missing --json-out"))?;
    let out_path = out_path.unwrap_or_else(|| wasm_path.clone());

    let bytes = fs::read(&wasm_path)?;
    let rewritten = rewrite_wasm_with_spec(&bytes)?;
    write_atomically(&out_path, &rewritten.wasm)?;
    if rewritten.stripped_typedef_only_fragments {
        eprintln!(
            "warning: {} carried raw spec fragments with no interface; \
             they were stripped and no spec was emitted. This is expected \
             for raw contracts linking SDK typedefs, but also what a \
             missing #[contract] interface looks like.",
            wasm_path.display()
        );
    }
    if let Some(spec) = rewritten.spec {
        let json = match json_format {
            JsonFormat::Debug => render_spec_json(&spec),
            JsonFormat::RoundTrip => spec_to_json(&spec).map_err(|err| {
                SpecToolError::Wasm(format!(
                    "failed to serialize spec JSON: {err}"
                ))
            })?,
        };
        fs::write(&json_out, json)?;
    } else {
        // No spec sections in the input at all: remove any stale sidecar
        // from a previous run so it cannot outlive its spec.
        match fs::remove_file(&json_out) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// Write via a temporary sibling file + rename, so an interrupted write
/// cannot leave a truncated/corrupt binary at the destination (which may be
/// the input itself when rewriting in place).
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn run_describe(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    use groundhog_spec_tool::describe::{
        OutputFormat, REGISTRY, build_describe, lookup, write_output,
    };

    let mut list = false;
    let mut type_name: Option<String> = None;
    let mut format: Option<OutputFormat> = None;
    let mut output_path: Option<PathBuf> = None;

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => list = true,
            "--json" => {
                if format.is_some() {
                    return Err(usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Json);
            }
            "--xdr" => {
                if format.is_some() {
                    return Err(usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Xdr);
            }
            "-o" => {
                let value =
                    args.next().ok_or(usage("missing value for -o"))?;
                output_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if !other.starts_with('-') => {
                if type_name.is_some() {
                    return Err(usage("describe takes a single type name"));
                }
                type_name = Some(other.to_string());
            }
            other => {
                return Err(usage(format!("unexpected argument '{other}'")));
            }
        }
    }

    if list {
        if type_name.is_some() {
            return Err(usage(
                "--list is mutually exclusive with a type name",
            ));
        }
        if format.is_some() {
            return Err(usage(
                "--list is mutually exclusive with --json and --xdr",
            ));
        }
        if output_path.is_some() {
            return Err(usage("--list is mutually exclusive with -o"));
        }
        for entry in REGISTRY {
            println!("{}", entry.name);
        }
        return Ok(());
    }

    let type_name =
        type_name.ok_or(usage("expected type name or --list"))?;
    let ty = lookup(&type_name)
        .ok_or_else(|| usage(format!("unknown type name '{type_name}'")))?;
    let spec = build_describe(ty);
    let format = format.unwrap_or(OutputFormat::Json);

    match output_path {
        Some(path) => {
            let mut file = fs::File::create(&path)?;
            write_output(&spec, format, &mut file)?;
        }
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            write_output(&spec, format, &mut handle)?;
        }
    }
    Ok(())
}
