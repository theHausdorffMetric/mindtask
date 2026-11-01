//! The `--format` flag shared by the listing commands, and the JSON printer.

use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;

/// How a listing is rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Terminal tables and trees (the default).
    #[default]
    Text,
    /// The listing's view model as pretty-printed JSON.
    Json,
}

/// Print `value` as pretty JSON with a trailing newline.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
