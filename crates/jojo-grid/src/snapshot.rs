//! Converts a TermGrid back into VT sequences (spec section 5.5).
//!
//! Limits (Plan 1, "Known Gaps Carried Forward"):
//! 1. On the alternate screen only the alternate screen is serialized;
//!    alacritty_terminal keeps the inactive primary grid private.
//! 2. Scroll region (DECSTBM), origin mode and title are not restored.
//! 3. Tab cells are written as spaces. alacritty_terminal stores '\t' only in
//!    the first cell a tab covers, so writing '\t' would move the cursor
//!    past cells that are then written again.

use crate::TermGrid;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use vte::ansi::{Color, CursorShape, CursorStyle, NamedColor};

/// Cell flags that map to SGR attributes.
pub(crate) const STYLE_FLAGS: Flags = Flags::BOLD
    .union(Flags::DIM)
    .union(Flags::ITALIC)
    .union(Flags::UNDERLINE)
    .union(Flags::DOUBLE_UNDERLINE)
    .union(Flags::UNDERCURL)
    .union(Flags::DOTTED_UNDERLINE)
    .union(Flags::DASHED_UNDERLINE)
    .union(Flags::INVERSE)
    .union(Flags::HIDDEN)
    .union(Flags::STRIKEOUT);

/// Terminal modes the snapshot restores. Used by debug_dump to compare only
/// what is restored.
pub(crate) const RESTORED_MODES: TermMode = TermMode::ALT_SCREEN
    .union(TermMode::APP_CURSOR)
    .union(TermMode::APP_KEYPAD)
    .union(TermMode::BRACKETED_PASTE)
    .union(TermMode::MOUSE_REPORT_CLICK)
    .union(TermMode::MOUSE_DRAG)
    .union(TermMode::MOUSE_MOTION)
    .union(TermMode::SGR_MOUSE)
    .union(TermMode::UTF8_MOUSE)
    .union(TermMode::FOCUS_IN_OUT)
    .union(TermMode::SHOW_CURSOR)
    .union(TermMode::LINE_WRAP);

/// DEC private modes restored when set, as (mode flag, parameter).
const PRIVATE_MODES: [(TermMode, &str); 8] = [
    (TermMode::APP_CURSOR, "1"),
    (TermMode::MOUSE_REPORT_CLICK, "1000"),
    (TermMode::MOUSE_DRAG, "1002"),
    (TermMode::MOUSE_MOTION, "1003"),
    (TermMode::FOCUS_IN_OUT, "1004"),
    (TermMode::UTF8_MOUSE, "1005"),
    (TermMode::SGR_MOUSE, "1006"),
    (TermMode::BRACKETED_PASTE, "2004"),
];

#[derive(Clone, Copy, PartialEq)]
struct Pen {
    fg: Color,
    bg: Color,
    flags: Flags,
}

impl Pen {
    const DEFAULT: Pen = Pen {
        fg: Color::Named(NamedColor::Foreground),
        bg: Color::Named(NamedColor::Background),
        flags: Flags::empty(),
    };

    fn of(cell: &Cell) -> Pen {
        Pen { fg: cell.fg, bg: cell.bg, flags: cell.flags & STYLE_FLAGS }
    }

    /// Writes a full SGR sequence that starts from reset.
    fn write(&self, out: &mut Vec<u8>) {
        let mut s = String::from("\x1b[0");
        let attrs = [
            (Flags::BOLD, ";1"),
            (Flags::DIM, ";2"),
            (Flags::ITALIC, ";3"),
            (Flags::UNDERLINE, ";4"),
            // vte 0.15 parses bare `21` as CancelBold (ansi.rs:1849), not
            // double underline; double underline is the colon-subparameter
            // form `4:2` (ansi.rs:1839).
            (Flags::DOUBLE_UNDERLINE, ";4:2"),
            (Flags::UNDERCURL, ";4:3"),
            (Flags::DOTTED_UNDERLINE, ";4:4"),
            (Flags::DASHED_UNDERLINE, ";4:5"),
            (Flags::INVERSE, ";7"),
            (Flags::HIDDEN, ";8"),
            (Flags::STRIKEOUT, ";9"),
        ];
        for (flag, code) in attrs {
            if self.flags.contains(flag) {
                s.push_str(code);
            }
        }
        push_color(&mut s, self.fg, false);
        push_color(&mut s, self.bg, true);
        s.push('m');
        out.extend_from_slice(s.as_bytes());
    }
}

fn push_color(s: &mut String, color: Color, background: bool) {
    match color {
        Color::Named(named) => {
            let n = named as usize;
            if n < 8 {
                s.push_str(&format!(";{}", if background { 40 } else { 30 } + n));
            } else if n < 16 {
                s.push_str(&format!(";{}", if background { 100 } else { 90 } + n - 8));
            }
            // Foreground, Background and the other named colors are the
            // terminal defaults, which the leading reset already selects.
        }
        Color::Indexed(i) => s.push_str(&format!(";{};5;{i}", if background { 48 } else { 38 })),
        Color::Spec(rgb) => s.push_str(&format!(
            ";{};2;{};{};{}",
            if background { 48 } else { 38 },
            rgb.r,
            rgb.g,
            rgb.b
        )),
    }
}

fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && Pen::of(cell) == Pen::DEFAULT
        && cell.zerowidth().is_none()
}

impl TermGrid {
    fn snapshot_skipping(&self, skip_history: usize) -> Vec<u8> {
        let grid = self.term.grid();
        let mode = *self.term.mode();
        let cols = grid.columns();
        let mut out = Vec::with_capacity(64 * 1024);

        // Full reset, then clear scrollback, so the receiver starts empty.
        out.extend_from_slice(b"\x1bc\x1b[3J");

        let alt = mode.contains(TermMode::ALT_SCREEN);
        if alt {
            out.extend_from_slice(b"\x1b[?1049h\x1b[H\x1b[2J");
        }

        // Rows from the oldest scrollback line to the bottom of the screen.
        // Writing history + screen rows with CRLF between them scrolls the
        // history rows into the receiver's scrollback.
        let history = grid.history_size();
        let kept = history - skip_history.min(history);
        let top = if alt { 0 } else { -(kept as i32) };
        let bottom = grid.screen_lines() as i32 - 1;
        let mut pen = Pen::DEFAULT;
        // C1: whether the row just written left the receiver's cursor
        // pending-wrap at the last column. `\x1b[K` (used below to clear a
        // row's untouched tail) is a no-op while pending-wrap is set
        // (alacritty term/mod.rs:1643; xterm.js matches), so if this row
        // would otherwise write nothing, the following `\r\n` clears
        // pending-wrap and lands directly on this row without ever forcing
        // the wrap: the blank continuation row is lost, WRAPLINE never gets
        // set on the previous row in the receiver, and every later row
        // shifts by one.
        let mut prev_wrapped = false;
        for line in top..=bottom {
            let row = &grid[Line(line)];
            let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
            // A wrapped row is written in full so the next character wraps
            // naturally and sets WRAPLINE in the receiver. Other rows stop
            // after the last non-blank cell; except, per the `prev_wrapped`
            // comment above, a row immediately after a wrapped row always
            // writes at least one default-pen space (column 0, which is
            // genuinely blank whenever `end` would otherwise be 0 — that is
            // what made it blank) so the wrap is actually forced.
            let end = if wrapped {
                cols
            } else {
                let found = (0..cols).rev().find(|&c| !is_blank(&row[Column(c)])).map_or(0, |c| c + 1);
                if prev_wrapped {
                    found.max(1)
                } else {
                    found
                }
            };
            for col in 0..end {
                let cell = &row[Column(col)];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let cell_pen = Pen::of(cell);
                if cell_pen != pen {
                    cell_pen.write(&mut out);
                    pen = cell_pen;
                }
                let c = if cell.c == '\t' { ' ' } else { cell.c };
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                if let Some(extra) = cell.zerowidth() {
                    for z in extra {
                        out.extend_from_slice(z.encode_utf8(&mut buf).as_bytes());
                    }
                }
            }
            if !wrapped {
                if end < cols {
                    // Columns beyond `end` were not written because they
                    // matched the default pen (see is_blank), but
                    // Grid::scroll_up reveals a newly scrolled-in row by
                    // resetting it with whichever pen is active at that
                    // instant. If this row was itself revealed with a
                    // non-default pen and the program later cleared its
                    // tail back to default explicitly (e.g. EL) rather than
                    // simply never reaching it, the untouched columns are
                    // genuinely default here, but the receiver's own reveal
                    // of this same row (which lands on the same pen, since
                    // its timing mirrors ours byte for byte) would leave
                    // them colored unless told otherwise. Erase them
                    // explicitly so they match regardless of what the
                    // receiver's own scroll left behind.
                    out.extend_from_slice(b"\x1b[0m\x1b[K");
                    pen = Pen::DEFAULT;
                } else if pen != Pen::DEFAULT {
                    // Reset before the line feed so a scroll does not fill
                    // the new row with the current background color.
                    out.extend_from_slice(b"\x1b[0m");
                    pen = Pen::DEFAULT;
                }
                if line != bottom {
                    out.extend_from_slice(b"\r\n");
                }
            }
            prev_wrapped = wrapped;
        }

        // Cursor position (1-based), then the pen the program had selected.
        //
        // If the cursor is pending-wrap (the program filled the last column
        // and the *next* character printed will wrap first), a plain CUP to
        // that column is not enough: CUP always clears pending-wrap in
        // alacritty_terminal, so the receiver would treat the next write as
        // landing in that column rather than wrapping first. Instead,
        // position on that column and re-emit the cell already there, so
        // writing it sets pending-wrap in the receiver the same way the
        // original write did, rather than merely moving the cursor there.
        let cursor = &grid.cursor;
        if cursor.input_needs_wrap {
            let row = &grid[cursor.point.line];
            let last = Column(cols - 1);
            // A wide character's trailing spacer cell carries no content of
            // its own; the glyph is in the cell before it, so re-emitting
            // the spacer's column must instead write the leading cell.
            let (col, cell) = if row[last].flags.contains(Flags::WIDE_CHAR_SPACER) {
                (cols - 2, &row[Column(cols - 2)])
            } else {
                (cols - 1, &row[last])
            };
            out.extend_from_slice(format!("\x1b[{};{}H", cursor.point.line.0 + 1, col + 1).as_bytes());
            Pen::of(cell).write(&mut out);
            let mut buf = [0u8; 4];
            out.extend_from_slice(cell.c.encode_utf8(&mut buf).as_bytes());
            if let Some(extra) = cell.zerowidth() {
                for z in extra {
                    out.extend_from_slice(z.encode_utf8(&mut buf).as_bytes());
                }
            }
        } else {
            out.extend_from_slice(
                format!("\x1b[{};{}H", cursor.point.line.0 + 1, cursor.point.column.0 + 1).as_bytes(),
            );
        }
        Pen::of(&cursor.template).write(&mut out);

        for (flag, param) in PRIVATE_MODES {
            if mode.contains(flag) {
                out.extend_from_slice(format!("\x1b[?{param}h").as_bytes());
            }
        }
        if mode.contains(TermMode::APP_KEYPAD) {
            out.extend_from_slice(b"\x1b=");
        }
        if !mode.contains(TermMode::LINE_WRAP) {
            out.extend_from_slice(b"\x1b[?7l");
        }
        if !mode.contains(TermMode::SHOW_CURSOR) {
            out.extend_from_slice(b"\x1b[?25l");
        }

        // DECSCUSR is emitted only when the resolved style differs from
        // CursorStyle::default() (Block, not blinking) — the same value
        // Config::default().default_cursor_style resolves to, which is what
        // TermGrid::new builds its Term with. term.cursor_style() always
        // resolves to a concrete style, including when the program never
        // requested one at all, so emitting it unconditionally would
        // override a receiver's own configured cursor style on every
        // snapshot. alacritty_terminal does not expose whether the style
        // was set explicitly (Term::cursor_style is a private field), so
        // this comparison cannot distinguish "the program never asked" from
        // "the program explicitly asked for the default style"; both emit
        // nothing.
        let style = self.term.cursor_style();
        if style != CursorStyle::default() {
            let base = match style.shape {
                CursorShape::Block => Some(1),
                CursorShape::Underline => Some(3),
                CursorShape::Beam => Some(5),
                _ => None,
            };
            if let Some(base) = base {
                let n = if style.blinking { base } else { base + 1 };
                out.extend_from_slice(format!("\x1b[{n} q").as_bytes());
            }
        }

        out
    }

    /// VT bytes that reproduce this grid in a fresh terminal of the same size
    /// and scrollback capacity.
    pub fn snapshot(&self) -> Vec<u8> {
        self.snapshot_skipping(0)
    }

    /// Like `snapshot`, but drops the oldest scrollback rows until the result
    /// is at most `max_len` bytes. A frame larger than `MAX_FRAME_LEN` would be
    /// rejected by the client and end its whole connection (Plan 1 carry-over
    /// 2.1, item 2). If the visible screen alone is still too large, which only
    /// a very large grid of truecolor cells can produce, the result is the
    /// screen as plain text with no attributes.
    pub fn snapshot_bounded(&self, max_len: usize) -> Vec<u8> {
        let full = self.snapshot_skipping(0);
        if full.len() <= max_len {
            return full;
        }
        // On the alternate screen, `snapshot_skipping` always serializes only
        // the alternate screen itself (Limit 1 above): there is no
        // scrollback to drop, so every candidate the binary search below
        // would try is identical to `full`. Go straight to the fallback.
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return self.bounded_plain_screen_snapshot(max_len);
        }
        // Smallest number of skipped history rows whose snapshot fits.
        let (mut lo, mut hi) = (1usize, self.term.grid().history_size());
        let mut best = None;
        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            let candidate = self.snapshot_skipping(mid);
            if candidate.len() <= max_len {
                best = Some(candidate);
                hi = mid - 1;
            } else {
                lo = mid + 1;
            }
        }
        best.unwrap_or_else(|| self.bounded_plain_screen_snapshot(max_len))
    }

    /// The visible screen as plain text after a full reset, truncated to
    /// `max_len` on a UTF-8 character boundary if it would otherwise exceed
    /// it (only a screen of implausible size reaches this: `MAX_COLS` x
    /// `MAX_ROWS` plain ASCII text alone is well under `MAX_SNAPSHOT_BYTES`).
    /// On the alternate screen, also switches the receiver to the alternate
    /// screen so it ends up in the same mode this grid is in.
    fn bounded_plain_screen_snapshot(&self, max_len: usize) -> Vec<u8> {
        let mut out = b"\x1bc\x1b[3J".to_vec();
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            out.extend_from_slice(b"\x1b[?1049h");
        }
        let text = self.visible_text().replace('\n', "\r\n");
        let budget = max_len.saturating_sub(out.len());
        let mut end = text.len().min(budget);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        out.extend_from_slice(&text.as_bytes()[..end]);
        out
    }
}
