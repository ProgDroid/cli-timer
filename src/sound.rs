//! Built-in sounds, sounds from files, and bounded playback.
//!
//! The alarm and chime are synthesised as mono `f32` samples, so they are deterministic and need
//! no audio device to build. Only [`open_output`] opens the output device.

use rodio::buffer::SamplesBuffer;
use rodio::decoder::{DecoderBuilder, DecoderError};
use rodio::source::SeekError;
use rodio::{
    ChannelCount, Decoder, DeviceSinkError, MixerDeviceSink, Player, Sample, SampleRate, Source,
};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Sample rate of the built-in sounds, in Hz.
pub const SAMPLE_RATE: u32 = 48_000;

/// [`SAMPLE_RATE`] as rodio takes it.
const RATE: SampleRate = SampleRate::new(SAMPLE_RATE).unwrap();

/// The built-in sounds are mono.
const MONO: ChannelCount = rodio::nz!(1);

/// Longest a reminder may ring, whatever was asked for.
pub const MAX_RING: Duration = Duration::from_mins(10);

/// How long a reminder rings when no `--ring` is given.
pub const DEFAULT_RING: Duration = Duration::from_secs(30);

/// A sound file given to `ding` is cut off after this long.
pub const DING_FILE_MAX: Duration = Duration::from_secs(5);

/// The full-screen alarm fades in over this long.
const ALARM_FADE_IN: Duration = Duration::from_millis(500);

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
    #[error("cannot decode sound file {}: {}", .path.display(), decode_detail(.source))]
    Decode { path: PathBuf, source: DecoderError },
    #[error("no audio output available: {0}")]
    Output(#[from] DeviceSinkError),
}

/// What a [`SoundError::Decode`] says after the path.
///
/// Every variant keeps the wording `-s` has always shown (rodio 0.16's), on stderr and in
/// `--json`'s `error`, rather than rodio's own, which changes between releases: "Unrecognized
/// format" for an unrecognised file, and the cause itself (such as "end of stream" for a cut-off
/// file) where rodio 0.23 prints a fixed sentence without it.
fn decode_detail(error: &DecoderError) -> String {
    match error {
        DecoderError::UnrecognizedFormat => "Unrecognized format".to_owned(),
        DecoderError::IoError(msg) => msg.clone(),
        DecoderError::DecodeError(msg) | DecoderError::LimitError(msg) => (*msg).to_owned(),
        DecoderError::ResetRequired => "Reset required".to_owned(),
        DecoderError::NoStreams => "No streams".to_owned(),
    }
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

/// Opens `sound` as a source of samples.
///
/// A file is opened and its header decoded here, so a bad path or format fails now rather than
/// when the sound is needed.
///
/// # Errors
/// Returns [`SoundError::Open`] if the file cannot be opened and [`SoundError::Decode`] if it is
/// not audio the decoder understands.
pub fn source(sound: &Sound) -> Result<Box<dyn Source + Send>, SoundError> {
    match sound {
        Sound::Alarm => Ok(Box::new(SamplesBuffer::new(MONO, RATE, alarm_samples()))),
        Sound::Chime => Ok(Box::new(SamplesBuffer::new(MONO, RATE, chime_samples()))),
        Sound::File(path) => {
            let decoder = file_decoder(path)?
                .build()
                .map_err(|source| decode_error(path, source))?;
            Ok(Box::new(decoder))
        }
    }
}

/// Opens the file at `path` for decoding, set up as rodio's `Decoder::try_from(File)` sets it up
/// (`decoder/mod.rs`): buffered, with its length, and seekable.
fn file_decoder(path: &Path) -> Result<DecoderBuilder<BufReader<File>>, SoundError> {
    let file = File::open(path).map_err(|source| SoundError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    // `Decoder::try_from(File)` reports a failed metadata read as a decode error; so does this.
    let len = file
        .metadata()
        .map_err(|e| decode_error(path, DecoderError::IoError(e.to_string())))?
        .len();
    Ok(Decoder::builder()
        .with_data(BufReader::new(file))
        .with_byte_len(len)
        .with_seekable(true))
}

fn decode_error(path: &Path, source: DecoderError) -> SoundError {
    SoundError::Decode {
        path: path.to_path_buf(),
        source,
    }
}

/// Checks that `sound` can be played, without playing it or needing an audio device.
///
/// # Errors
/// Fails exactly when [`source`] does.
pub fn check(sound: &Sound) -> Result<(), SoundError> {
    source(sound).map(drop)
}

/// Opens `sound` as a source that repeats it without end: no fade, no time limit.
///
/// A file loops through rodio's looping decoder, which seeks back to the start and decodes again.
/// `repeat_infinite()` would not do for files: rodio's decoders report an empty first span for
/// Ogg Vorbis and for MP3 at 24 kHz or below, and `repeat_infinite()` (rodio's `Buffered`) reads
/// that as the end, so the loop would be silent. It would also keep every decoded sample in
/// memory as it plays. The built-in sounds are in memory already, so they keep it.
///
/// # Errors
/// Fails exactly when [`source`] does.
pub fn looped(sound: &Sound) -> Result<Box<dyn Source + Send>, SoundError> {
    match sound {
        Sound::Alarm | Sound::Chime => Ok(Box::new(source(sound)?.repeat_infinite())),
        Sound::File(path) => {
            let decoder = file_decoder(path)?
                .build_looped()
                .map_err(|source| decode_error(path, source))?;
            Ok(Box::new(decoder))
        }
    }
}

/// Opens `sound` as the full-screen timer's alarm: [`looped`], fading in over 500 ms.
///
/// # Errors
/// Fails exactly when [`source`] does.
pub fn alarm_loop(sound: &Sound) -> Result<Box<dyn Source + Send>, SoundError> {
    Ok(Box::new(looped(sound)?.fade_in(ALARM_FADE_IN)))
}

/// Opens `sound` and limits it as `playback` says.
fn bound(sound: &Sound, playback: Playback) -> Result<Box<dyn Source + Send>, SoundError> {
    Ok(match playback {
        Playback::Once { max } => Box::new(source(sound)?.take_duration(max)),
        Playback::Loop { for_ } => Box::new(looped(sound)?.take_duration(for_)),
    })
}

/// Fits `source` to an output with `channels` channels, as rodio 0.16 did.
///
/// rodio 0.23's own conversion leaves every channel after the second silent, so on a 5.1 or 7.1
/// output the mono built-in sounds and stereo files would play from the front pair only. rodio
/// 0.16 copied the last channel of each frame into every extra channel, and [`FillChannels`] does
/// that again. Outputs of one or two channels get `source` itself, so stereo output is exactly
/// what rodio makes of it.
#[must_use]
pub fn fit_channels(
    source: Box<dyn Source + Send>,
    channels: ChannelCount,
) -> Box<dyn Source + Send> {
    if channels.get() <= 2 {
        source
    } else {
        Box::new(FillChannels {
            inner: source,
            to: channels,
            from: channels,
            width: channels,
            pos: 0,
            last: 0.0,
        })
    }
}

/// Widens each frame of `inner` to `to` channels by repeating its last channel, like rodio 0.16's
/// `ChannelCountConverter` (`conversions/channels.rs`). Frames with `to` or more channels pass
/// through unchanged and are left to rodio.
struct FillChannels {
    inner: Box<dyn Source + Send>,
    to: ChannelCount,
    /// Channels of the input frame being read, and of the frame being written. Both are fixed at
    /// the frame's first sample: once a span's last sample is read, `inner` may already report
    /// the next span's channels while this frame still has repeats to write.
    from: ChannelCount,
    width: ChannelCount,
    /// Position in that frame.
    pos: u16,
    /// The last sample read from `inner`.
    last: Sample,
}

impl FillChannels {
    /// The output width of a frame of `from` channels.
    fn widen(&self, from: ChannelCount) -> ChannelCount {
        from.max(self.to)
    }
}

impl Iterator for FillChannels {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.pos == 0 {
            self.from = self.inner.channels();
            self.width = self.widen(self.from);
        }
        if self.pos < self.from.get() {
            self.last = self.inner.next()?;
        }
        self.pos += 1;
        if self.pos == self.width.get() {
            self.pos = 0;
        }
        Some(self.last)
    }
}

impl Source for FillChannels {
    fn current_span_len(&self) -> Option<usize> {
        let from = usize::from(self.inner.channels().get());
        let to = usize::from(self.channels().get());
        self.inner.current_span_len().map(|len| len / from * to)
    }

    fn channels(&self) -> ChannelCount {
        if self.pos == 0 {
            self.widen(self.inner.channels())
        } else {
            self.width
        }
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.pos = 0;
        Ok(())
    }
}

/// Opens the default audio output device. This is the only code that opens it.
///
/// Nothing here may print: a line on stderr would land inside `--json` output and over the
/// full-screen timer. So this mirrors how rodio 0.23 opens its default sink (`stream.rs`): the
/// default device with its default config, else the first other device, skipping "null" drivers,
/// that opens with any config it supports, else the first error. The one difference is that
/// rodio's version prints to stderr when the devices cannot be listed; here that just returns the
/// first error. The sink's own message on drop is switched off too.
///
/// # Errors
/// Returns rodio's error if no audio output device can be opened. It is bare, so each caller
/// words it: [`play`] as [`SoundError::Output`], the full-screen timer with its own prefix.
pub fn open_output() -> Result<MixerDeviceSink, DeviceSinkError> {
    use rodio::DeviceSinkBuilder;
    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    let mut sink = DeviceSinkBuilder::from_default_device()
        .and_then(DeviceSinkBuilder::open_stream)
        .or_else(|first_error| {
            let Ok(devices) = rodio::cpal::default_host().output_devices() else {
                return Err(first_error);
            };
            devices
                .filter(|device| {
                    device
                        .description()
                        .is_ok_and(|d| d.driver().is_some_and(|driver| driver != "null"))
                })
                .find_map(|device| {
                    DeviceSinkBuilder::from_device(device)
                        .and_then(|builder| builder.open_sink_or_fallback())
                        .ok()
                })
                .ok_or(first_error)
        })?;
    sink.log_on_drop(false);
    Ok(sink)
}

/// Plays `sound` as `playback` says and blocks until it has finished.
///
/// The sound is checked before the output device is opened, so a bad file fails without any
/// device.
///
/// # Errors
/// Returns the errors of [`source`], and [`SoundError::Output`] if there is no audio output
/// device.
pub fn play(sound: &Sound, playback: Playback) -> Result<(), SoundError> {
    let source = bound(sound, playback)?;
    let output = open_output()?;
    let player = Player::connect_new(output.mixer());
    player.append(fit_channels(source, output.config().channel_count()));
    player.sleep_until_end();
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
            assert_eq!(src.channels().get(), 1);
            assert_eq!(src.sample_rate().get(), SAMPLE_RATE);
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
        assert_eq!(src.sample_rate().get(), 8_000);
        assert_eq!(src.channels().get(), 1);
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

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures")).join(name)
    }

    #[test]
    fn real_mp3_flac_and_ogg_files_decode() {
        for name in ["tone.mp3", "tone.flac", "tone.ogg"] {
            let sound = Sound::File(fixture(name));
            check(&sound).unwrap_or_else(|e| panic!("{name}: {e}"));
            let src = source(&sound).unwrap();
            assert_eq!(src.channels().get(), 1, "{name}");
            assert_eq!(src.sample_rate().get(), 22_050, "{name}");
            let count = src.count();
            assert!(
                (11_025 - 2_205..=11_025 + 2_205).contains(&count),
                "{name}: {count} samples"
            );
        }
    }

    #[test]
    fn a_fixture_without_an_extension_still_decodes() {
        let pid = std::process::id();
        for (from, to) in [
            ("tone.ogg", format!("cli-timer-{pid}-noext")),
            ("tone.mp3", format!("cli-timer-{pid}-UPPER.MP3")),
        ] {
            let path = std::env::temp_dir().join(to);
            std::fs::copy(fixture(from), &path).unwrap();
            let result = check(&Sound::File(path.clone()));
            std::fs::remove_file(&path).unwrap();
            result.unwrap_or_else(|e| panic!("{from} as {}: {e}", path.display()));
        }
    }

    #[test]
    fn a_text_file_reports_unrecognized_format() {
        let path = std::env::temp_dir().join(format!("cli-timer-{}-words.txt", std::process::id()));
        std::fs::write(&path, "this is plainly not audio, just some text\n").unwrap();
        let err = check(&Sound::File(path.clone())).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            err.to_string(),
            format!(
                "cannot decode sound file {}: Unrecognized format",
                path.display()
            )
        );
    }

    /// The first `len` bytes of `bytes`, in a fresh temp file called `name`.
    fn truncated_copy(bytes: &[u8], len: usize, name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("cli-timer-{}-{name}", std::process::id()));
        std::fs::write(&path, &bytes[..len]).unwrap();
        path
    }

    #[test]
    fn a_truncated_flac_reports_end_of_stream() {
        let flac = std::fs::read(fixture("tone.flac")).unwrap();
        let path = truncated_copy(&flac, 300, "cut.flac");
        let err = check(&Sound::File(path.clone())).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            err.to_string(),
            format!("cannot decode sound file {}: end of stream", path.display())
        );
    }

    #[test]
    fn a_wav_cut_off_before_its_data_reports_end_of_stream() {
        let whole = write_wav("whole.wav", 800);
        let wav = std::fs::read(&whole).unwrap();
        std::fs::remove_file(whole).unwrap();
        // 36 bytes: RIFF header and fmt chunk, no data chunk.
        let path = truncated_copy(&wav, 36, "no-data.wav");
        let err = check(&Sound::File(path.clone())).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            err.to_string(),
            format!("cannot decode sound file {}: end of stream", path.display())
        );
    }

    #[test]
    fn every_decoder_error_reads_as_it_did_with_rodio_0_16() {
        // rodio 0.16's `Display` for `DecoderError` (decoder/mod.rs), which `-s` errors showed.
        let cases = [
            (DecoderError::UnrecognizedFormat, "Unrecognized format"),
            (
                DecoderError::IoError("end of stream".to_owned()),
                "end of stream",
            ),
            (DecoderError::DecodeError("bad packet"), "bad packet"),
            (DecoderError::LimitError("too big"), "too big"),
            (DecoderError::ResetRequired, "Reset required"),
            (DecoderError::NoStreams, "No streams"),
        ];
        for (error, text) in cases {
            assert_eq!(decode_detail(&error), text, "{error:?}");
        }
    }

    #[test]
    fn an_adpcm_wav_is_rejected_as_undecodable() {
        // `-s` takes PCM wav only (chosen 2026-10-27); rodio's `symphonia-adpcm` stays off.
        let err = check(&Sound::File(fixture("tone-adpcm.wav"))).unwrap_err();
        assert!(matches!(err, SoundError::Decode { .. }), "{err:?}");
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
            &Sound::Alarm,
            Playback::Once {
                max: Duration::from_millis(500),
            },
        )
        .unwrap();
        assert_eq!(cut.count(), n(500));

        let whole = bound(
            &Sound::Alarm,
            Playback::Once {
                max: Duration::from_secs(10),
            },
        )
        .unwrap();
        assert_eq!(whole.count(), n(1350));
    }

    #[test]
    fn loop_repeats_the_cycle_for_the_requested_time_then_stops() {
        let looped: Vec<f32> = bound(
            &Sound::Alarm,
            Playback::Loop {
                for_: Duration::from_secs(3),
            },
        )
        .unwrap()
        .collect();
        // rodio rounds the cut-off to a frame or two, so allow a few samples either way.
        assert!(looped.len().abs_diff(n(3000)) <= 8, "{}", looped.len());
        let cycle = n(1350);
        let alarm = collect(&Sound::Alarm);
        assert_eq!(looped[..cycle], alarm[..]);
        assert_eq!(looped[cycle..2 * cycle], alarm[..]);
    }

    /// `samples` as a source of `channels` interleaved channels.
    fn buffer(channels: u16, samples: &[f32]) -> Box<dyn Source + Send> {
        let channels = ChannelCount::new(channels).unwrap();
        Box::new(SamplesBuffer::new(channels, RATE, samples.to_vec()))
    }

    const SIX: ChannelCount = rodio::nz!(6);

    #[test]
    fn mono_fills_all_six_channels_as_rodio_0_16_did() {
        let six = fit_channels(buffer(1, &[0.1, 0.2, 0.3]), SIX);
        assert_eq!(six.channels(), SIX);
        assert_eq!(six.sample_rate(), RATE);
        assert_eq!(six.current_span_len(), Some(18));
        let samples: Vec<f32> = six.collect();
        assert_eq!(samples, [[0.1; 6], [0.2; 6], [0.3; 6]].concat());
    }

    #[test]
    fn stereo_repeats_its_right_channel_into_the_four_extra_channels() {
        let six = fit_channels(buffer(2, &[0.1, 0.2, 0.3, 0.4]), SIX);
        assert_eq!(six.channels(), SIX);
        assert_eq!(six.current_span_len(), Some(12));
        let samples: Vec<f32> = six.collect();
        assert_eq!(
            samples,
            [0.1, 0.2, 0.2, 0.2, 0.2, 0.2, 0.3, 0.4, 0.4, 0.4, 0.4, 0.4]
        );
    }

    #[test]
    fn the_alarm_fills_six_channels_for_its_whole_cycle() {
        let alarm = collect(&Sound::Alarm);
        let six: Vec<f32> = fit_channels(source(&Sound::Alarm).unwrap(), SIX).collect();
        let expected: Vec<f32> = alarm.iter().flat_map(|s| [*s; 6]).collect();
        assert_eq!(six, expected);
    }

    #[test]
    fn a_mono_frame_is_finished_before_a_stereo_span_starts() {
        // rodio's queue reports the next source's channels as soon as the current one's last
        // sample is read, which is in the middle of the widened frame.
        let (input, output) = rodio::queue::queue(false);
        input.append(SamplesBuffer::new(MONO, RATE, vec![0.1]));
        input.append(SamplesBuffer::new(
            rodio::nz!(2),
            RATE,
            vec![0.2, 0.3, 0.4, 0.5],
        ));
        let samples: Vec<f32> = fit_channels(Box::new(output), SIX).collect();
        assert_eq!(
            samples,
            [
                [0.1; 6],
                [0.2, 0.3, 0.3, 0.3, 0.3, 0.3],
                [0.4, 0.5, 0.5, 0.5, 0.5, 0.5]
            ]
            .concat()
        );
    }

    #[test]
    fn a_stereo_output_gets_mono_and_stereo_sources_unchanged() {
        for (channels, samples) in [(1, vec![0.1, 0.2, 0.3]), (2, vec![0.1, 0.2, 0.3, 0.4])] {
            let src = fit_channels(buffer(channels, &samples), rodio::nz!(2));
            assert_eq!(src.channels().get(), channels);
            assert_eq!(src.current_span_len(), Some(samples.len()));
            assert_eq!(src.collect::<Vec<f32>>(), samples, "{channels} channels");
        }
    }

    #[test]
    fn a_source_as_wide_as_the_output_or_wider_passes_through() {
        // rodio drops the extra channels of a wider source, in 0.16 and 0.23 alike.
        for channels in [6, 8] {
            let samples: Vec<f32> = (0..channels * 2).map(|i| f32::from(i) / 100.0).collect();
            let src = fit_channels(buffer(channels, &samples), SIX);
            assert_eq!(src.channels().get(), channels);
            assert_eq!(src.current_span_len(), Some(samples.len()));
            assert_eq!(src.collect::<Vec<f32>>(), samples, "{channels} channels");
        }
    }

    #[test]
    fn remind_and_ding_still_word_a_missing_output_the_same_way() {
        let err = SoundError::from(DeviceSinkError::NoDevice);
        assert_eq!(
            err.to_string(),
            "no audio output available: Could not find any output device"
        );
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

    /// Samples louder than this count as sound. The fixtures are a sine at amplitude 0.125.
    const PEAK: f32 = 0.05;

    /// Plays `src` the way the app does (a `Player` on a mixer, behind [`fit_channels`]) on a
    /// mono mixer at `rate` with no device, and returns its first `n` output samples. The
    /// player keeps the output alive after the sound ends, so the result is always `n` long.
    fn through_player(src: Box<dyn Source + Send>, rate: u32, n: usize) -> Vec<f32> {
        let (mixer, output) = rodio::mixer::mixer(MONO, SampleRate::new(rate).unwrap());
        let player = Player::connect_new(&mixer);
        player.append(fit_channels(src, MONO));
        let samples = output.take(n).collect();
        drop(player);
        samples
    }

    fn loud(samples: &[f32]) -> usize {
        samples.iter().filter(|s| s.abs() > PEAK).count()
    }

    /// Samples in one play of fixture `name`.
    fn one_play(name: &str) -> usize {
        source(&Sound::File(fixture(name))).unwrap().count()
    }

    #[test]
    fn looped_files_sound_every_time_round_through_the_player() {
        // tone.flac is the control: it looped before the fix too.
        for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
            let p = one_play(name);
            let out = through_player(looped(&Sound::File(fixture(name))).unwrap(), 22_050, 3 * p);
            for w in 0..3 {
                // measured: about 0.72 p loud samples per play
                let count = loud(&out[w * p..(w + 1) * p]);
                assert!(
                    count > p * 6 / 10,
                    "{name}: play {} has {count} loud",
                    w + 1
                );
            }
        }
    }

    #[test]
    fn looped_files_repeat_exactly() {
        for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
            let p = one_play(name);
            let out: Vec<f32> = looped(&Sound::File(fixture(name)))
                .unwrap()
                .take(3 * p)
                .collect();
            assert_eq!(out.len(), 3 * p, "{name}");
            assert_eq!(
                out[p..2 * p],
                out[2 * p..],
                "{name}: play 3 differs from play 2"
            );
        }
    }

    #[test]
    fn the_full_screen_alarm_sounds_after_its_fade_in() {
        for name in ["tone.ogg", "tone.mp3"] {
            let p = one_play(name);
            let alarm = alarm_loop(&Sound::File(fixture(name))).unwrap();
            let out = through_player(alarm, 22_050, 3 * p);
            // the first 100 ms are still fading in: none loud (about 1 450 without the fade)
            assert_eq!(loud(&out[..2_205]), 0, "{name}: no fade-in");
            // the fade-in covers play 1; plays 2 and 3 have about 0.72 p loud samples each
            let count = loud(&out[p..]);
            assert!(
                count > p * 12 / 10,
                "{name}: {count} loud after the fade-in"
            );
        }
    }

    #[test]
    fn a_looped_file_rings_for_the_requested_time() {
        for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
            let ring = bound(
                &Sound::File(fixture(name)),
                Playback::Loop {
                    for_: Duration::from_secs(2),
                },
            )
            .unwrap();
            // TakeDuration truncates the nanoseconds per sample, so its error grows with ring
            // time, rate and channels; 2 s of 22.05 kHz mono lands exactly on 44 100.
            assert_eq!(ring.count(), 44_100, "{name}");
        }
    }

    #[test]
    fn one_play_of_each_fixture_through_the_player_has_sound() {
        // A control for `ding`: one-shot plays already worked through the player.
        for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
            let p = one_play(name);
            let out = through_player(source(&Sound::File(fixture(name))).unwrap(), 22_050, p);
            assert!(loud(&out) > p * 6 / 10, "{name}");
        }
    }

    #[test]
    fn ogg_and_low_rate_mp3_need_rodios_player_to_be_heard() {
        // Tripwire. rodio 0.23's decoders report an empty first span for Vorbis and for MP3 at
        // 24 kHz or below, and only `Player`'s queue reads past it: fed straight to rodio's
        // converter (what direct mixer wiring does) these yield no samples at all. While this
        // holds, sounds must go through `Player`. The day it fails, the parked surround rewire
        // (docs/parking-lot.md) may be possible.
        let rate = SampleRate::new(22_050).unwrap();
        let cases: [(&str, Box<dyn Source + Send>); 4] = [
            (
                "looped tone.ogg",
                looped(&Sound::File(fixture("tone.ogg"))).unwrap(),
            ),
            (
                "looped tone.mp3",
                looped(&Sound::File(fixture("tone.mp3"))).unwrap(),
            ),
            (
                "one tone.ogg",
                source(&Sound::File(fixture("tone.ogg"))).unwrap(),
            ),
            (
                "one tone.mp3",
                source(&Sound::File(fixture("tone.mp3"))).unwrap(),
            ),
        ];
        // No samples at all, not merely quiet ones: the converter ends at once. Every case is
        // checked before failing, so one change does not hide the others.
        let produced: Vec<(&str, usize)> = cases
            .into_iter()
            .map(|(what, src)| {
                let n = rodio::source::UniformSourceIterator::new(src, MONO, rate)
                    .take(30_000)
                    .count();
                (what, n)
            })
            .filter(|&(_, n)| n > 0)
            .collect();
        assert!(produced.is_empty(), "now produce samples: {produced:?}");
    }

    #[test]
    fn looped_files_keep_sounding_when_resampled() {
        for name in ["tone.ogg", "tone.mp3", "tone.flac"] {
            // one play at the output rate, a few samples over what the resampler makes
            let p = one_play(name) * 48_000 / 22_050;
            let out = through_player(looped(&Sound::File(fixture(name))).unwrap(), 48_000, 3 * p);
            for seam in 1..3 {
                // the 100 ms just after each seam; measured: about 3 500 loud samples
                let count = loud(&out[seam * p..seam * p + 4_800]);
                assert!(count > 2_500, "{name}: {count} loud after seam {seam}");
            }
        }
    }

    #[test]
    fn a_looped_wav_repeats_exactly() {
        // A control: wav files looped before the fix too.
        // write_wav's ramp repeats every 1 000 samples; 3 999 is not a multiple of that, so a loop
        // that restarted off by a whole ramp would still show.
        let path = write_wav("loop.wav", 3_999); // about 0.5 s at 8 kHz
        let out: Vec<f32> = looped(&Sound::File(path.clone()))
            .unwrap()
            .take(3 * 3_999)
            .collect();
        let once: Vec<f32> = source(&Sound::File(path.clone())).unwrap().collect();
        std::fs::remove_file(path).unwrap();
        assert_eq!(out[..3_999], once[..]);
        assert_eq!(out[3_999..2 * 3_999], once[..]);
        assert_eq!(out[2 * 3_999..], once[..]);
    }
}
