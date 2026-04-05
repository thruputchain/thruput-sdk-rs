use std::{env, fs, path::PathBuf};

use groundhog_spec_tool::{
    SpecToolError, render_spec_json, rewrite::rewrite_wasm_with_interfaces,
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
        Some(_) => Err(CliError::Usage("expected subcommand 'rewrite'")),
        None => Err(CliError::Usage("expected subcommand 'rewrite'")),
    }
}

fn run_rewrite(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    let mut package_name: Option<String> = None;
    let mut wasm_path: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--package" => {
                package_name =
                    Some(args.next().ok_or(CliError::Usage(
                        "missing value for --package",
                    ))?);
            }
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

    let package_name =
        package_name.ok_or(CliError::Usage("missing --package"))?;
    let wasm_path = wasm_path.ok_or(CliError::Usage("missing --wasm"))?;
    let json_out = json_out.ok_or(CliError::Usage("missing --json-out"))?;

    let bytes = fs::read(&wasm_path)?;
    let rewritten = rewrite_wasm_with_interfaces(&bytes, &package_name)?;
    fs::write(&wasm_path, rewritten.wasm)?;
    if rewritten.interfaces.is_empty() {
        match fs::remove_file(&json_out) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    } else {
        fs::write(&json_out, render_spec_json(&rewritten.interfaces))?;
    }
    Ok(())
}
