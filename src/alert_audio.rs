use crate::{
    alerts::{Imported, Intensity, Kind, Rule},
    i18n::Text,
};
use rodio::{Decoder, OutputStream, Sink};
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
    let imported = rule
        .imported
        .as_ref()
        .filter(|_| rule.use_import)
        .and_then(|i| fs::File::open(i.path(directory).ok()?).ok())
        .and_then(|file| Decoder::new_wav(std::io::BufReader::new(file)).ok());
    let fallback = rule.use_import && imported.is_none();
    if let Some(source) = imported {
        sink.set_volume(rule.intensity.gain());
        sink.append(source);
    } else {
        // G7 records each intensity separately. Play its samples at their native level.
        let bytes = g7_recording(rule.tone, rule.intensity);
        sink.append(Decoder::new_flac(Cursor::new(bytes)).map_err(|_| Text::AudioError)?);
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

fn g7_recording(tone: Kind, intensity: Intensity) -> &'static [u8] {
    macro_rules! recordings {
        ($name:literal) => {
            match intensity {
                Intensity::Soft => {
                    include_bytes!(concat!("../assets/sounds/g7/soft/", $name, ".flac"))
                }
                Intensity::Medium => {
                    include_bytes!(concat!("../assets/sounds/g7/medium/", $name, ".flac"))
                }
                Intensity::Intense => {
                    include_bytes!(concat!("../assets/sounds/g7/intense/", $name, ".flac"))
                }
            }
        };
    }
    match tone {
        Kind::UrgentLow => recordings!("urgent_low"),
        Kind::Low => recordings!("low"),
        Kind::High => recordings!("high"),
        Kind::SignalLoss => recordings!("signal_loss"),
        Kind::Rise => recordings!("rise"),
        Kind::Fall => recordings!("fall"),
    }
}

impl Imported {
    pub(crate) fn g7_preset(&self) -> Option<(Kind, Intensity)> {
        for &(digest, title, level) in g7::SOUNDS {
            if digest != self.digest {
                continue;
            }
            let Some(level) = level else {
                continue;
            };
            let kind = match title {
                Text::UrgentLow => Kind::UrgentLow,
                Text::Low => Kind::Low,
                Text::High => Kind::High,
                Text::FastRise => Kind::Rise,
                Text::FastFall => Kind::Fall,
                // System alerts share signal-loss bytes at each intensity. Preserve
                // their own imported label unless the supplied file identifies signal loss.
                Text::SignalLossTone if self.filename == "Perte de signal.wav" => Kind::SignalLoss,
                _ => continue,
            };
            return Some((kind, level));
        }
        None
    }
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
    fn all_default_tones_decode_to_the_original_g7_samples() {
        use rodio::Source;
        let original_pcm = [
            [
                "e33e6381f21db3ae3f1085165e73070804d5eaa8a58a8303a3d90f8721cf0986",
                "c3a0a2bffdd61a2423646d872f4df257a1e15de5ef9f0a4f0d84472b65f899c2",
                "3ec26e3fa9b3c4b7a235a65a693db476ebc6df85f05759789297308b3c3f27a9",
            ],
            [
                "a83b918b93bd227ab9033077cc47e53a3f76732538f9c896170cce4f5fc66ae3",
                "f1d671a333977495cd2b562c0617a134868ac0c7ba7fbb9e72ea1ece4a6bcc9b",
                "c7e9b264bc8081ed847c74c845e9935ae72255b449d3927dfe887441dcdd0414",
            ],
            [
                "17485c10541d336ed81c3d30a55b907a66a7e2f1dea1e3cd7cd6d9e953c1bdec",
                "dfb7e664439415383403b5130e79bfb70ebec436fc8d398f0e3f0fcd32d89013",
                "4bbf1b525795cae433ec965a9cf116d7d04d252480ddf23e46f6e6ca1adea3d9",
            ],
            [
                "5d86d4a5cf0856f38461d93d60a6e4f75a1d788f1ce19df34102dab980a184db",
                "c30e4ec70bdffe19f9b678f0ed45093acc26d9483a19a1e513e56587ff56c657",
                "1b374fe85cc340c2287e5aa501cbeb50347cb708a551b8252842b4c47c463920",
            ],
            [
                "d09e42b91a9af8cb709ae118c3275e98c51d9358d555f323e0a1279651ad5f0e",
                "2c669f5ac0e4880f7560ca94fb07f9be1671fdc1bd3737912f146316623b3d90",
                "3f94cf2a6be237e655a2e9d76b4a6371c1d06bae8518bfc3e525f420e424ef82",
            ],
            [
                "6e91d221da2cdc09c33a1da3648c7cd322f0ebdc1419c8d985412b42e5a36dbb",
                "b1d39b9e5367657aac0661d63d17f4f3b5e248d42c9b0f1eb29dd3047ab53027",
                "e673fd168f988afca5161ac9e377b07b30f4636c146c5567c0529dc43330d97b",
            ],
        ];
        let settings = crate::alerts::Settings::default();
        for (kind, hashes) in Kind::ALL.into_iter().zip(original_pcm) {
            let rule = &settings.rules[kind as usize];
            assert_eq!(rule.tone, kind);
            assert!(!rule.use_import);
            for (level, hash) in [Intensity::Soft, Intensity::Medium, Intensity::Intense]
                .into_iter()
                .zip(hashes)
            {
                let audio = Decoder::new_flac(Cursor::new(g7_recording(rule.tone, level))).unwrap();
                assert_eq!((audio.channels(), audio.sample_rate()), (2, 48_000));
                let bytes: Vec<_> = audio.flat_map(i16::to_le_bytes).collect();
                assert_eq!(
                    format!("{:x}", Sha256::digest(bytes)),
                    hash,
                    "{kind:?} {level:?}"
                );
            }
        }
    }

    #[test]
    fn existing_g7_import_becomes_a_selectable_preset() {
        let mut settings = crate::alerts::Settings::default();
        let rule = &mut settings.rules[Kind::UrgentLow as usize];
        rule.enabled = true;
        rule.use_import = true;
        rule.imported = Some(Imported {
            digest: "d9247d0175c3dce1bcc2313e377b99a8d313f42ef1305aa30711cb2ab30ceba5".into(),
            filename: "Bas urgent.wav".into(),
        });
        settings.normalize();
        let rule = &settings.rules[Kind::UrgentLow as usize];
        assert!(rule.enabled);
        assert!(!rule.use_import);
        assert_eq!(
            (rule.tone, rule.intensity),
            (Kind::UrgentLow, Intensity::Soft)
        );
    }
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
