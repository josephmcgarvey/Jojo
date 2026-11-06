//! Terminal grid for one session, backed by `alacritty_terminal`.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use std::sync::{Arc, Mutex};
use vte::ansi::Processor;

/// Collects bytes the emulator wants to send back to the program, such as
/// the answer to a cursor position request (`ESC [ 6 n`).
#[derive(Clone, Default)]
struct ReplyCollector(Arc<Mutex<Vec<u8>>>);

impl EventListener for ReplyCollector {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.lock().expect("reply lock").extend_from_slice(text.as_bytes());
        }
    }
}

/// Grid size in the form `alacritty_terminal` expects.
struct GridSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// The screen and scrollback of one session.
pub struct TermGrid {
    term: Term<ReplyCollector>,
    parser: Processor,
    replies: ReplyCollector,
}

impl TermGrid {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        let replies = ReplyCollector::default();
        let config = Config { scrolling_history: scrollback, ..Config::default() };
        let size = GridSize { cols: cols as usize, rows: rows as usize };
        let term = Term::new(config, &size, replies.clone());
        Self { term, parser: Processor::new(), replies }
    }

    /// Interprets program output.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize { cols: cols as usize, rows: rows as usize });
    }

    /// Returns and clears the bytes the emulator generated as replies to
    /// queries in the output. The daemon writes these to the PTY only when
    /// no window is attached, because an attached xterm.js answers itself.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.replies.0.lock().expect("reply lock"))
    }

    pub fn cols(&self) -> u16 {
        self.term.grid().columns() as u16
    }

    pub fn rows(&self) -> u16 {
        self.term.grid().screen_lines() as u16
    }

    /// The visible screen as plain text: one line per row with trailing
    /// spaces removed, and trailing empty rows removed.
    pub fn visible_text(&self) -> String {
        let grid = self.term.grid();
        let mut lines = Vec::with_capacity(grid.screen_lines());
        for line in 0..grid.screen_lines() as i32 {
            let row = &grid[Line(line)];
            let mut text = String::new();
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                text.push(if cell.c == '\t' { ' ' } else { cell.c });
                if let Some(extra) = cell.zerowidth() {
                    text.extend(extra.iter());
                }
            }
            lines.push(text.trim_end().to_string());
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_shows_text_on_screen() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"hello\r\nworld");
        let text = g.visible_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn colors_do_not_appear_in_visible_text() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"\x1b[31mred\x1b[0m plain");
        assert_eq!(g.visible_text(), "red plain");
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.resize(100, 30);
        assert_eq!((g.cols(), g.rows()), (100, 30));
    }

    #[test]
    fn cursor_position_report_is_collected_as_reply() {
        let mut g = TermGrid::new(80, 24, 1000);
        g.feed(b"ab\x1b[6n");
        assert_eq!(g.take_replies(), b"\x1b[1;3R".to_vec());
        assert!(g.take_replies().is_empty());
    }

    #[test]
    fn grid_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TermGrid>();
    }
}
