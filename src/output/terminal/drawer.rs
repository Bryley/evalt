use std::io::stdout;

use std::io::Write as _;

use crossterm::cursor::MoveUp;
use crossterm::execute;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;

use crate::utils::clip;

pub struct Drawer {
    term_width: u16,
    lines: Vec<String>,
}

impl Default for Drawer {
    fn default() -> Self {
        let (term_width, _) = crossterm::terminal::size().unwrap();
        Self {
            term_width,
            lines: Vec::new(),
        }
    }
}

impl Drawer {
    #[allow(dead_code)]
    pub fn add_wrapped_line(&mut self, line: &str) {
        let lines = textwrap::wrap(line, self.term_width as usize)
            .into_iter()
            .map(|line| line.into_owned());
        self.lines.extend(lines);
    }

    pub fn add_clipped_line(&mut self, line: &str) {
        let length = self.term_width as usize - 5;
        let line = clip(line, length);

        self.lines.push(line);
    }

    pub fn draw(&self) -> anyhow::Result<()> {
        let mut out = stdout();

        // Clear any residual lines left from a previous longer draw.
        self.clear()?;

        for line in &self.lines {
            writeln!(out, "{line}")?;
        }

        // Move cursor back to top so the next draw call overwrites in-place.
        execute!(stdout(), MoveUp(self.lines.len() as u16))?;

        Ok(())
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        execute!(stdout(), Clear(ClearType::FromCursorDown))?;
        Ok(())
    }
}
