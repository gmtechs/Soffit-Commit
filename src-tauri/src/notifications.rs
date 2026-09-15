/// OS notification helpers (wraps tauri-plugin-notification).
/// Call these from the network event loop when relevant events arrive.
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}
