use crate::{
    alerts::{Imported, Intensity, Kind, Rule},
    i18n::Text,
};
use rodio::{Decoder, OutputStream, Sink, buffer::SamplesBuffer};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

mod g7 {
    use super::*;
    include!("g7_sounds.rs");
}

impl Imported {
    pub fn label(&self) -> String {
        let mut matches = g7::SOUNDS
            .iter()
            .filter(|(hash, _, _)| *hash == self.digest);
        let first = matches.next();
        // Three system tones are byte-identical to signal-loss tones in the G7 pack.
        // The pack filename distinguishes them; a renamed copy gets both labels.
        if let (Some((_, system, Some(level))), Some((_, signal, _))) = (first, matches.next()) {
            let title = match self.filename.as_str() {
                "Alerte système.wav" => system.get().to_owned(),
                "Perte de signal.wav" => signal.get().to_owned(),
                _ => format!("{} / {}", system.get(), signal.get()),
            };
            return format!("G7 · {} · {}", title, level.title().get());
        }
        match first {
            Some((_, title, Some(level))) => {
                format!("G7 · {} · {}", title.get(), level.title().get())
            }
            Some((_, title, None)) => format!("G7 · {}", title.get()),
            None => self.filename.clone(),
        }
    }
    fn path(&self, directory: &Path) -> Result<PathBuf, Text> {
        if self.digest.len() != 64 || !self.digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Text::AudioError);
        }
        Ok(directory
            .join("sounds")
            .join(format!("{}.wav", self.digest)))
    }
}

const MAX_BYTES: u64 = 32 * 1024 * 1024;
pub fn import(source: &Path, directory: &Path) -> Result<Imported, Text> {
    let mut bytes = Vec::new();
    fs::File::open(source)
        .and_then(|f| f.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|_| Text::ReadError)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Text::InvalidAudio);
    }
    let mut reader = hound::WavReader::new(Cursor::new(&bytes)).map_err(|_| Text::InvalidAudio)?;
    let spec = reader.spec();
    if !(1..=2).contains(&spec.channels)
        || !(8000..=192000).contains(&spec.sample_rate)
        || reader.duration() == 0
        || reader.duration() > 60 * spec.sample_rate
    {
        return Err(Text::InvalidAudio);
    }
    // Decode the complete file before replacing the user's previous selection.
    let valid = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 8 | 16 | 24 | 32) => reader.samples::<i32>().all(|s| s.is_ok()),
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .all(|s| s.is_ok_and(|v| v.is_finite())),
        _ => false,
    };
    if !valid {
        return Err(Text::InvalidAudio);
    }
    let imported = Imported {
        digest: format!("{:x}", Sha256::digest(&bytes)),
        filename: source
            .file_name()
            .ok_or(Text::InvalidAudio)?
            .to_string_lossy()
            .into(),
    };
    let sounds = directory.join("sounds");
    fs::create_dir_all(&sounds).map_err(|_| Text::WriteError)?;
    let path = imported.path(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(&sounds).map_err(|_| Text::WriteError)?;
    file.write_all(&bytes).map_err(|_| Text::WriteError)?;
    file.persist(path).map_err(|_| Text::WriteError)?;
    Ok(imported)
}

pub fn play(rule: &Rule, directory: &Path, cancelled: impl Fn() -> bool) -> Result<(), Text> {
    let (_device, handle) = OutputStream::try_default().map_err(|_| Text::AudioError)?;
    let sink = Sink::try_new(&handle).map_err(|_| Text::AudioError)?;
    sink.set_volume(rule.intensity.gain());
    let mut fallback = false;
    if rule.use_import {
        let source = rule
            .imported
            .as_ref()
            .and_then(|i| fs::File::open(i.path(directory).ok()?).ok())
            .and_then(|f| Decoder::new_wav(std::io::BufReader::new(f)).ok());
        if let Some(source) = source {
            sink.append(source);
        } else {
            fallback = true;
        }
    }
    if !rule.use_import || fallback {
        sink.append(chime(rule.tone));
    }
    while !sink.empty() && !cancelled() {
        std::thread::sleep(Duration::from_millis(40));
    }
    if fallback {
        Err(Text::AudioError)
    } else {
        Ok(())
    }
}

// Distinct local melodies. The short envelope avoids clicks on note boundaries.
fn chime(tone: Kind) -> SamplesBuffer<f32> {
    let notes: &[f32] = match tone {
        Kind::UrgentLow => &[880., 660., 880., 660.],
        Kind::Low => &[660., 440.],
        Kind::High => &[660., 880.],
        Kind::SignalLoss => &[440., 440., 440.],
        Kind::Rise => &[440., 660., 880.],
        Kind::Fall => &[880., 660., 440.],
    };
    let rate = 24_000;
    let mut samples = Vec::new();
    for &frequency in notes {
        for i in 0..4800 {
            let edge = (i.min(4799 - i) as f32 / 240.).min(1.);
            samples.push(
                (i as f32 * std::f32::consts::TAU * frequency / rate as f32).sin() * edge * 0.45,
            );
        }
        samples.resize(samples.len() + 1800, 0.);
    }
    SamplesBuffer::new(1, rate, samples)
}

pub(crate) fn dispatch(
    alerts: Vec<Kind>,
    rules: Vec<Rule>,
    directory: PathBuf,
    events: std::sync::mpsc::Sender<crate::services::Event>,
    ctx: eframe::egui::Context,
    cancel: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ticket: u64,
) {
    if !alerts.is_empty() {
        let events = events.clone();
        let ctx = ctx.clone();
        let sequence = cancel.clone();
        std::thread::spawn(move || {
            if sequence.load(std::sync::atomic::Ordering::Relaxed) != ticket {
                return;
            }
            let labels = alerts
                .iter()
                .map(|k| k.title().get())
                .collect::<Vec<_>>()
                .join(" · ");
            let mut notification = notify_rust::Notification::new();
            #[cfg(windows)]
            notification.app_id("velopack.EasyGlucose");
            if notification
                .summary("Easy Glucose")
                .body(&labels)
                .show()
                .is_err()
            {
                let _ = events.send(crate::services::Event::Notice(
                    Text::NotificationError.english().into(),
                ));
                ctx.request_repaint_of(eframe::egui::ViewportId::ROOT);
            }
        });
    }
    std::thread::spawn(move || {
        let cancelled = || cancel.load(std::sync::atomic::Ordering::Relaxed) != ticket;
        for rule in rules {
            if cancelled() {
                break;
            }
            if let Err(error) = play(&rule, &directory, cancelled) {
                let _ = events.send(crate::services::Event::Notice(error.english().into()));
                ctx.request_repaint_of(eframe::egui::ViewportId::ROOT);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_wav_survives_source_removal_and_truncated_audio_is_rejected() {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let path = source.path().join("My chime.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 24000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for sample in [0i16, 1000, 0, -1000] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        let file = import(&path, data.path()).ok().unwrap();
        assert_eq!(file.label(), "My chime.wav");
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 2]).unwrap();
        assert!(import(&path, data.path()).is_err());
        fs::remove_file(path).unwrap();
        assert_eq!(
            fs::read(file.path(data.path()).ok().unwrap()).unwrap(),
            bytes
        );
    }
}
