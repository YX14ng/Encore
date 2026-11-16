//! The volume bar of the player: dragging it, turning the mouse wheel over it (by the step
//! picked on the audio screen), and its percentage, shown while the pointer is over it.

use iced::mouse::ScrollDelta;
use iced::widget::{container, mouse_area, row, slider, text, tooltip};
use iced::{Center, Element, border};

use crate::player::Controls;
use crate::{Message, theme};

/// The steps the wheel can move the volume by, in percent.
pub const STEPS: [u32; 4] = [1, 2, 5, 10];
/// The step until another is picked.
pub const DEFAULT_STEP: u32 = 5;
/// How many pixels of a touchpad scroll make one notch of a wheel.
const PIXELS_PER_NOTCH: f32 = 40.0;

/// The whole notches of a scroll, keeping the rest in `pending` for the next one: a wheel
/// sends lines, a touchpad many small pixel moves.
pub fn notches(pending: &mut f32, delta: ScrollDelta) -> i32 {
    *pending += match delta {
        ScrollDelta::Lines { y, .. } => y,
        ScrollDelta::Pixels { y, .. } => y / PIXELS_PER_NOTCH,
    };
    let whole = pending.trunc();
    *pending -= whole;
    whole as i32
}

/// The volume `notches` steps of `step` percent away from `volume`, landing on a multiple of
/// the step (83 goes up to 85 and down to 80 by 5).
pub fn stepped(volume: f32, notches: i32, step: u32) -> f32 {
    let step = step.max(1) as f32;
    let from = if notches > 0 {
        (volume / step).floor()
    } else {
        (volume / step).ceil()
    };
    ((from + notches as f32) * step).clamp(0.0, 100.0)
}

/// The mute button and the bar, which the wheel turns too.
pub fn bar(mute: Element<'static, Message>, controls: Controls) -> Element<'static, Message> {
    let volume = if controls.muted { 0.0 } else { controls.volume };
    let slider = slider(0.0..=100.0, volume, Message::VolumeChanged)
        .on_release(Message::VolumeDone)
        .width(90)
        .height(14)
        .style(|_, status| theme::thin_slider(status, theme::text_secondary()));
    let percent = container(text(format!("{}%", volume.round() as u32)).size(12))
        .padding([3, 8])
        .style(|_| theme::panel(theme::hero()).border(border::rounded(6)));
    let slider = tooltip(slider, percent, tooltip::Position::Top);
    mouse_area(row![mute, slider].align_y(Center))
        .on_scroll(Message::VolumeScrolled)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_lands_on_steps() {
        assert_eq!(stepped(83.0, 1, 5), 85.0);
        assert_eq!(stepped(83.0, -1, 5), 80.0);
        assert_eq!(stepped(85.0, 1, 5), 90.0);
        assert_eq!(stepped(85.0, -2, 5), 75.0);
        assert_eq!(stepped(42.0, 1, 1), 43.0);
        assert_eq!(stepped(98.0, 3, 10), 100.0);
        assert_eq!(stepped(4.0, -1, 10), 0.0);
    }

    #[test]
    fn touchpads_add_up_to_notches() {
        let mut pending = 0.0;
        let small = ScrollDelta::Pixels { x: 0.0, y: 15.0 };
        assert_eq!(notches(&mut pending, small), 0);
        assert_eq!(notches(&mut pending, small), 0);
        assert_eq!(notches(&mut pending, small), 1, "45 pixels make a notch");
        let wheel = ScrollDelta::Lines { x: 0.0, y: -1.0 };
        assert_eq!(
            notches(&mut pending, wheel),
            0,
            "the leftover cancels part of it"
        );
        assert_eq!(notches(&mut pending, wheel), -1);
    }
}
