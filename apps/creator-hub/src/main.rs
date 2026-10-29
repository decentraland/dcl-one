// Native shell for the Creator Hub. The window is created in code (not
// tauri.conf.json) so the target URL can come from the environment:
//   DCL_HUB_URL=http://127.0.0.1:5159/create  (local build)
//   DCL_HUB_URL=https://example.org/create     (a deployment; required, no default)
// capabilities/default.json allows loopback pages only; the origin of
// DCL_HUB_URL gets the same IPC permissions as a runtime capability
// (`hub-remote`), so an operator's own domain needs no JSON edit.
//
// Sign-in: the webview can't host wallet extensions and OAuth providers
// dislike webviews, so the page's "Continue in your browser" flow navigates
// to /auth/native?id=... — we intercept that navigation here and open it in
// the user's default browser instead (where they're already signed in). The
// page detects the shell via the injected window.__DCL_SHELL__ marker and
// polls the same-origin relay for the approval. See
// catalyrst/sites/packages/data/src/lib/auth/native-shell.ts for the protocol.
//   DCL_SHELL_BROWSER_CMD=<program>  overrides how the URL is opened
//   (program receives the URL as its single argument; for tests/headless).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

const SHELL_MARKER: &str =
    "window.__DCL_SHELL__ = Object.freeze({ shell: 'tauri', version: '0.1.0' });";

fn open_in_browser(url: &str, cmd_override: Option<&str>) {
    let result = match cmd_override {
        Some(cmd) => std::process::Command::new(cmd)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string()),
        None => open::that_detached(url).map_err(|e| e.to_string()),
    };
    match result {
        Ok(()) => eprintln!("[shell] opened in browser: {url}"),
        Err(e) => eprintln!("[shell] FAILED to open browser for {url}: {e}"),
    }
}

#[tauri::command]
fn mp_hello(name: String) -> String {
    eprintln!("[mp] mp_hello invoked: {name}");
    format!("hello {name} from rust")
}

#[tauri::command]
fn mp_log(msg: String) {
    eprintln!("[mp] js: {msg}");
}

fn main() {
    let url = std::env::var("DCL_HUB_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| {
            eprintln!(
                "[shell] set DCL_HUB_URL to the Creator Hub page, e.g. https://example.org/create"
            );
            std::process::exit(2)
        });
    let browser_cmd = std::env::var("DCL_SHELL_BROWSER_CMD").ok();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![mp_hello, mp_log])
        .setup(move |app| {
            let external: tauri::Url = url.parse()?;
            if matches!(external.scheme(), "http" | "https") {
                app.add_capability(
                    tauri::ipc::CapabilityBuilder::new("hub-remote")
                        .remote(external.origin().ascii_serialization())
                        .window("main")
                        .permission("core:default")
                        .permission("allow-mp-hello")
                        .permission("allow-mp-log"),
                )?;
            }
            let browser_cmd = browser_cmd.clone();
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::External(external))
                .title("Decentraland Creator Hub")
                .inner_size(1600.0, 900.0)
                .initialization_script(SHELL_MARKER)
                .on_navigation(move |nav| {
                    if nav.query().is_some_and(|q| q.contains("mp_ipc_error=")) {
                        eprintln!("[mp] ipc failure beacon: {nav}");
                    }
                    // The external-browser sign-in leg: never load the approve
                    // page in the webview; hand it to the real browser.
                    if nav.path() == "/auth/native"
                        && nav.query().is_some_and(|q| q.contains("id="))
                    {
                        open_in_browser(nav.as_str(), browser_cmd.as_deref());
                        return false;
                    }
                    true
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run creator-hub shell");
}
