//! The sign-in window on Windows: a WebView2 view (the Edge engine Windows ships) whose profile
//! (the Google sign-in) is kept in Encore's private data. With `silent` the window stays hidden.

use std::io::{IsTerminal, Write};
use std::time::Instant;

use rust_i18n::t;
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use wry::{PageLoadEvent, WebContext, WebViewBuilder};

use super::{
    CLOSED, LANGUAGE_VAR, MUSIC_URL, NEEDS_YOU, SIGN_IN_URL, SIGNED_IN, SILENT_TIME, cookie_header,
    profile_dir, wants_the_user,
};

/// The kind of page that finished loading.
#[derive(Debug, Clone, Copy)]
enum Loaded {
    Music,
    SignInForm,
}

/// Shows the sign-in window, or with `silent` signs in with it hidden. Returns the process's
/// exit code: signed in (the cookies went to standard output), closed by the user, could not
/// start (2), or, silent, Google wants the user.
pub fn show(silent: bool) -> i32 {
    // It prints the session cookies, so it only runs for the Encore that reads them.
    if std::io::stdout().is_terminal() {
        return 2;
    }
    if let Ok(language) = std::env::var(LANGUAGE_VAR) {
        rust_i18n::set_locale(&language);
    }
    let dir = profile_dir();
    if crate::platform::private_dir(&dir).is_err() {
        return 2;
    }
    // A page finished loading: YouTube Music (look for the session) or a Google sign-in form.
    let mut event_loop = EventLoopBuilder::<Loaded>::with_user_event().build();
    let loaded = event_loop.create_proxy();
    let Ok(window) = WindowBuilder::new()
        .with_title(t!("sign_in_window.title"))
        .with_inner_size(LogicalSize::new(480.0, 720.0))
        .with_visible(!silent)
        .build(&event_loop)
    else {
        return 2;
    };
    let mut context = WebContext::new(Some(dir));
    let Ok(view) = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(SIGN_IN_URL)
        .with_on_page_load_handler(move |event, url| {
            if !matches!(event, PageLoadEvent::Finished) {
                return;
            }
            if url.starts_with(MUSIC_URL) {
                let _ = loaded.send_event(Loaded::Music);
            } else if wants_the_user(&url) {
                let _ = loaded.send_event(Loaded::SignInForm);
            }
        })
        .build(&window)
    else {
        return 2;
    };

    // Until it signs in: closed (the window) or the user needed (silent).
    let mut code = if silent { NEEDS_YOU } else { CLOSED };
    let deadline = Instant::now() + SILENT_TIME;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = if silent {
            ControlFlow::WaitUntil(deadline)
        } else {
            ControlFlow::Wait
        };
        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            // With no window, nobody can fill the form in.
            Event::UserEvent(Loaded::SignInForm) if silent => *control_flow = ControlFlow::Exit,
            Event::UserEvent(Loaded::SignInForm) => {}
            Event::UserEvent(Loaded::Music) => {
                let Ok(cookies) = view.cookies_for_url(MUSIC_URL) else {
                    return;
                };
                let pairs: Vec<(String, String)> = cookies
                    .iter()
                    .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
                    .collect();
                match cookie_header(&pairs) {
                    Some(header) => {
                        let mut out = std::io::stdout();
                        if writeln!(out, "{header}").and_then(|()| out.flush()).is_ok() {
                            code = SIGNED_IN;
                        }
                        *control_flow = ControlFlow::Exit;
                    }
                    // Google did not know the account any more: only the user can sign in.
                    None if silent => *control_flow = ControlFlow::Exit,
                    // YouTube Music opened, but the user has not signed in yet.
                    None => {}
                }
            }
            _ => {}
        }
    });
    code
}
