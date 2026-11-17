//! The player's keyboard shortcuts: ↑ and ↓ turn the volume by the mouse wheel's step, ← and →
//! go back or forward five seconds, and Space plays or pauses. Keys a text field takes (the
//! arrows while typing a search) never reach them.

use iced::Task;
use iced::keyboard::{self, Key, key::Named};

use crate::{App, Message, volume};

/// How far ← and → jump.
const SEEK_SECONDS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    VolumeUp,
    VolumeDown,
    Back,
    Forward,
    PlayPause,
}

/// The shortcut of a key press with no modifier held, if it is one.
pub fn from_key(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    if !modifiers.is_empty() {
        return None;
    }
    let shortcut = match key {
        Key::Named(Named::ArrowUp) => Shortcut::VolumeUp,
        Key::Named(Named::ArrowDown) => Shortcut::VolumeDown,
        Key::Named(Named::ArrowLeft) => Shortcut::Back,
        Key::Named(Named::ArrowRight) => Shortcut::Forward,
        Key::Named(Named::Space) => Shortcut::PlayPause,
        _ => return None,
    };
    Some(Message::Shortcut(shortcut))
}

impl App {
    pub(crate) fn shortcut(&mut self, shortcut: Shortcut) -> Task<Message> {
        match shortcut {
            Shortcut::VolumeUp => self.step_volume(1),
            Shortcut::VolumeDown => self.step_volume(-1),
            Shortcut::Back => self.jump(-SEEK_SECONDS),
            Shortcut::Forward => self.jump(SEEK_SECONDS),
            Shortcut::PlayPause => return self.handle(Message::TogglePlay),
        }
        Task::none()
    }

    /// Turns the volume `notches` steps of the wheel's step, unmuting it.
    pub(crate) fn step_volume(&mut self, notches: i32) {
        if notches == 0 {
            return;
        }
        let from = self.audio_settings.effective_volume();
        let volume = volume::stepped(from, notches, self.audio_settings.volume_step);
        self.audio_settings.volume = volume;
        self.audio_settings.muted = false;
        self.online.set_volume(volume);
        self.audio_settings.save();
    }

    /// Goes `seconds` back or forward in the song, staying inside it.
    fn jump(&mut self, seconds: f32) {
        if !self.has_song() {
            return;
        }
        let at = self.position_ms() as f32 / 1000.0 + seconds;
        let end = match self.playback.duration {
            0 => f32::MAX,
            // Short of the very end, so the song does not end by the jump alone.
            duration => (duration as f32 - 1.0).max(0.0),
        };
        self.seek(at.clamp(0.0, end));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: keyboard::Modifiers) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }
    }

    #[test]
    fn the_arrows_and_space_are_shortcuts_without_modifiers() {
        let plain = keyboard::Modifiers::empty();
        assert!(matches!(
            from_key(press(Key::Named(Named::ArrowUp), plain)),
            Some(Message::Shortcut(Shortcut::VolumeUp))
        ));
        assert!(matches!(
            from_key(press(Key::Named(Named::ArrowLeft), plain)),
            Some(Message::Shortcut(Shortcut::Back))
        ));
        assert!(matches!(
            from_key(press(Key::Named(Named::Space), plain)),
            Some(Message::Shortcut(Shortcut::PlayPause))
        ));
        assert!(from_key(press(Key::Named(Named::ArrowUp), keyboard::Modifiers::CTRL)).is_none());
        assert!(from_key(press(Key::Character("a".into()), plain)).is_none());
    }

    #[test]
    fn the_shortcuts_turn_the_volume_and_jump_through_the_song() {
        let mut app = App::new();
        app.audio_settings.volume = 50.0;
        app.audio_settings.volume_step = 5;
        let _ = app.shortcut(Shortcut::VolumeUp);
        assert_eq!(app.audio_settings.volume, 55.0);
        let _ = app.shortcut(Shortcut::VolumeDown);
        let _ = app.shortcut(Shortcut::VolumeDown);
        assert_eq!(app.audio_settings.volume, 45.0);

        // Tests start on a sample song, which keeps its time on the clock.
        app.playback.elapsed = 20;
        app.playback.duration = 200;
        let _ = app.shortcut(Shortcut::Forward);
        assert_eq!(app.playback.elapsed, 25);
        app.playback.elapsed = 3;
        let _ = app.shortcut(Shortcut::Back);
        assert_eq!(app.playback.elapsed, 0, "never before the start");
        app.playback.elapsed = 197;
        let _ = app.shortcut(Shortcut::Forward);
        assert_eq!(app.playback.elapsed, 199, "never past the end");
    }
}
