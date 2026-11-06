//! Canonical text form of a TermGrid, used to compare grids in tests and
//! when debugging a snapshot mismatch.

use crate::TermGrid;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use std::fmt::Write;

impl TermGrid {
    /// One line per grid row (oldest scrollback first), each listing every
    /// cell as `char/fg/bg/flags`, followed by cursor, pen, mode and cursor
    /// style lines. Tab cells are written as spaces (see snapshot.rs).
    pub fn debug_dump(&self) -> String {
        let grid = self.term.grid();
        let mut out = String::new();
        let history = grid.history_size() as i32;
        writeln!(out, "size {}x{} history {}", grid.columns(), grid.screen_lines(), history).unwrap();
        for line in -history..grid.screen_lines() as i32 {
            let row = &grid[Line(line)];
            write!(out, "{line:>6}:").unwrap();
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                let c = if cell.c == '\t' { ' ' } else { cell.c };
                write!(out, " {c:?}/{:?}/{:?}/{:#x}", cell.fg, cell.bg, cell.flags.bits()).unwrap();
                if let Some(extra) = cell.zerowidth() {
                    write!(out, "+{extra:?}").unwrap();
                }
            }
            out.push('\n');
        }
        let cursor = &grid.cursor;
        writeln!(out, "cursor {:?} {:?}", cursor.point.line, cursor.point.column).unwrap();
        writeln!(
            out,
            "pen {:?}/{:?}/{:#x}",
            cursor.template.fg,
            cursor.template.bg,
            (cursor.template.flags & crate::snapshot::STYLE_FLAGS).bits()
        )
        .unwrap();
        writeln!(out, "mode {:#x}", (*self.term.mode() & crate::snapshot::RESTORED_MODES).bits()).unwrap();
        writeln!(out, "cursor_style {:?}", self.term.cursor_style()).unwrap();
        out
    }
}
