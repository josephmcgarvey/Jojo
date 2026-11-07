//! Golden round-trip tests for TermGrid::snapshot (spec section 11, item 1).
//! Each fixture is fed into grid A; A's snapshot is fed into a fresh grid B;
//! the canonical dumps of A and B must be identical.

use jojo_grid::TermGrid;

const COLS: u16 = 80;
const ROWS: u16 = 24;
const SCROLLBACK: usize = 10_000;

fn assert_roundtrip(name: &str, input: &[u8]) {
    assert_roundtrip_sized(name, COLS, ROWS, SCROLLBACK, input);
}

fn assert_roundtrip_sized(name: &str, cols: u16, rows: u16, scrollback: usize, input: &[u8]) {
    let mut a = TermGrid::new(cols, rows, scrollback);
    a.feed(input);
    let snapshot = a.snapshot();
    let mut b = TermGrid::new(cols, rows, scrollback);
    b.feed(&snapshot);
    let (da, db) = (a.debug_dump(), b.debug_dump());
    if da != db {
        let first_diff = da
            .lines()
            .zip(db.lines())
            .find(|(x, y)| x != y)
            .map(|(x, y)| format!("original: {x}\nrestored: {y}"))
            .unwrap_or_else(|| "dumps differ in length".to_string());
        panic!("fixture {name} did not round-trip\n{first_diff}");
    }
}

#[test]
fn empty_grid() {
    assert_roundtrip("empty", b"");
}

#[test]
fn colors_and_attributes() {
    let mut s = Vec::new();
    s.extend_from_slice(b"\x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munder\x1b[0m\r\n");
    s.extend_from_slice(b"\x1b[7minverse\x1b[0m \x1b[9mstrike\x1b[0m \x1b[8mhidden\x1b[0m\r\n");
    for i in 0..8 {
        s.extend_from_slice(format!("\x1b[{}m{i}\x1b[{}m{i}", 30 + i, 100 + i).as_bytes());
    }
    s.extend_from_slice(b"\x1b[0m\r\n\x1b[38;5;208m256-orange\x1b[48;5;24m on blue\x1b[0m\r\n");
    s.extend_from_slice(b"\x1b[38;2;155;140;255mtruecolor violet\x1b[48;2;7;10;22m on navy\x1b[0m");
    assert_roundtrip("colors_and_attributes", &s);
}

#[test]
fn background_colored_trailing_spaces_survive() {
    assert_roundtrip("bg_spaces", b"text\x1b[44m      \x1b[0m\r\n\x1b[41m\x1b[2K\x1b[0mafter erase");
}

#[test]
fn long_line_wraps() {
    let line: String = (0..200).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    assert_roundtrip("wrap", line.as_bytes());
}

#[test]
fn scrollback_beyond_capacity() {
    let mut s = Vec::new();
    for i in 0..12_000 {
        s.extend_from_slice(format!("line {i}\r\n").as_bytes());
    }
    assert_roundtrip("scrollback", &s);
}

#[test]
fn emoji_cjk_and_combining_marks() {
    let mut s = Vec::new();
    s.extend_from_slice("日本語テキスト 🎉 ok e\u{301}\r\n".as_bytes());
    s.extend_from_slice("x".repeat(79).as_bytes());
    s.extend_from_slice("漢 wide char forced to wrap".as_bytes());
    assert_roundtrip("wide", &s);
}

#[test]
fn claude_style_permission_prompt_with_redraw() {
    let mut s = Vec::new();
    s.extend_from_slice(b"\x1b[?25l\x1b[?2004h");
    s.extend_from_slice("\x1b[38;2;155;140;255m●\x1b[0m Bash(git commit -m \"docs: add spec\")\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m╭──────────────────────────╮\x1b[0m\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m│\x1b[0m Do you want to proceed?  \x1b[38;2;255;179;71m│\x1b[0m\r\n".as_bytes());
    s.extend_from_slice("\x1b[38;2;255;179;71m╰──────────────────────────╯\x1b[0m\r\n".as_bytes());
    s.extend_from_slice(b"spinner frame 1");
    s.extend_from_slice(b"\r\x1b[2Kspinner frame 2\x1b[3A\x1b[5G");
    assert_roundtrip("permission_prompt", &s);
}

#[test]
fn alternate_screen_with_mouse_modes() {
    let mut s = Vec::new();
    s.extend_from_slice(b"primary content\r\n");
    s.extend_from_slice(b"\x1b[?1049h\x1b[H\x1b[2J");
    s.extend_from_slice(b"\x1b[?1h\x1b=\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1004h");
    s.extend_from_slice(b"\x1b[5;10Hvim-like content\x1b[24;1H-- INSERT --\x1b[2;3H");
    s.extend_from_slice(b"\x1b[6 q");
    assert_roundtrip("alt_screen", &s);
}

#[test]
fn line_wrap_disabled() {
    assert_roundtrip("nowrap", b"\x1b[?7lno wrap mode");
}

// --- Fix round 1 fixtures (task-4-findings-r1.md) ---

#[test]
fn underline_variants_round_trip() {
    // I1: double underline is SGR `4:2`, not `21` (vte 0.15 parses `21` as
    // CancelBold). Exercised alongside the sibling underline subparameters
    // (undercurl, dotted, dashed) to confirm they still round-trip.
    let mut s = Vec::new();
    s.extend_from_slice(b"\x1b[1;4:2mbold_double\x1b[0m \x1b[4:3mundercurl\x1b[0m \x1b[4:4mdotted\x1b[0m \x1b[4:5mdashed\x1b[0m");
    assert_roundtrip("underline_variants", &s);
}

#[test]
fn wrap_at_bottom_does_not_bleed_background() {
    // I2: a natural wrap that scrolls the screen bottom reveals the new row
    // by resetting it with whichever pen is active at that instant
    // (Grid::scroll_up uses cursor.template). Here that pen is blue: the
    // continuation row starts out entirely blue before any text reaches it.
    // An explicit `\x1b[K` (with the pen freshly reset to default) then
    // clears the untouched tail of that row back to true default in A.
    // The snapshot's trim relies on "never written == blank", but grid B's
    // own reveal of the same row is also blue (its reveal timing matches
    // A's byte-for-byte), so without re-emitting the erase, B's untouched
    // tail stays blue while A's is genuinely default.
    let mut s = Vec::new();
    for i in 0..24 {
        s.extend_from_slice(format!("line {i}\r\n").as_bytes());
    }
    s.extend_from_slice(b"\x1b[44m");
    s.extend_from_slice("x".repeat(85).as_bytes());
    s.extend_from_slice(b"\x1b[0m\x1b[K");
    s.extend_from_slice(b"\r\nmore\r\nlines\r\nhere");
    assert_roundtrip("wrap_bottom_bg_bleed", &s);
}

#[test]
fn pending_wrap_at_screen_edge() {
    // I3: exactly 160 printable characters at 80 columns ends with the
    // cursor on row 2 in pending-wrap state (input_needs_wrap), which the
    // snapshot's final CUP otherwise clears.
    let line: String = (0..160).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    assert_roundtrip("pending_wrap", line.as_bytes());
}

#[test]
fn pending_wrap_with_colored_last_cell() {
    // I3: same pending-wrap state, but the last cell on the row carries a
    // non-default pen, exercising the re-emit-last-cell path with a pen
    // change instead of the default pen.
    let mut s = Vec::new();
    let line: String = (0..159).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    s.extend_from_slice(line.as_bytes());
    s.extend_from_slice(b"\x1b[31mZ");
    assert_roundtrip("pending_wrap_colored", &s);
}

// --- Final-review fix wave (final-fixes.md, C1) ---

#[test]
fn blank_continuation_row_after_wrapped_row_round_trips() {
    // C1: row 0 wraps ("0123456789" fills all 10 columns and sets
    // WRAPLINE), and the 11th character (the trailing space) lands on row
    // 1's column 0. Because that space has the default pen, `is_blank`
    // reports row 1 as entirely blank (`end == 0`), which is exactly the
    // trigger case: row 0's WRAPLINE left the receiver's cursor pending-wrap
    // at the last column after replaying row 0, and `\x1b[K` is a no-op
    // while pending-wrap is set, so writing nothing for row 1 would let the
    // following `\r\n` clear pending-wrap and land directly on row 1
    // without ever forcing the wrap, collapsing row 1 into row 0 and
    // shifting "next" and "last" up by one row each.
    assert_roundtrip_sized("blank_continuation_row", 10, 5, 10_000, b"0123456789 \r\nnext\r\nlast");
}

#[test]
fn blank_continuation_row_cleared_by_el_round_trips() {
    // C1: same trigger as above, but row 1 ("AB" wrapped from row 0) is
    // cleared in full by `\x1b[2K` rather than merely holding a disguised
    // default-pen space, so row 1 is genuinely blank going into the
    // snapshot, not just blank by `is_blank`'s definition.
    assert_roundtrip_sized("blank_continuation_row_el", 10, 5, 10_000, b"0123456789AB\x1b[2K\r\nnext");
}

#[test]
fn blank_continuation_row_with_history_round_trips() {
    // C1: the same shift bug, but with enough prior scrollback that the
    // wrapped row and its blank continuation row are no longer the first
    // rows fed, exercising the history-row path of the snapshot loop (`top`
    // starts negative) rather than only the screen-row path.
    let mut s = Vec::new();
    for i in 0..20 {
        s.extend_from_slice(format!("line {i}\r\n").as_bytes());
    }
    s.extend_from_slice(b"0123456789AB\x1b[2K\r\nnext");
    assert_roundtrip_sized("blank_continuation_row_with_history", 10, 5, 10_000, &s);
}

#[test]
fn fresh_grid_snapshot_has_no_cursor_style_sequence() {
    // M4: term.cursor_style() always resolves to a concrete style (falling
    // back to the configured default), so emitting DECSCUSR unconditionally
    // would override a receiver's own configured cursor style even when the
    // program never asked for one.
    let g = TermGrid::new(COLS, ROWS, SCROLLBACK);
    let snapshot = g.snapshot();
    let text = String::from_utf8_lossy(&snapshot);
    assert!(!text.contains(" q"), "unexpected DECSCUSR in snapshot: {text:?}");
}

#[test]
fn bounded_snapshot_fits_and_keeps_the_visible_screen() {
    let mut a = TermGrid::new(80, 24, 10_000);
    let mut input = Vec::new();
    for i in 0..10_000u32 {
        // Every cell carries its own truecolor foreground and background so
        // the full snapshot is far larger than the bound used below.
        for c in 0..80u32 {
            input.extend_from_slice(
                format!("\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}mx", c, i % 256, 7, 255 - c, 3, i % 200).as_bytes(),
            );
        }
        input.extend_from_slice(b"\x1b[0m\r\n");
    }
    input.extend_from_slice(b"last visible line");
    a.feed(&input);

    let full = a.snapshot();
    let limit = 1_000_000;
    assert!(full.len() > limit, "fixture too small: {}", full.len());

    let bounded = a.snapshot_bounded(limit);
    assert!(bounded.len() <= limit, "bounded snapshot is {} bytes", bounded.len());
    // Every cell in the fixture carries a truecolor foreground, which only
    // the binary-search-over-skipped-history path can reproduce; the plain
    // fallback (`bounded_plain_screen_snapshot`) emits no SGR sequences at
    // all. This confirms the bounded snapshot took that path rather than
    // falling back to plain text.
    let text = String::from_utf8_lossy(&bounded);
    assert!(text.contains("\x1b[0;38;2;"), "no truecolor SGR sequence found; fallback ran instead");

    let mut b = TermGrid::new(80, 24, 10_000);
    b.feed(&bounded);
    assert_eq!(b.visible_text(), a.visible_text());
}

#[test]
fn bounded_snapshot_respects_a_tiny_limit() {
    let mut a = TermGrid::new(80, 24, 10_000);
    for i in 0..10_000u32 {
        a.feed(format!("line {i}\r\n").as_bytes());
    }
    a.feed(b"last visible line");

    let limit = 50;
    let bounded = a.snapshot_bounded(limit);
    assert!(bounded.len() <= limit, "bounded snapshot is {} bytes, limit {limit}", bounded.len());
}

#[test]
fn bounded_snapshot_equals_full_snapshot_when_it_fits() {
    let mut a = TermGrid::new(80, 24, 1000);
    a.feed(b"\x1b[31mhello\x1b[0m\r\nworld");
    assert_eq!(a.snapshot_bounded(1_000_000), a.snapshot());
}

// --- Task 5 fixtures (task-5-brief.md) ---

#[test]
fn combined_attributes() {
    assert_roundtrip(
        "combined",
        b"\x1b[1;3;4;7;38;2;1;2;3;48;5;200mcombo\x1b[0m \x1b[2;9;38;5;45mdim-strike\x1b[0m \x1b[1;4:3;31mbold-curl\x1b[0m",
    );
}

#[test]
fn pending_wrap_after_a_wide_character() {
    let mut s = "x".repeat(78).into_bytes();
    s.extend_from_slice("漢".as_bytes());
    assert_roundtrip("pending_wide", &s);
}
