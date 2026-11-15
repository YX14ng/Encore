//! The audio tab of the settings: the sound output, download quality, equalizer with presets,
//! compressor, volume
//! normalization, silence skipping, playback speed and crossfade. Opened with the equalizer
//! button in the player bar.

use iced::font::Weight;
use iced::widget::{
    Column, Row, button, column, container, row, slider, text, toggler, vertical_slider,
};
use iced::{Center, Element, Fill, Font, border};

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
        .label("Equalizer")
        .on_toggle(|on| change(AudioChange::Equalizer(on)))
        .style(theme::switch);

    let selected = settings.preset();
    let presets = Row::with_children(PRESETS.iter().enumerate().map(|(index, preset)| {
        chip(
            preset.name,
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
        Some("Switch the equalizer on to hear these bands.")
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
        "System default".to_string(),
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
            format!("{name} (not connected)"),
            false,
            Message::AudioOutput(Some(name.to_string())),
        ));
    }
    column![heading("Sound output", 16.0), list.spacing(2).width(460)]
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
        Quality::Low => "Opus around 60 kbit/s, for slow or metered connections.",
        Quality::Normal => "Opus around 130 kbit/s, the best YouTube gives without Premium.",
        Quality::High => {
            "Opus around 256 kbit/s with YouTube Music Premium (signed in); without Premium it \
             is the same as Normal. yt-dlp downloads with your session, which YouTube may \
             notice: yt-dlp warns that accounts used this way can get blocked for a while."
        }
    };
    column![
        heading("Audio quality", 16.0),
        choices,
        note(explanation),
        note("Takes effect from the next song that downloads."),
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
        heading("Compressor", 16.0),
        strengths,
        note("Evens out the loud and quiet parts inside a song."),
    ]
    .spacing(10)
    .into()
}

fn silence(settings: &AudioSettings) -> Element<'_, Message> {
    column![
        toggler(settings.skip_silence)
            .label("Skip silence")
            .on_toggle(|on| change(AudioChange::SkipSilence(on)))
            .style(theme::switch),
        note(
            "Starts songs where the music starts and ends them where it ends. Silences inside \
             a song stay. Takes effect from the next song.",
        ),
    ]
    .spacing(8)
    .into()
}

fn normalization(settings: &AudioSettings) -> Element<'_, Message> {
    column![
        toggler(settings.normalize)
            .label("Normalize volume")
            .on_toggle(|on| change(AudioChange::Normalize(on)))
            .style(theme::switch),
        note("Plays every song at about the same loudness, so quiet and loud tracks match."),
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
        heading("Playback speed", 16.0),
        row![control, text(format!("{:.2}\u{d7}", settings.speed))]
            .spacing(14)
            .align_y(Center),
        toggler(settings.keep_pitch)
            .label("Keep the pitch when changing speed")
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
        0 => "Off".to_string(),
        seconds => format!("{seconds} s"),
    };

    column![
        heading("Crossfade", 16.0),
        row![control, text(label)].spacing(14).align_y(Center),
        note("Fades the end of each song into the start of the next one."),
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

fn chip(label: &str, selected: bool, message: Message) -> Element<'_, Message> {
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

fn heading(label: &str, size: f32) -> Element<'_, Message> {
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

fn note(message: &str) -> Element<'_, Message> {
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
