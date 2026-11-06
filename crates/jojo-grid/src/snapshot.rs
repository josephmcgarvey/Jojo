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
use vte::ansi::{Color, CursorShape, NamedColor};

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
            (Flags::DOUBLE_UNDERLINE, ";21"),
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
    /// VT bytes that reproduce this grid in a fresh terminal of the same size
    /// and scrollback capacity.
    pub fn snapshot(&self) -> Vec<u8> {
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
        let top = if alt { 0 } else { -(grid.history_size() as i32) };
        let bottom = grid.screen_lines() as i32 - 1;
        let mut pen = Pen::DEFAULT;
        for line in top..=bottom {
            let row = &grid[Line(line)];
            let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
            // A wrapped row is written in full so the next character wraps
            // naturally and sets WRAPLINE in the receiver. Other rows stop
            // after the last non-blank cell.
            let end = if wrapped {
                cols
            } else {
                (0..cols).rev().find(|&c| !is_blank(&row[Column(c)])).map_or(0, |c| c + 1)
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
            if line != bottom && !wrapped {
                // Reset before the line feed so a scroll does not fill the
                // new row with the current background color.
                if pen != Pen::DEFAULT {
                    out.extend_from_slice(b"\x1b[0m");
                    pen = Pen::DEFAULT;
                }
                out.extend_from_slice(b"\r\n");
            }
        }

        // Cursor position (1-based), then the pen the program had selected.
        let cursor = &grid.cursor;
        out.extend_from_slice(
            format!("\x1b[{};{}H", cursor.point.line.0 + 1, cursor.point.column.0 + 1).as_bytes(),
        );
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

        let style = self.term.cursor_style();
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

        out
    }
}
