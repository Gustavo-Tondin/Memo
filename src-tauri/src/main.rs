// Prevents an extra console window from opening on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMABUF renderer crashes the Wayland connection on several
    // drivers ("Gdk-Message: Error 71 dispatching to Wayland display"), which
    // kills the window before it is drawn. Disabling it must happen before GTK
    // initializes. Honour the value if the user already set one.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    memo_lib::run()
}
