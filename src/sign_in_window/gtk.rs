//! The sign-in window on Linux: a WebKitGTK view whose profile (the Google sign-in) is kept in
//! Encore's private data. With `silent` it is drawn off screen, so no window ever shows.

use std::cell::Cell;
use std::io::{IsTerminal, Write};
use std::rc::Rc;

use gtk::prelude::*;
use rust_i18n::t;
use webkit2gtk::{
    CookieManagerExt, CookiePersistentStorage, HardwareAccelerationPolicy, LoadEvent, SettingsExt,
    WebContext, WebViewExt, WebsiteDataManager, WebsiteDataManagerExt,
};

use super::{
    CLOSED, LANGUAGE_VAR, MUSIC_URL, NEEDS_YOU, SIGN_IN_URL, SIGNED_IN, SILENT_TIME, cookie_header,
    profile_dir, wants_the_user,
};

/// Shows the sign-in window, or with `silent` signs in with none. Returns the process's exit
/// code: signed in (the cookies went to standard output), closed by the user, could not start
/// (2), or, silent, Google wants the user.
pub fn show(silent: bool) -> i32 {
    // It prints the session cookies, so it only runs for the Encore that reads them.
    if std::io::stdout().is_terminal() || gtk::init().is_err() {
        return 2;
    }
    if let Ok(language) = std::env::var(LANGUAGE_VAR) {
        rust_i18n::set_locale(&language);
    }
    let Some(context) = kept_profile() else {
        return 2;
    };
    // Until it signs in: closed (the window) or the user needed (silent).
    let code = Rc::new(Cell::new(if silent { NEEDS_YOU } else { CLOSED }));
    let view = webkit2gtk::WebView::with_context(&context);
    // Off screen there is no GL context for WebKit to draw with; nothing is seen anyway.
    if silent && let Some(settings) = WebViewExt::settings(&view) {
        settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Never);
    }
    let window: gtk::Window = if silent {
        gtk::OffscreenWindow::new().upcast()
    } else {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title(&t!("sign_in_window.title"));
        window
    };
    window.set_default_size(480, 720);
    window.add(&view);
    window.connect_destroy(|_| gtk::main_quit());

    let result = code.clone();
    view.connect_load_changed(move |view, event| {
        if event != LoadEvent::Finished || result.get() == SIGNED_IN {
            return;
        }
        let uri = view.uri().map(|uri| uri.to_string()).unwrap_or_default();
        // Google shows its sign-in form: with no window, nobody can fill it in.
        if silent && wants_the_user(&uri) {
            gtk::main_quit();
            return;
        }
        if !uri.starts_with(MUSIC_URL) {
            return;
        }
        let Some(manager) = view
            .website_data_manager()
            .and_then(|data| data.cookie_manager())
        else {
            return;
        };
        let result = result.clone();
        manager.cookies(MUSIC_URL, None::<&gtk::gio::Cancellable>, move |cookies| {
            let Ok(mut cookies) = cookies else {
                return;
            };
            let pairs: Vec<(String, String)> = cookies
                .iter_mut()
                .filter_map(|cookie| {
                    Some((cookie.name()?.to_string(), cookie.value()?.to_string()))
                })
                .collect();
            match cookie_header(&pairs) {
                Some(header) => {
                    let mut out = std::io::stdout();
                    if writeln!(out, "{header}").and_then(|()| out.flush()).is_ok() {
                        result.set(SIGNED_IN);
                    }
                    gtk::main_quit();
                }
                // Google did not know the account any more and sent YouTube Music back signed
                // out: only the user can sign in.
                None if silent => gtk::main_quit(),
                // YouTube Music opened, but the user has not signed in yet.
                None => {}
            }
        });
    });
    if silent {
        gtk::glib::timeout_add_local_once(SILENT_TIME, gtk::main_quit);
    }

    view.load_uri(SIGN_IN_URL);
    window.show_all();
    gtk::main();
    code.get()
}

/// A web context whose data and cookies live in Encore's private profile folder, so the Google
/// sign-in outlasts the window.
fn kept_profile() -> Option<WebContext> {
    let dir = profile_dir();
    crate::platform::private_dir(&dir).ok()?;
    let data = dir.join("data");
    let cache = dir.join("cache");
    let manager = WebsiteDataManager::builder()
        .base_data_directory(data.to_str()?)
        .base_cache_directory(cache.to_str()?)
        .build();
    let cookies = manager.cookie_manager()?;
    cookies.set_persistent_storage(
        dir.join("cookies.sqlite").to_str()?,
        CookiePersistentStorage::Sqlite,
    );
    Some(WebContext::with_website_data_manager(&manager))
}
