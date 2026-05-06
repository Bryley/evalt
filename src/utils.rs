use std::sync::Arc;

use crossterm::style::Stylize as _;
use miette::JSONReportHandler;
use serde::{Serialize, Serializer};
use serde_json::Value;

pub trait AnyhowExt<T> {
    fn into_miette(self) -> miette::Result<T>;
}

impl<T> AnyhowExt<T> for anyhow::Result<T> {
    fn into_miette(self) -> miette::Result<T> {
        self.map_err(|e| miette::miette!("{e:#}"))
    }
}

pub fn render_rail_text(
    f: &mut dyn std::fmt::Write,
    text: &str,
    indent: usize,
) -> std::fmt::Result {
    let (term_width, _) = crossterm::terminal::size().unwrap();
    let prefix = "  ".repeat(indent);

    let lines = textwrap::wrap(text, term_width as usize - indent * 2 - 5);

    let lines_len = lines.len();
    for (index, line) in lines.into_iter().enumerate() {
        write!(f, "{prefix}   {} {}", "│".dim(), line.dim().italic())?;
        if index < lines_len - 1 {
            writeln!(f)?;
        }
    }

    Ok(())
}

pub fn clip(text: &str, length: usize) -> String {
    let line = text.replace(['\r', '\n'], "⏎");
    let mut it = line.chars();

    let mut line: String = it.by_ref().take(length).collect();

    if it.next().is_some() {
        line.push('…');
    }
    line
}

pub fn pass() -> String {
    "✔".green().to_string()
}

pub fn fail() -> String {
    "✖".red().to_string()
}

pub fn timeout() -> String {
    "✖".yellow().to_string()
}

pub fn miette_to_json_string(err: &miette::Report) -> anyhow::Result<Value> {
    let mut out = String::new();
    JSONReportHandler::new().render_report(&mut out, err.as_ref())?;
    Ok(serde_json::from_str(&out)?)
}

pub fn serialize_miette_report<S>(
    err: &Arc<miette::Report>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    miette_to_json_string(err.as_ref())
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}
