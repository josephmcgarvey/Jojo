//! Terminal grid for one session, backed by `alacritty_terminal`.

mod dump;
mod snapshot;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use std::sync::{Arc, Mutex};
use vte::ansi::{Processor, Rgb};

/// Collects bytes the emulator wants to send back to the program, such as
/// the answer to a cursor position request (`ESC [ 6 n`) or a color query
/// (`ESC ] 11 ; ? BEL`). Replies are appended in the order the queries
/// appear in the output.
///
/// Color queries must be answered here, in that order, and not by the
/// window. Programs such as `gh` (through termenv) send `OSC 11 ; ?` and
/// then a cursor position request as an end marker, and stop reading at the
/// first reply that is not a color: when the window answered the color
/// query, its reply arrived after the daemon's cursor reply, the program had
/// already exited, and the shell received `ESC ]11;rgb:0808/0b0b/1616` as
/// typed text.
#[derive(Clone, Default)]
struct ReplyCollector(Arc<Mutex<Vec<u8>>>);

impl EventListener for ReplyCollector {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.0.lock().expect("reply lock").extend_from_slice(text.as_bytes()),
            Event::ColorRequest(index, format) => {
                self.0.lock().expect("reply lock").extend_from_slice(format(reported_color(index)).as_bytes());
            }
            _ => {}
        }
    }
}

/// The 16 ANSI colors of the Neon Glass terminal palette
/// (app/src/themes/neon-glass.json, `terminal`).
const ANSI: [(u8, u8, u8); 16] = [
    (0x1b, 0x20, 0x40),
    (0xff, 0x5c, 0x7a),
    (0x3e, 0xe6, 0xb0),
    (0xff, 0xb3, 0x47),
    (0x7f, 0xa8, 0xff),
    (0xc4, 0x9b, 0xff),
    (0x7f, 0xd4, 0xff),
    (0xd6, 0xdc, 0xff),
    (0x5a, 0x63, 0x90),
    (0xff, 0x8a, 0xa0),
    (0x7f, 0xf0, 0xc8),
    (0xff, 0xcc, 0x80),
    (0xa8, 0xc4, 0xff),
    (0xdc, 0xc4, 0xff),
    (0xb0, 0xe6, 0xff),
    (0xff, 0xff, 0xff),
];
const FOREGROUND: (u8, u8, u8) = (0xd6, 0xdc, 0xff);
const BACKGROUND: (u8, u8, u8) = (0x08, 0x0b, 0x16);
const CURSOR: (u8, u8, u8) = (0x9b, 0x8c, 0xff);

/// The color reported for a query of palette `index`: 0 to 15 the ANSI
/// colors, 16 to 231 the xterm 6x6x6 color cube, 232 to 255 the xterm gray
/// ramp, 256 the foreground (OSC 10), 257 the background (OSC 11) and 258
/// the cursor (OSC 12). Any other index reports the foreground.
///
/// These are the Neon Glass defaults. The daemon does not know the theme
/// or base color the window shows (Settings), so a program asking under
/// Daylight or another base color is told the Neon Glass colors.
fn reported_color(index: usize) -> Rgb {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let (r, g, b) = match index {
        0..=15 => ANSI[index],
        16..=231 => {
            let i = index - 16;
            (CUBE[i / 36], CUBE[(i / 6) % 6], CUBE[i % 6])
        }
        232..=255 => {
            let level = (8 + 10 * (index - 232)) as u8;
            (level, level, level)
        }
        257 => BACKGROUND,
        258 => CURSOR,
        _ => FOREGROUND,
    };
    Rgb { r, g, b }
}

/// Smallest grid width accepted by `TermGrid::new` and `resize`. Below this,
/// alacritty_terminal's grid resize can panic (C2).
pub const MIN_COLS: u16 = 2;
/// Largest grid width accepted. Above this, the allocation becomes
/// unreasonably large: 65535x65535 attempts to allocate roughly 100 GB and
/// aborts the process (C2).
pub const MAX_COLS: u16 = 1000;
/// Smallest grid height accepted by `TermGrid::new` and `resize`.
pub const MIN_ROWS: u16 = 1;
/// Largest grid height accepted, for the same reason as `MAX_COLS`.
pub const MAX_ROWS: u16 = 1000;

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
        let cols = cols.clamp(MIN_COLS, MAX_COLS);
        let rows = rows.clamp(MIN_ROWS, MAX_ROWS);
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
        let cols = cols.clamp(MIN_COLS, MAX_COLS);
        let rows = rows.clamp(MIN_ROWS, MAX_ROWS);
        self.term.resize(GridSize { cols: cols as usize, rows: rows as usize });
    }

    /// Returns and clears the bytes the emulator generated as replies to
    /// queries in the output. The daemon always writes these to the PTY: it
    /// is the sole responder to terminal queries (DSR, DA, DECRQM, and the
    /// like), and an attached window must not also answer them, since the
    /// program issuing the query only expects one reply.
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
            lines.push(self.row_text(Line(line)).0.trim_end().to_string());
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    /// One grid row as text (wide-character spacers skipped, tabs as
    /// spaces), and whether the row continues on the next one because the
    /// terminal wrapped it (the last cell's WRAPLINE flag).
    fn row_text(&self, line: Line) -> (String, bool) {
        let grid = self.term.grid();
        let row = &grid[line];
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
        let wraps = row[Column(grid.columns() - 1)].flags.contains(Flags::WRAPLINE);
        (text, wraps)
    }

    /// Finds `query` in the scrollback and the screen, ignoring case, and
    /// returns at most `max` matching lines, newest first. A line the
    /// terminal wrapped over several rows is searched as one logical line,
    /// so a match split by the wrap is found. Trailing spaces are removed and
    /// lines longer than MATCH_TEXT_CHARS are shortened around the match.
    pub fn search(&self, query: &str, max: usize) -> Vec<GridMatch> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() || max == 0 {
            return Vec::new();
        }
        let grid = self.term.grid();
        let first = -(grid.history_size() as i32);
        // Logical lines, oldest first: rows joined while a row wraps.
        let mut logical: Vec<String> = Vec::new();
        let mut current = String::new();
        for line in first..grid.screen_lines() as i32 {
            let (text, wraps) = self.row_text(Line(line));
            current.push_str(&text);
            if !wraps {
                logical.push(std::mem::take(&mut current).trim_end().to_string());
            }
        }
        if !current.is_empty() {
            logical.push(current.trim_end().to_string());
        }
        let total = logical.len();
        let mut matches = Vec::new();
        for (index, text) in logical.iter().enumerate().rev() {
            if let Some(at) = text.to_lowercase().find(&needle) {
                matches.push(GridMatch { text: excerpt(text, at, needle.len()), lines_from_bottom: (total - 1 - index) as u32 });
                if matches.len() == max {
                    break;
                }
            }
        }
        matches
    }
}

/// The longest line text a search match returns; longer lines are cut to a
/// window around the match.
pub const MATCH_TEXT_CHARS: usize = 240;

/// One line found by `TermGrid::search`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridMatch {
    pub text: String,
    /// How many logical lines are below this one (0 for the last line).
    pub lines_from_bottom: u32,
}

/// `text` cut to MATCH_TEXT_CHARS characters around the match at byte
/// offset `at` (of the lowercased text, whose offsets equal the original's
/// for the ASCII text terminals mostly show), with "…" where it was cut.
fn excerpt(text: &str, at: usize, len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MATCH_TEXT_CHARS {
        return text.to_string();
    }
    let at_char = text.char_indices().take_while(|(i, _)| *i < at).count();
    let len_chars = len.min(MATCH_TEXT_CHARS);
    let start = at_char.saturating_sub((MATCH_TEXT_CHARS - len_chars) / 2).min(chars.len() - MATCH_TEXT_CHARS);
    let end = start + MATCH_TEXT_CHARS;
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
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
    fn color_queries_are_answered_in_order_with_the_other_replies() {
        let mut g = TermGrid::new(80, 24, 1000);
        // termenv's background query followed by its end marker.
        g.feed(b"\x1b]11;?\x07\x1b[6n");
        assert_eq!(g.take_replies(), b"\x1b]11;rgb:0808/0b0b/1616\x07\x1b[1;1R".to_vec());
        g.feed(b"\x1b]10;?\x1b\\\x1b]4;1;?\x07\x1b]4;196;?\x07");
        assert_eq!(
            String::from_utf8(g.take_replies()).unwrap(),
            "\x1b]10;rgb:d6d6/dcdc/ffff\x1b\\\x1b]4;1;rgb:ffff/5c5c/7a7a\x07\x1b]4;196;rgb:ffff/0000/0000\x07"
        );
    }

    #[test]
    fn search_finds_lines_newest_first_ignoring_case() {
        let mut g = TermGrid::new(40, 5, 1000);
        for i in 0..30 {
            g.feed(format!("line {i}\r\n").as_bytes());
        }
        g.feed(b"Error: ECONNREFUSED 127.0.0.1:5432\r\nok\r\nerror again\r\n");
        let found = g.search("error", 10);
        assert_eq!(found.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), ["error again", "Error: ECONNREFUSED 127.0.0.1:5432"]);
        assert!(found[0].lines_from_bottom < found[1].lines_from_bottom);
        // Lines that scrolled off the screen are searched too.
        assert_eq!(g.search("LINE 3", 5).iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), ["line 3"]);
        assert_eq!(g.search("error", 1).len(), 1);
        assert!(g.search("   ", 10).is_empty());
        assert!(g.search("absent", 10).is_empty());
    }

    #[test]
    fn search_joins_lines_the_terminal_wrapped() {
        let mut g = TermGrid::new(10, 5, 100);
        // 24 characters on a 10-column grid wrap over three rows; the match
        // spans the first wrap.
        g.feed(b"aaaaaaaFAILEDbbbbbbbbbbb\r\n");
        let found = g.search("failed", 5);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "aaaaaaaFAILEDbbbbbbbbbbb");
    }

    #[test]
    fn search_shortens_long_lines_around_the_match() {
        let mut g = TermGrid::new(500, 5, 100);
        let long = format!("{}NEEDLE{}", "x".repeat(200), "y".repeat(200));
        g.feed(format!("{long}\r\n").as_bytes());
        let text = &g.search("needle", 1)[0].text;
        assert!(text.contains("NEEDLE"));
        assert!(text.starts_with('…') && text.ends_with('…'));
        assert_eq!(text.chars().count(), MATCH_TEXT_CHARS + 2);
    }

    #[test]
    fn grid_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TermGrid>();
    }

    #[test]
    fn resize_to_zero_is_clamped_and_feed_does_not_panic() {
        // C2: `Resize { cols: 0, rows: 0 }` panics inside alacritty's grid
        // resize. `TermGrid::resize` clamps to `MIN_COLS..=MAX_COLS` /
        // `MIN_ROWS..=MAX_ROWS` as defense in depth, independent of
        // jojod's own validation in `handle_control`.
        let mut g = TermGrid::new(80, 24, 1000);
        g.resize(0, 0);
        g.feed(b"x");
        assert_eq!(g.cols(), MIN_COLS);
        assert_eq!(g.rows(), MIN_ROWS);
    }

    #[test]
    fn new_clamps_sizes_out_of_range() {
        // C2: `TermGrid::new` clamps the same way `resize` does, so a
        // CreateSession with an out-of-range size cannot reach alacritty's
        // allocator unclamped even if jojod's own validation were bypassed.
        let g = TermGrid::new(0, 0, 1000);
        assert_eq!((g.cols(), g.rows()), (MIN_COLS, MIN_ROWS));
        let g = TermGrid::new(65535, 65535, 1000);
        assert_eq!((g.cols(), g.rows()), (MAX_COLS, MAX_ROWS));
    }
}
