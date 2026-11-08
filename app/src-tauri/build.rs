fn main() {
    // A custom manifest replaces Tauri's default one; it keeps the default's
    // Common Controls 6 dependency and adds supportedOS (see the file).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows-app-manifest.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("failed to run tauri-build");
}
