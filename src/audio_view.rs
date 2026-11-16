//! The audio tab of the settings: the sound output, the volume wheel's step, download quality,
//! equalizer with presets, compressor, volume normalization, silence skipping, playback speed and crossfade. Opened with the equalizer
//! button in the player bar.

use iced::font::Weight;
use iced::widget::{
    Column, Row, button, column, container, row, slider, text, toggler, vertical_slider,
};
use iced::{Center, Element, Fill, Font, border};

use rust_i18n::t;

use crate::audio::Device;
use crate::audio_settings::{
    AudioChange, AudioSettings, BANDS, Compressor, MAX_CROSSFADE, MAX_GAIN, MAX_SPEED, MIN_SPEED,
    PRESETS,
};
use crate::download::Quality;
use crate::{Message, theme};

/// The tab's content: `devices` are the outputs to pick from, and `error` is what mpv said when
/// it refused the settings, if it did.
pub fn view<'a>(
    settings: &'a AudioSettings,
    devices: &'a [Device],
    error: Option<&'a str>,
) -> Element<'a, Message> {
    column![]
        .push(error.map(|error| text(error).color(theme::accent())))
        .push(output(settings, devices))
        .push(volume_step(settings))
        .push(quality(settings))
        .push(equalizer(settings))
        .push(compressor(settings))
        .push(normalization(settings))
        .push(silence(settings))
        .push(speed(settings))
        .push(crossfade(settings))
        .spacing(26)
        .into()
}

fn equalizer(settings: &AudioSettings) -> Element<'_, Message> {
    let switch = toggler(settings.equalizer)
        .label(t!("audio.equalizer"))
        .on_toggle(|on| change(AudioChange::Equalizer(on)))
        .style(theme::switch);

    let selected = settings.preset();
    let presets = Row::with_children(PRESETS.iter().enumerate().map(|(index, preset)| {
        chip(
            preset.name(),
            settings.equalizer && selected == Some(index),
            change(AudioChange::Preset(index)),
        )
    }))
    .spacing(8)
    .wrap();

    let bands = Row::with_children(BANDS.iter().zip(settings.gains).enumerate().map(
        |(band, (frequency, gain))| {
            let slider = vertical_slider(-MAX_GAIN..=MAX_GAIN, gain, move |gain| {
                change(AudioChange::Gain(band, gain))
            })
            .step(0.5_f32)
            .default(0.0_f32)
            .on_release(Message::AudioChangeDone)
            .height(160)
            .style(|_, status| {
                let mut style = theme::thin_slider(status, theme::accent());
                style.handle.shape = slider::HandleShape::Circle { radius: 7.0 };
                style
            });
            column![
                small(format!("{gain:+.1}")),
                slider,
                small(frequency_label(*frequency))
            ]
            .spacing(8)
            .align_x(Center)
            .width(52)
            .into()
        },
    ));

    let dimmed = if settings.equalizer {
        None
    } else {
        Some(t!("audio.equalizer_off_note"))
    };
    column![switch, presets, bands]
        .push(dimmed.map(note))
        .spacing(14)
        .into()
}

/// The system's default output and every device, the chosen one marked. A saved device that is
/// not connected stays listed, so it is clear why sound goes elsewhere.
fn output<'a>(settings: &'a AudioSettings, devices: &'a [Device]) -> Element<'a, Message> {
    let chosen = settings.output.as_deref();
    let missing = chosen.filter(|name| !devices.iter().any(|device| device.name == *name));
    let mut list = Column::new().push(choice(
        t!("audio.system_default").into_owned(),
        chosen.is_none() || missing.is_some(),
        Message::AudioOutput(None),
    ));
    for device in devices {
        list = list.push(choice(
            device.description.clone(),
            chosen == Some(device.name.as_str()),
            Message::AudioOutput(Some(device.name.clone())),
        ));
    }
    if let Some(name) = missing {
        list = list.push(choice(
            t!("audio.not_connected", name = name).into_owned(),
            false,
            Message::AudioOutput(Some(name.to_string())),
        ));
    }
    column![
        heading(t!("audio.sound_output"), 16.0),
        list.spacing(2).width(460)
    ]
    .spacing(10)
    .into()
}

/// A row of the output list, with a check mark when it is the one in use.
fn choice<'a>(label: String, selected: bool, message: Message) -> Element<'a, Message> {
    let mark = text(if selected { "\u{2713}" } else { "" })
        .size(14)
        .width(20)
        .color(theme::accent());
    button(row![mark, text(label).size(14)].align_y(Center).width(Fill))
        .on_press(message)
        .padding([7, 10])
        .width(Fill)
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            style.border = border::rounded(8);
            style
        })
        .into()
}

fn volume_step(settings: &AudioSettings) -> Element<'_, Message> {
    let choices = Row::with_children(crate::volume::STEPS.into_iter().map(|step| {
        chip(
            format!("{step}%"),
            settings.volume_step == step,
            change(AudioChange::VolumeStep(step)),
        )
    }))
    .spacing(8)
    .wrap();
    column![
        heading(t!("audio.volume_step"), 16.0),
        choices,
        note(t!("audio.volume_step_note")),
    ]
    .spacing(10)
    .into()
}

fn quality(settings: &AudioSettings) -> Element<'_, Message> {
    let choices = Row::with_children(Quality::ALL.into_iter().map(|quality| {
        chip(
            quality.label(),
            settings.quality == quality,
            change(AudioChange::Quality(quality)),
        )
    }))
    .spacing(8)
    .wrap();
    let explanation = match settings.quality {
        Quality::Low => t!("audio.quality_low_note"),
        Quality::Normal => t!("audio.quality_normal_note"),
        Quality::High => t!("audio.quality_high_note"),
    };
    column![
        heading(t!("audio.quality"), 16.0),
        choices,
        note(explanation),
        note(t!("audio.quality_next_song")),
    ]
    .spacing(10)
    .into()
}

fn compressor(settings: &AudioSettings) -> Element<'_, Message> {
    let strengths = Row::with_children(Compressor::ALL.into_iter().map(|strength| {
        chip(
            strength.label(),
            settings.compressor == strength,
            change(AudioChange::Compressor(strength)),
        )
    }))
    .spacing(8)
    .wrap();
    column![
        heading(t!("audio.compressor"), 16.0),
        strengths,
        note(t!("audio.compressor_note")),
    ]
    .spacing(10)
    .into()
}

fn silence(settings: &AudioSettings) -> Element<'_, Message> {
    column![
        toggler(settings.skip_silence)
            .label(t!("audio.skip_silence"))
            .on_toggle(|on| change(AudioChange::SkipSilence(on)))
            .style(theme::switch),
        note(t!("audio.skip_silence_note")),
    ]
    .spacing(8)
    .into()
}

fn normalization(settings: &AudioSettings) -> Element<'_, Message> {
    column![
        toggler(settings.normalize)
            .label(t!("audio.normalize"))
            .on_toggle(|on| change(AudioChange::Normalize(on)))
            .style(theme::switch),
        note(t!("audio.normalize_note")),
    ]
    .spacing(8)
    .into()
}

fn speed(settings: &AudioSettings) -> Element<'_, Message> {
    let control = slider(MIN_SPEED..=MAX_SPEED, settings.speed, |speed| {
        change(AudioChange::Speed(speed))
    })
    .step(0.05_f32)
    .default(1.0_f32)
    .on_release(Message::AudioChangeDone)
    .width(320)
    .height(18)
    .style(|_, status| theme::thin_slider(status, theme::accent()));

    column![
        heading(t!("audio.speed"), 16.0),
        row![control, text(format!("{:.2}\u{d7}", settings.speed))]
            .spacing(14)
            .align_y(Center),
        toggler(settings.keep_pitch)
            .label(t!("audio.keep_pitch"))
            .on_toggle(|on| change(AudioChange::KeepPitch(on)))
            .style(theme::switch),
    ]
    .spacing(10)
    .into()
}

fn crossfade(settings: &AudioSettings) -> Element<'_, Message> {
    let control = slider(
        0.0..=MAX_CROSSFADE as f32,
        settings.crossfade as f32,
        |seconds| change(AudioChange::Crossfade(seconds)),
    )
    .step(1.0_f32)
    .on_release(Message::AudioChangeDone)
    .width(320)
    .height(18)
    .style(|_, status| theme::thin_slider(status, theme::accent()));
    let label = match settings.crossfade {
        0 => t!("audio.crossfade_off"),
        seconds => t!("audio.crossfade_seconds", seconds = seconds),
    };

    column![
        heading(t!("audio.crossfade"), 16.0),
        row![control, text(label)].spacing(14).align_y(Center),
        note(t!("audio.crossfade_note")),
    ]
    .spacing(10)
    .into()
}

fn change(change: AudioChange) -> Message {
    Message::AudioChanged(change)
}

/// "31", "500", "1k", "16k".
fn frequency_label(hertz: u32) -> String {
    if hertz >= 1000 {
        format!("{}k", hertz / 1000)
    } else {
        hertz.to_string()
    }
}

fn chip<'a>(
    label: impl text::IntoFragment<'a>,
    selected: bool,
    message: Message,
) -> Element<'a, Message> {
    button(text(label).size(13))
        .on_press(message)
        .padding([6, 14])
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            if selected {
                style.background = Some(theme::accent().into());
                style.text_color = theme::text_primary();
            }
            style.border = border::rounded(16).color(theme::line()).width(1);
            style
        })
        .into()
}

fn heading<'a>(label: impl text::IntoFragment<'a>, size: f32) -> Element<'a, Message> {
    text(label)
        .size(size)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

fn small<'a>(label: String) -> Element<'a, Message> {
    text(label).size(11).color(theme::text_secondary()).into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    container(text(message).size(13).color(theme::text_secondary())).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_frequencies_like_an_equalizer() {
        assert_eq!(frequency_label(31), "31");
        assert_eq!(frequency_label(1000), "1k");
        assert_eq!(frequency_label(16000), "16k");
    }
}
