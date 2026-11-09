// The platform the app runs on, read once from the webview. WKWebView on
// macOS reports a navigator.platform of "MacIntel" on both Apple Silicon and
// Intel Macs; WebView2 on Windows reports "Win32".

export const isMac = typeof navigator !== "undefined" && /^Mac/.test(navigator.platform);

/** The local machine's name, matching the backend's machines.toml default. */
export const LOCAL_LABEL = isMac ? "This Mac" : "This PC";

/** The operating system's name in messages ("when macOS restarted"). */
export const OS_NAME = isMac ? "macOS" : "Windows";
