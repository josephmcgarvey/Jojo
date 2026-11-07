#[test]
fn log_lines_are_written_to_a_dated_file() {
    let dir = tempfile::tempdir().unwrap();
    let guard = jojod::logging::init(dir.path()).unwrap();
    tracing::info!(session = "test", "logging works");
    drop(guard); // flushes the background writer
    let log_dir = dir.path().join("logs");
    let files: Vec<_> = std::fs::read_dir(&log_dir).unwrap().filter_map(|e| e.ok()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().to_string_lossy().into_owned();
    assert!(name.starts_with("jojod.") && name.ends_with(".log"), "{name}");
    let text = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(text.contains("logging works"), "{text}");
}
