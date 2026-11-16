//! The sign-in window on Windows: a WebView2 view (the Edge engine Windows ships) in a private
//! (InPrivate) session, so its cookies stay in memory only.

use std::io::{IsTerminal, Write};

use rust_i18n::t;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use wry::{PageLoadEvent, WebContext, WebViewBuilder};

use super::{LANGUAGE_VAR, MUSIC_URL, SIGN_IN_URL, cookie_header};

/// Shows the sign-in window. Returns the process's exit code: 0 once signed in (the cookies
/// went to standard output), 1 if the user closed the window, 2 if it could not start.
pub fn run() -> i32 {
    // It prints the session cookies, so it only runs for the Encore that reads them.
    if std::io::stdout().is_terminal() {
        return 2;
    }
    if let Ok(language) = std::env::var(LANGUAGE_VAR) {
        rust_i18n::set_locale(&language);
    }
    // A page of YouTube Music finished loading: time to look for the session.
    let mut event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let loaded = event_loop.create_proxy();
    let Ok(window) = WindowBuilder::new()
        .with_title(t!("sign_in_window.title"))
        .with_inner_size(LogicalSize::new(480.0, 720.0))
        .build(&event_loop)
    else {
        return 2;
    };
    // WebView2 keeps its working files in a folder of its own; next to encore.exe it may not
    // be allowed to write.
    let mut context = WebContext::new(Some(crate::platform::cache_dir().join("webview2")));
    let Ok(view) = WebViewBuilder::new_with_web_context(&mut context)
        .with_incognito(true)
        .with_url(SIGN_IN_URL)
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) && url.starts_with(MUSIC_URL) {
                let _ = loaded.send_event(());
            }
        })
        .build(&window)
    else {
        return 2;
    };

    let mut code = 1;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(()) => {
                let Ok(cookies) = view.cookies_for_url(MUSIC_URL) else {
                    return;
                };
                let pairs: Vec<(String, String)> = cookies
                    .iter()
                    .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
                    .collect();
                // YouTube Music opened, but not signed in yet.
                let Some(header) = cookie_header(&pairs) else {
                    return;
                };
                let mut out = std::io::stdout();
                if writeln!(out, "{header}").and_then(|()| out.flush()).is_ok() {
                    code = 0;
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
    code
}
