//! The sign-in window on Linux: a WebKitGTK view with its cookies in memory only.

use std::cell::Cell;
use std::io::{IsTerminal, Write};
use std::rc::Rc;

use gtk::prelude::*;
use rust_i18n::t;
use webkit2gtk::{CookieManagerExt, LoadEvent, WebContext, WebViewExt, WebsiteDataManagerExt};

use super::{LANGUAGE_VAR, MUSIC_URL, SIGN_IN_URL, cookie_header};

/// Shows the sign-in window. Returns the process's exit code: 0 once signed in (the cookies
/// went to standard output), 1 if the user closed the window, 2 if it could not start.
pub fn run() -> i32 {
    // It prints the session cookies, so it only runs for the Encore that reads them.
    if std::io::stdout().is_terminal() || gtk::init().is_err() {
        return 2;
    }
    if let Ok(language) = std::env::var(LANGUAGE_VAR) {
        rust_i18n::set_locale(&language);
    }
    let signed_in = Rc::new(Cell::new(false));
    let context = WebContext::new_ephemeral();
    let view = webkit2gtk::WebView::with_context(&context);
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title(&t!("sign_in_window.title"));
    window.set_default_size(480, 720);
    window.add(&view);
    window.connect_destroy(|_| gtk::main_quit());

    let done = signed_in.clone();
    view.connect_load_changed(move |view, event| {
        let on_music = view.uri().is_some_and(|uri| uri.starts_with(MUSIC_URL));
        if event != LoadEvent::Finished || !on_music || done.get() {
            return;
        }
        let Some(manager) = view
            .website_data_manager()
            .and_then(|data| data.cookie_manager())
        else {
            return;
        };
        let done = done.clone();
        manager.cookies(MUSIC_URL, None::<&gtk::gio::Cancellable>, move |result| {
            let Ok(mut cookies) = result else {
                return;
            };
            let pairs: Vec<(String, String)> = cookies
                .iter_mut()
                .filter_map(|cookie| {
                    Some((cookie.name()?.to_string(), cookie.value()?.to_string()))
                })
                .collect();
            let Some(header) = cookie_header(&pairs) else {
                // YouTube Music opened, but not signed in yet.
                return;
            };
            let mut out = std::io::stdout();
            if writeln!(out, "{header}").and_then(|()| out.flush()).is_ok() {
                done.set(true);
            }
            gtk::main_quit();
        });
    });

    view.load_uri(SIGN_IN_URL);
    window.show_all();
    gtk::main();
    if signed_in.get() { 0 } else { 1 }
}
