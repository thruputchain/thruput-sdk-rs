use std::{env, fs, io, path::PathBuf};

use thiserror::Error;
use thruput_spec_tool::SpecToolError;

const USAGE: &str = "\
Usage: thruput-spec-tool <SUBCOMMAND>

Subcommands:
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
        Some("describe") => run_describe(args),
        Some("--help") | Some("-h") | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(usage(format!("unexpected subcommand '{other}'"))),
        None => Err(usage("expected subcommand 'describe'")),
    }
}

fn run_describe(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    use thruput_spec_tool::describe::{
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

    let type_name = type_name.ok_or(usage("expected type name or --list"))?;
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
