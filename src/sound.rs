//! Built-in sounds, sounds from files, and bounded playback.
//!
//! The alarm and chime are synthesised as mono `f32` samples, so they are deterministic and need
//! no audio device to build. Only [`play`] touches the output device.

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::time::Duration;

/// Sample rate of the built-in sounds. rodio 0.16's `SineWave` is fixed at 48 kHz too.
pub const SAMPLE_RATE: u32 = 48_000;

/// Longest a reminder may ring, whatever was asked for.
pub const MAX_RING: Duration = Duration::from_mins(10);

/// How long a reminder rings when no `--ring` is given.
pub const DEFAULT_RING: Duration = Duration::from_secs(30);

/// A sound file given to `ding` is cut off after this long.
pub const DING_FILE_MAX: Duration = Duration::from_secs(5);

/// Peak level of the built-in sounds (full scale is 1.0).
const AMPLITUDE: f32 = 0.3;

/// Fade at each edge of a tone, so beeps do not click.
const FADE_MS: u32 = 10;

const ALARM_HZ: f32 = 880.0;
const ALARM_BEEP_MS: u32 = 150;
const ALARM_GAP_MS: u32 = 100;
const ALARM_BEEPS: usize = 3;
const ALARM_PAUSE_MS: u32 = 600;

const CHIME_LOW_HZ: f32 = 660.0;
const CHIME_LOW_MS: u32 = 180;
const CHIME_HIGH_HZ: f32 = 990.0;
const CHIME_HIGH_MS: u32 = 300;

/// What to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sound {
    /// Three beeps then a pause, about 1.35 s per cycle; meant to be looped.
    Alarm,
    /// A short two-note chime.
    Chime,
    /// An audio file chosen by the user.
    File(PathBuf),
}

/// How long and how often to play a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    /// Play once, cut off after `max`.
    Once { max: Duration },
    /// Repeat until `for_` has passed.
    Loop { for_: Duration },
}

#[derive(Debug, thiserror::Error)]
pub enum SoundError {
    #[error("cannot open sound file {}: {source}", .path.display())]
    Open { path: PathBuf, source: io::Error },
    #[error("cannot decode sound file {}: {source}", .path.display())]
    Decode {
        path: PathBuf,
        source: rodio::decoder::DecoderError,
    },
    #[error("no audio output available: {0}")]
    Output(#[from] rodio::StreamError),
    #[error("cannot start playback: {0}")]
    Playback(#[from] rodio::PlayError),
}

/// The ring time to use for a requested one: never more than [`MAX_RING`].
#[must_use]
pub fn ring_for(requested: Duration) -> Duration {
    requested.min(MAX_RING)
}

/// Number of samples in `ms` milliseconds.
fn samples_in(ms: u32) -> usize {
    // At most a few hundred ms, so the product fits; `usize` is at least 32 bits.
    usize::try_from(ms * SAMPLE_RATE / 1_000).unwrap_or_default()
}

fn silence(ms: u32) -> Vec<f32> {
    vec![0.0; samples_in(ms)]
}

/// A sine of `hz` lasting `ms`, at [`AMPLITUDE`], with [`FADE_MS`] fades at both edges.
#[allow(clippy::cast_precision_loss)] // sample indices are far below 2^24
fn tone(hz: f32, ms: u32) -> Vec<f32> {
    let len = samples_in(ms);
    let fade = samples_in(FADE_MS) as f32;
    let step = std::f32::consts::TAU * hz / SAMPLE_RATE as f32;

    (0..len)
        .map(|i| {
            let from_start = (i as f32 / fade).min(1.0);
            let from_end = ((len - 1 - i) as f32 / fade).min(1.0);
            AMPLITUDE * from_start.min(from_end) * (step * i as f32).sin()
        })
        .collect()
}

fn alarm_samples() -> Vec<f32> {
    let mut samples = Vec::new();
    for _ in 0..ALARM_BEEPS {
        samples.extend(tone(ALARM_HZ, ALARM_BEEP_MS));
        samples.extend(silence(ALARM_GAP_MS));
    }
    samples.extend(silence(ALARM_PAUSE_MS));
    samples
}

fn chime_samples() -> Vec<f32> {
    let mut samples = tone(CHIME_LOW_HZ, CHIME_LOW_MS);
    samples.extend(tone(CHIME_HIGH_HZ, CHIME_HIGH_MS));
    samples
}

/// Opens `sound` as a source of `f32` samples.
///
/// A file is opened and its header decoded here, so a bad path or format fails now rather than
/// when the sound is needed.
///
/// # Errors
/// Returns [`SoundError::Open`] if the file cannot be opened and [`SoundError::Decode`] if it is
/// not audio the decoder understands.
pub fn source(sound: &Sound) -> Result<Box<dyn Source<Item = f32> + Send>, SoundError> {
    match sound {
        Sound::Alarm => Ok(Box::new(SamplesBuffer::new(
            1,
            SAMPLE_RATE,
            alarm_samples(),
        ))),
        Sound::Chime => Ok(Box::new(SamplesBuffer::new(
            1,
            SAMPLE_RATE,
            chime_samples(),
        ))),
        Sound::File(path) => {
            let file = File::open(path).map_err(|source| SoundError::Open {
                path: path.clone(),
                source,
            })?;
            let decoder =
                Decoder::new(BufReader::new(file)).map_err(|source| SoundError::Decode {
                    path: path.clone(),
                    source,
                })?;
            Ok(Box::new(decoder.convert_samples()))
        }
    }
}

/// Checks that `sound` can be played, without playing it or needing an audio device.
///
/// # Errors
/// Fails exactly when [`source`] does.
pub fn check(sound: &Sound) -> Result<(), SoundError> {
    source(sound).map(drop)
}

/// Limits `source` as `playback` says.
fn bound(
    source: Box<dyn Source<Item = f32> + Send>,
    playback: Playback,
) -> Box<dyn Source<Item = f32> + Send> {
    match playback {
        Playback::Once { max } => Box::new(source.take_duration(max)),
        Playback::Loop { for_ } => Box::new(source.repeat_infinite().take_duration(for_)),
    }
}

/// Plays `sound` as `playback` says and blocks until it has finished.
///
/// The sound is checked before the output device is opened, so a bad file fails without any
/// device.
///
/// # Errors
/// Returns the errors of [`source`], [`SoundError::Output`] if there is no audio output device,
/// and [`SoundError::Playback`] if playback cannot start.
pub fn play(sound: &Sound, playback: Playback) -> Result<(), SoundError> {
    let source = bound(source(sound)?, playback);
    let (_stream, handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&handle)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::cast_precision_loss)] // lengths here are far below 2^23
    fn alarm_is_a_few_seconds_and_chime_is_short() {
        let alarm: Vec<f32> = source(&Sound::Alarm).unwrap().collect();
        let chime: Vec<f32> = source(&Sound::Chime).unwrap().collect();
        let secs = |n: usize| n as f32 / (SAMPLE_RATE as f32);
        assert!((1.0..=4.0).contains(&secs(alarm.len())));
        assert!((0.2..=1.5).contains(&secs(chime.len())));
        assert!(alarm.iter().chain(&chime).all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn missing_file_fails_at_check_time() {
        assert!(check(&Sound::File("definitely/not/here.wav".into())).is_err());
    }

    #[test]
    fn ring_time_is_capped() {
        assert_eq!(ring_for(Duration::from_hours(1)), MAX_RING);
        assert_eq!(ring_for(Duration::from_secs(5)), Duration::from_secs(5));
    }

    /// Samples in `ms` milliseconds at the built-in rate.
    fn n(ms: usize) -> usize {
        ms * SAMPLE_RATE as usize / 1000
    }

    fn crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    fn collect(sound: &Sound) -> Vec<f32> {
        source(sound).unwrap().collect()
    }

    /// A minimal 16-bit mono PCM WAV of a rising ramp, in a fresh temp file.
    fn write_wav(name: &str, samples: u32) -> PathBuf {
        let rate: u32 = 8_000;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes()); // PCM
        bytes.extend(1_u16.to_le_bytes()); // mono
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * 2).to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        for i in 0..samples {
            bytes.extend(i16::try_from(i % 1000 * 10).unwrap().to_le_bytes());
        }
        let path = std::env::temp_dir().join(format!("cli-timer-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn alarm_is_three_beeps_then_a_pause_in_one_35_second_cycle() {
        let alarm = collect(&Sound::Alarm);
        assert_eq!(alarm.len(), n(1350));
        for beep in 0..3 {
            let start = n(beep * 250);
            let body = &alarm[start..start + n(150)];
            // 880 Hz for 150 ms is 132 cycles, so about 264 sign changes.
            assert!(
                (255..=270).contains(&crossings(body)),
                "beep {beep}: {}",
                crossings(body)
            );
            assert!(body.iter().any(|s| s.abs() > 0.29), "beep {beep} too quiet");
            assert!(
                alarm[start + n(150)..start + n(250)]
                    .iter()
                    .all(|s| *s == 0.0),
                "gap after beep {beep} not silent"
            );
        }
        assert!(alarm[n(750)..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn chime_is_a_low_note_then_a_higher_one() {
        let chime = collect(&Sound::Chime);
        assert_eq!(chime.len(), n(480));
        let low = crossings(&chime[..n(180)]); // 660 Hz: about 238
        let high = crossings(&chime[n(180)..]); // 990 Hz: about 594
        assert!((230..=245).contains(&low), "low note: {low}");
        assert!((585..=600).contains(&high), "high note: {high}");
        assert!(chime.iter().any(|s| s.abs() > 0.29));
    }

    #[test]
    fn tones_fade_in_and_out_instead_of_clicking() {
        let chime = collect(&Sound::Chime);
        assert!(chime[0].abs() < 0.001);
        assert!(chime[chime.len() - 1].abs() < 0.001);
        // the join between the two notes is faded as well
        assert!(chime[n(180) - 1].abs() < 0.001);
        assert!(chime[n(180)].abs() < 0.001);
    }

    #[test]
    fn built_in_sources_are_mono_at_the_stated_rate() {
        for sound in [Sound::Alarm, Sound::Chime] {
            let src = source(&sound).unwrap();
            assert_eq!(src.channels(), 1);
            assert_eq!(src.sample_rate(), SAMPLE_RATE);
        }
    }

    #[test]
    fn built_in_sounds_pass_check() {
        assert!(check(&Sound::Alarm).is_ok());
        assert!(check(&Sound::Chime).is_ok());
    }

    #[test]
    fn a_valid_wav_file_passes_check_and_decodes() {
        let path = write_wav("valid.wav", 800);
        assert!(check(&Sound::File(path.clone())).is_ok());
        let src = source(&Sound::File(path.clone())).unwrap();
        assert_eq!(src.sample_rate(), 8_000);
        assert_eq!(src.channels(), 1);
        assert_eq!(src.count(), 800);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_missing_file_is_an_open_error_naming_the_path() {
        let err = check(&Sound::File("definitely/not/here.wav".into())).unwrap_err();
        assert!(matches!(err, SoundError::Open { .. }), "{err:?}");
        assert!(err.to_string().contains("definitely/not/here.wav"));
    }

    #[test]
    fn a_file_that_is_not_audio_is_a_decode_error() {
        let path = std::env::temp_dir().join(format!("cli-timer-{}-notes.txt", std::process::id()));
        std::fs::write(&path, "this is plainly not audio, just some text\n").unwrap();
        let err = check(&Sound::File(path.clone())).unwrap_err();
        assert!(matches!(err, SoundError::Decode { .. }), "{err:?}");
        assert!(source(&Sound::File(path.clone())).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn play_rejects_a_bad_file_before_touching_the_audio_device() {
        // Must not open the output device: a bad file is reported as such, not as "no audio".
        let missing = Sound::File("definitely/not/here.wav".into());
        let err = play(&missing, Playback::Once { max: MAX_RING }).unwrap_err();
        assert!(matches!(err, SoundError::Open { .. }), "{err:?}");
    }

    #[test]
    fn once_stops_at_max_or_at_the_end_of_the_sound() {
        let cut = bound(
            source(&Sound::Alarm).unwrap(),
            Playback::Once {
                max: Duration::from_millis(500),
            },
        );
        assert_eq!(cut.count(), n(500));

        let whole = bound(
            source(&Sound::Alarm).unwrap(),
            Playback::Once {
                max: Duration::from_secs(10),
            },
        );
        assert_eq!(whole.count(), n(1350));
    }

    #[test]
    fn loop_repeats_the_cycle_for_the_requested_time_then_stops() {
        let looped: Vec<f32> = bound(
            source(&Sound::Alarm).unwrap(),
            Playback::Loop {
                for_: Duration::from_secs(3),
            },
        )
        .collect();
        // rodio rounds the cut-off to a frame or two, so allow a few samples either way.
        assert!(looped.len().abs_diff(n(3000)) <= 8, "{}", looped.len());
        let cycle = n(1350);
        let alarm = collect(&Sound::Alarm);
        assert_eq!(looped[..cycle], alarm[..]);
        assert_eq!(looped[cycle..2 * cycle], alarm[..]);
    }

    #[test]
    fn ring_for_leaves_the_cap_and_zero_alone_and_caps_just_above() {
        assert_eq!(ring_for(MAX_RING), MAX_RING);
        assert_eq!(ring_for(MAX_RING + Duration::from_millis(1)), MAX_RING);
        let just_under = MAX_RING.checked_sub(Duration::from_millis(1)).unwrap();
        assert_eq!(ring_for(just_under), just_under);
        assert_eq!(ring_for(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn constants_have_the_agreed_values() {
        assert_eq!(MAX_RING, Duration::from_mins(10));
        assert_eq!(DEFAULT_RING, Duration::from_secs(30));
        assert_eq!(DING_FILE_MAX, Duration::from_secs(5));
        assert_eq!(SAMPLE_RATE, 48_000);
        assert!(DEFAULT_RING <= MAX_RING);
    }
}
