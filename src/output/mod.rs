use std::io::Write;

use anyhow::Result;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::chunk::Chunk;
use crate::search::SearchResult;

/// A search result paired with its chunk for display.
#[derive(serde::Serialize)]
pub struct DisplayResult<'a> {
    pub score: f32,
    #[serde(flatten)]
    pub chunk: &'a Chunk,
}

/// Print search results in grep-like format to the terminal.
pub fn print_results(
    results: &[SearchResult],
    chunks: &[Chunk],
    context_lines: usize,
    no_color: bool,
) -> Result<()> {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let mut stdout = StandardStream::stdout(color_choice);

    for (i, result) in results.iter().enumerate() {
        let chunk = &chunks[result.chunk_index];

        // Score in magenta/bold
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)).set_bold(true))?;
        write!(stdout, "{:.4}", result.score)?;

        // Separator
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
        write!(stdout, "  ")?;

        // File path in green
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)).set_bold(true))?;
        write!(stdout, "{}", chunk.file_path)?;

        // Separator
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
        write!(stdout, ":")?;

        // Line number in yellow
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        write!(stdout, "{}", chunk.line_number)?;

        stdout.reset()?;
        writeln!(stdout)?;

        // Print preview lines from the chunk
        let lines: Vec<&str> = chunk.text.lines().collect();
        let show_lines = if context_lines == 0 {
            3 // default preview
        } else {
            context_lines + 1
        };

        for line in lines.iter().take(show_lines) {
            if line.len() > 120 {
                writeln!(stdout, "  {}...", &line[..117])?;
            } else {
                writeln!(stdout, "  {line}")?;
            }
        }
        if lines.len() > show_lines {
            stdout.set_color(ColorSpec::new().set_dimmed(true))?;
            writeln!(stdout, "  ... ({} more lines)", lines.len() - show_lines)?;
            stdout.reset()?;
        }

        // Separator between results
        if i < results.len() - 1 {
            writeln!(stdout)?;
        }
    }

    Ok(())
}

/// Print search results as JSON to stdout.
pub fn print_results_json(results: &[SearchResult], chunks: &[Chunk]) -> Result<()> {
    let display: Vec<DisplayResult> = results
        .iter()
        .map(|r| DisplayResult {
            score: r.score,
            chunk: &chunks[r.chunk_index],
        })
        .collect();
    let json = serde_json::to_string_pretty(&display)?;
    println!("{json}");
    Ok(())
}

