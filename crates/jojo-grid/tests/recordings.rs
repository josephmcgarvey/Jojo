//! Round-trips every recording in tests/recordings and writes the inputs
//! for tests/xterm-crosscheck into tests/recordings/out (git-ignored).

use jojo_grid::TermGrid;
use std::path::PathBuf;

fn recordings_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/recordings")
}

/// Parses `name.COLSxROWS.bin` into (name, cols, rows).
fn parse_name(file: &str) -> Option<(String, u16, u16)> {
    let stem = file.strip_suffix(".bin")?;
    let (name, size) = stem.rsplit_once('.')?;
    let (cols, rows) = size.split_once('x')?;
    Some((name.to_string(), cols.parse().ok()?, rows.parse().ok()?))
}

#[test]
fn every_recording_round_trips_and_cross_check_inputs_are_written() {
    let dir = recordings_dir();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some((name, cols, rows)) = parse_name(&file) else { continue };
        let bytes = std::fs::read(&path).unwrap();

        let mut a = TermGrid::new(cols, rows, 10_000);
        a.feed(&bytes);
        let snapshot = a.snapshot();
        let mut b = TermGrid::new(cols, rows, 10_000);
        b.feed(&snapshot);
        let (da, db) = (a.debug_dump(), b.debug_dump());
        if da != db {
            let diff = da
                .lines()
                .zip(db.lines())
                .find(|(x, y)| x != y)
                .map(|(x, y)| format!("original: {x}\nrestored: {y}"))
                .unwrap_or_else(|| "dumps differ in length".into());
            panic!("recording {file} did not round-trip\n{diff}");
        }

        std::fs::write(out.join(format!("{name}.snap")), &snapshot).unwrap();
        std::fs::write(out.join(format!("{name}.txt")), a.visible_text()).unwrap();
        std::fs::write(out.join(format!("{name}.size")), format!("{cols} {rows}")).unwrap();
        count += 1;
    }
    assert!(count >= 3, "expected at least 3 recordings, found {count}");
}
