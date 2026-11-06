//! Golden round-trip tests for TermGrid::snapshot (spec section 11, item 1).
//! Each fixture is fed into grid A; A's snapshot is fed into a fresh grid B;
//! the canonical dumps of A and B must be identical.

use jojo_grid::TermGrid;

const COLS: u16 = 80;
const ROWS: u16 = 24;
const SCROLLBACK: usize = 10_000;

fn assert_roundtrip(name: &str, input: &[u8]) {
    let mut a = TermGrid::new(COLS, ROWS, SCROLLBACK);
    a.feed(input);
    let snapshot = a.snapshot();
    let mut b = TermGrid::new(COLS, ROWS, SCROLLBACK);
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
