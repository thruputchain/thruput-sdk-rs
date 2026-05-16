use std::{env, fs, io, path::PathBuf};

use groundhog_spec_tool::{
    SpecToolError, render_spec_json, rewrite::rewrite_wasm_with_spec,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Usage(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tool(#[from] SpecToolError),
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
        Some(_) => Err(CliError::Usage(
            "expected subcommand 'rewrite' or 'describe'",
        )),
        None => Err(CliError::Usage(
            "expected subcommand 'rewrite' or 'describe'",
        )),
    }
}

fn run_rewrite(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    let mut wasm_path: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wasm" => {
                let value = args
                    .next()
                    .ok_or(CliError::Usage("missing value for --wasm"))?;
                wasm_path = Some(PathBuf::from(value));
            }
            "--json-out" => {
                let value = args
                    .next()
                    .ok_or(CliError::Usage("missing value for --json-out"))?;
                json_out = Some(PathBuf::from(value));
            }
            _ => return Err(CliError::Usage("unexpected argument")),
        }
    }

    let wasm_path = wasm_path.ok_or(CliError::Usage("missing --wasm"))?;
    let json_out = json_out.ok_or(CliError::Usage("missing --json-out"))?;

    let bytes = fs::read(&wasm_path)?;
    let rewritten = rewrite_wasm_with_spec(&bytes)?;
    fs::write(&wasm_path, rewritten.wasm)?;
    if let Some(spec) = rewritten.spec {
        fs::write(&json_out, render_spec_json(&spec))?;
    } else {
        match fs::remove_file(&json_out) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

fn run_describe(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    use groundhog_spec_tool::describe::{
        OutputFormat, REGISTRY, build_spec, lookup, write_output,
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
                    return Err(CliError::Usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Json);
            }
            "--xdr" => {
                if format.is_some() {
                    return Err(CliError::Usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Xdr);
            }
            "-o" => {
                let value = args
                    .next()
                    .ok_or(CliError::Usage("missing value for -o"))?;
                output_path = Some(PathBuf::from(value));
            }
            other if !other.starts_with('-') => {
                if type_name.is_some() {
                    return Err(CliError::Usage(
                        "describe takes a single type name",
                    ));
                }
                type_name = Some(other.to_string());
            }
            _ => return Err(CliError::Usage("unexpected argument")),
        }
    }

    if list {
        if type_name.is_some() {
            return Err(CliError::Usage(
                "--list is mutually exclusive with a type name",
            ));
        }
        for entry in REGISTRY {
            println!("{}", entry.name);
        }
        return Ok(());
    }

    let type_name =
        type_name.ok_or(CliError::Usage("expected type name or --list"))?;
    let ty = lookup(&type_name).ok_or(CliError::Usage("unknown type name"))?;
    let spec = build_spec(ty)?;
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
