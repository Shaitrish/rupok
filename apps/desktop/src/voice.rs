//! Offline voice input (speech-to-text) for the board: the whisper.cpp model
//! catalogue, the model files on disk, the PCM the webview sends, and the
//! transcription itself (feature `whisper`).
//!
//! Privacy: audio arrives as 16 kHz mono PCM in memory, is transcribed and
//! dropped. Nothing is recorded or written to disk. The only network use is
//! the model download the teacher starts from Settings, pinned to a SHA-256.
//!
//! The catalogue, PCM decoding, WAV parsing, resampling, language mapping and
//! transcript clean-up are plain Rust, always built and tested. Loading a
//! model and transcribing need the `whisper` feature (whisper.cpp).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Whisper wants 16 kHz mono f32.
pub const SAMPLE_RATE: u32 = 16_000;
/// The longest clip one transcription accepts (seconds). Dictation is short;
/// this bounds memory for untrusted input from the webview.
pub const MAX_SECONDS: usize = 120;
/// [`MAX_SECONDS`] in samples.
pub const MAX_SAMPLES: usize = SAMPLE_RATE as usize * MAX_SECONDS;

/// A downloadable multilingual whisper.cpp model (English and Bangla).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    /// "tiny", "base", "small".
    pub id: &'static str,
    /// File name on Hugging Face and on disk.
    pub file: &'static str,
    /// Exact size in bytes.
    pub bytes: u64,
    /// SHA-256 of the file (lower-case hex).
    pub sha256: &'static str,
}

/// Where the models come from. The URL is built here, never taken from JS.
pub const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// The models the teacher can download (multilingual, so Bangla works).
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny",
        file: "ggml-tiny.bin",
        bytes: 77_691_713,
        sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    },
    ModelInfo {
        id: "base",
        file: "ggml-base.bin",
        bytes: 147_951_465,
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    },
    ModelInfo {
        id: "small",
        file: "ggml-small.bin",
        bytes: 487_601_967,
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    },
];

/// Errors of the voice module.
#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    #[error("unknown voice model \"{0}\"")]
    UnknownModel(String),
    #[error("the voice model is not downloaded")]
    ModelMissing,
    #[error("audio must be 16-bit PCM (an even number of bytes)")]
    OddPcm,
    #[error("the recording is too long (over {MAX_SECONDS} seconds)")]
    TooLong,
    #[error("not a WAV file this reader understands: {0}")]
    BadWav(&'static str),
    #[error("the download was cancelled")]
    Cancelled,
    #[error("the download is damaged (checksum mismatch); please try again")]
    Checksum,
    #[error("this build of Rupok has no offline voice (built without the `whisper` feature)")]
    NotBuilt,
    #[error("{0}")]
    Engine(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Look a model up by id.
pub fn model(id: &str) -> Result<&'static ModelInfo, VoiceError> {
    MODELS
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| VoiceError::UnknownModel(id.into()))
}

/// The model URL (fixed host and path).
pub fn model_url(m: &ModelInfo) -> String {
    format!("{MODEL_BASE_URL}{}", m.file)
}

/// The model files in one folder (the app data dir's `voice/`).
#[derive(Debug, Clone)]
pub struct ModelStore {
    dir: PathBuf,
}

impl ModelStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, m: &ModelInfo) -> PathBuf {
        self.dir.join(m.file)
    }

    /// The partial download, renamed into place once its checksum matches.
    pub fn part_path(&self, m: &ModelInfo) -> PathBuf {
        self.dir.join(format!("{}.part", m.file))
    }

    /// True when the model file is present with its exact size and the ggml
    /// magic (the checksum was verified when it was downloaded).
    pub fn installed(&self, m: &ModelInfo) -> bool {
        let p = self.path(m);
        let Ok(meta) = fs::metadata(&p) else {
            return false;
        };
        meta.len() == m.bytes && has_ggml_magic(&p)
    }

    /// Delete a model (and any partial download). Missing files are fine.
    pub fn remove(&self, m: &ModelInfo) -> Result<(), VoiceError> {
        for p in [self.path(m), self.part_path(m)] {
            match fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

/// whisper.cpp model files start with "ggml" as a little-endian u32 (b"lmgg").
fn has_ggml_magic(p: &Path) -> bool {
    use io::Read;
    let mut b = [0u8; 4];
    fs::File::open(p)
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
        && u32::from_le_bytes(b) == 0x6767_6d6c
}

/// Decode little-endian 16-bit mono PCM (what the board sends) to f32.
pub fn pcm16_to_f32(bytes: &[u8]) -> Result<Vec<f32>, VoiceError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(VoiceError::OddPcm);
    }
    if bytes.len() / 2 > MAX_SAMPLES {
        return Err(VoiceError::TooLong);
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    Ok(pairs
        .iter()
        .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0)
        .collect())
}

/// A minimal WAV reader (PCM 16-bit, any rate, mono or stereo), mixed down
/// to mono. Used by the tests and for clips dropped in by tools.
pub fn parse_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32), VoiceError> {
    let u16le = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32le = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(VoiceError::BadWav("missing RIFF/WAVE header"));
    }
    // (format, channels, rate, bits per sample)
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut i = 12usize;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32le(bytes, i + 4) as usize;
        let start = i + 8;
        let end = start
            .checked_add(len)
            .ok_or(VoiceError::BadWav("chunk size"))?
            .min(bytes.len());
        let body = &bytes[start..end];
        match id {
            b"fmt " if body.len() >= 16 => {
                fmt = Some((
                    u16le(body, 0),
                    u16le(body, 2),
                    u32le(body, 4),
                    u16le(body, 14),
                ));
            }
            b"data" => data = Some(body),
            _ => {}
        }
        // Chunks are padded to an even length.
        i = start.saturating_add(len).saturating_add(len & 1);
    }
    let (format, channels, rate, bits) = fmt.ok_or(VoiceError::BadWav("no fmt chunk"))?;
    let data = data.ok_or(VoiceError::BadWav("no data chunk"))?;
    if format != 1 || bits != 16 {
        return Err(VoiceError::BadWav("only 16-bit PCM is supported"));
    }
    if channels == 0 || rate == 0 {
        return Err(VoiceError::BadWav("zero channels or rate"));
    }
    let ch = channels as usize;
    let frames = data.len() / (2 * ch);
    if frames as u64 > rate as u64 * MAX_SECONDS as u64 {
        return Err(VoiceError::TooLong);
    }
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut sum = 0f32;
        for c in 0..ch {
            let k = (f * ch + c) * 2;
            sum += i16::from_le_bytes([data[k], data[k + 1]]) as f32 / 32768.0;
        }
        out.push(sum / ch as f32);
    }
    Ok((out, rate))
}

/// Linear resampling to 16 kHz. Good enough for speech; the board already
/// sends 16 kHz, so this only runs for WAV input.
pub fn resample_to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE || samples.is_empty() || rate == 0 {
        return samples.to_vec();
    }
    let ratio = rate as f64 / SAMPLE_RATE as f64;
    let n = ((samples.len() as f64) / ratio).floor() as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos.floor() as usize;
            let t = (pos - j as f64) as f32;
            let a = samples[j.min(samples.len() - 1)];
            let b = samples[(j + 1).min(samples.len() - 1)];
            a + (b - a) * t
        })
        .collect()
}

/// A BCP 47 tag from the board ("bn-BD", "en-US") to a whisper language code.
/// Unknown tags fall back to auto-detection (`None`).
pub fn whisper_lang(tag: &str) -> Option<&'static str> {
    let primary = tag
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match primary.as_str() {
        "bn" => Some("bn"),
        "en" => Some("en"),
        "hi" => Some("hi"),
        "ur" => Some("ur"),
        "ar" => Some("ar"),
        _ => None,
    }
}

/// Root-mean-square level; below [`SILENCE_RMS`] the clip is treated as
/// silence and not sent to the model (whisper invents text for silence).
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// See [`rms`].
pub const SILENCE_RMS: f32 = 0.004;

/// Tidy whisper output: drop non-speech tags ("[BLANK_AUDIO]", "(music)",
/// "*laughs*"), collapse whitespace.
pub fn clean_transcript(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth: Option<char> = None;
    for ch in raw.chars() {
        match (depth, ch) {
            (None, '[') => depth = Some(']'),
            (None, '(') => depth = Some(')'),
            (None, '*') => depth = Some('*'),
            (Some(close), c) if c == close => depth = None,
            (Some(_), _) => {}
            (None, c) => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- whisper

#[cfg(feature = "whisper")]
mod engine {
    use super::*;
    use std::sync::Mutex;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    /// The loaded model, kept between dictations (loading takes a moment).
    #[derive(Default)]
    pub struct Engine {
        ctx: Mutex<Option<(PathBuf, WhisperContext)>>,
    }

    impl Engine {
        /// Transcribe 16 kHz mono samples with the model at `model_path`.
        pub fn transcribe(
            &self,
            model_path: &Path,
            samples: &[f32],
            lang: Option<&str>,
        ) -> Result<String, VoiceError> {
            if samples.len() > MAX_SAMPLES {
                return Err(VoiceError::TooLong);
            }
            if rms(samples) < SILENCE_RMS {
                return Ok(String::new());
            }
            let mut guard = self.ctx.lock().unwrap_or_else(|e| e.into_inner());
            if guard.as_ref().is_none_or(|(p, _)| p != model_path) {
                *guard = None;
                let path = model_path
                    .to_str()
                    .ok_or_else(|| VoiceError::Engine("model path is not UTF-8".into()))?;
                let ctx =
                    WhisperContext::new_with_params(path, WhisperContextParameters::default())
                        .map_err(|e| {
                            VoiceError::Engine(format!("could not load the voice model: {e}"))
                        })?;
                *guard = Some((model_path.to_path_buf(), ctx));
            }
            let (_, ctx) = guard.as_ref().expect("loaded above");
            let mut state = ctx
                .create_state()
                .map_err(|e| VoiceError::Engine(e.to_string()))?;
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_language(Some(lang.unwrap_or("auto")));
            let threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(1, 8);
            p.set_n_threads(threads as i32);
            p.set_no_context(true);
            p.set_no_timestamps(true);
            p.set_suppress_blank(true);
            p.set_suppress_nst(true);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            state
                .full(p, samples)
                .map_err(|e| VoiceError::Engine(e.to_string()))?;
            let mut text = String::new();
            for seg in state.as_iter() {
                if let Ok(s) = seg.to_str_lossy() {
                    text.push_str(&s);
                }
            }
            Ok(clean_transcript(&text))
        }

        /// Forget the loaded model (after it is removed).
        pub fn unload(&self) {
            *self.ctx.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }

    /// Download a model to the store with progress; checks the SHA-256 and
    /// renames into place. `cancel` is polled between chunks.
    pub fn download(
        store: &ModelStore,
        m: &ModelInfo,
        cancel: &std::sync::atomic::AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<(), VoiceError> {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Write};
        use std::sync::atomic::Ordering;
        fs::create_dir_all(store.dir())?;
        let part = store.part_path(m);
        let res = (|| {
            let resp = ureq::get(&model_url(m))
                .call()
                .map_err(|e| VoiceError::Engine(format!("download failed: {e}")))?;
            let mut body = resp.into_body();
            let mut reader = body.as_reader();
            let mut file = fs::File::create(&part)?;
            let mut hash = Sha256::new();
            let mut buf = vec![0u8; 256 * 1024];
            let mut done = 0u64;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(VoiceError::Cancelled);
                }
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                done += n as u64;
                if done > m.bytes {
                    return Err(VoiceError::Checksum);
                }
                hash.update(&buf[..n]);
                file.write_all(&buf[..n])?;
                progress(done, m.bytes);
            }
            file.sync_all()?;
            let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if done != m.bytes || hex != m.sha256 {
                return Err(VoiceError::Checksum);
            }
            fs::rename(&part, store.path(m))?;
            Ok(())
        })();
        if res.is_err() {
            let _ = fs::remove_file(&part);
        }
        res
    }
}

#[cfg(feature = "whisper")]
pub use engine::{Engine, download};

/// True when this build can transcribe offline.
pub const BUILT: bool = cfg!(feature = "whisper");

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit PCM WAV with `samples` at `rate` and `channels`.
    fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        v.extend_from_slice(&(channels * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(b"abc\0"); // odd chunk + pad byte
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    #[test]
    fn catalogue_is_consistent() {
        for m in MODELS {
            assert_eq!(m.sha256.len(), 64);
            assert!(m.file.starts_with("ggml-") && m.file.ends_with(".bin"));
            assert!(model_url(m).starts_with("https://huggingface.co/"));
        }
        assert_eq!(model("base").unwrap().id, "base");
        assert!(matches!(model("../etc"), Err(VoiceError::UnknownModel(_))));
    }

    #[test]
    fn pcm_decodes_and_is_bounded() {
        let b = [0x00, 0x80, 0xff, 0x7f, 0, 0];
        let s = pcm16_to_f32(&b).unwrap();
        assert_eq!(s[0], -1.0);
        assert!((s[1] - 0.99997).abs() < 1e-4);
        assert_eq!(s[2], 0.0);
        assert!(matches!(pcm16_to_f32(&[1, 2, 3]), Err(VoiceError::OddPcm)));
        assert!(matches!(
            pcm16_to_f32(&vec![0; MAX_SAMPLES * 2 + 2]),
            Err(VoiceError::TooLong)
        ));
    }

    #[test]
    fn wav_parses_stereo_and_skips_chunks() {
        let w = wav(&[1000, 3000, -2000, -2000], 44_100, 2);
        let (s, rate) = parse_wav(&w).unwrap();
        assert_eq!(rate, 44_100);
        assert_eq!(s.len(), 2);
        assert!((s[0] - 2000.0 / 32768.0).abs() < 1e-6);
        assert!((s[1] + 2000.0 / 32768.0).abs() < 1e-6);
    }

    #[test]
    fn wav_rejects_garbage_without_panicking() {
        assert!(parse_wav(b"").is_err());
        assert!(parse_wav(b"RIFF\0\0\0\0WAVE").is_err());
        let mut w = wav(&[0; 4], 16_000, 1);
        w[16..20].copy_from_slice(&u32::MAX.to_le_bytes()); // absurd fmt size
        assert!(parse_wav(&w).is_err());
        let mut w = wav(&[0; 4], 16_000, 1);
        w[34] = 8; // 8-bit
        assert!(parse_wav(&w).is_err());
    }

    #[test]
    fn resampling_keeps_duration() {
        let s: Vec<f32> = (0..48_000).map(|i| (i as f32 / 10.0).sin()).collect();
        assert_eq!(resample_to_16k(&s, 48_000).len(), 16_000);
        assert_eq!(resample_to_16k(&s, 16_000).len(), 48_000);
        assert!(resample_to_16k(&[], 8_000).is_empty());
    }

    #[test]
    fn languages_map_to_whisper_codes() {
        assert_eq!(whisper_lang("bn-BD"), Some("bn"));
        assert_eq!(whisper_lang("en-US"), Some("en"));
        assert_eq!(whisper_lang("EN_gb"), Some("en"));
        assert_eq!(whisper_lang("xx"), None);
    }

    #[test]
    fn transcripts_are_cleaned() {
        assert_eq!(clean_transcript(" [BLANK_AUDIO] "), "");
        assert_eq!(
            clean_transcript(" Hello (music) world  *laughs* ."),
            "Hello world ."
        );
        assert_eq!(clean_transcript(" আমার সোনার বাংলা"), "আমার সোনার বাংলা");
    }

    #[test]
    fn store_paths_install_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::new(dir.path());
        let m = model("tiny").unwrap();
        assert!(!store.installed(m));
        fs::write(store.path(m), b"lmggxxxx").unwrap();
        assert!(!store.installed(m), "wrong size is not installed");
        fs::write(store.part_path(m), b"x").unwrap();
        store.remove(m).unwrap();
        assert!(!store.path(m).exists() && !store.part_path(m).exists());
        store.remove(m).unwrap(); // idempotent
    }

    /// Runs whisper on a synthetic tone with a real model when one is present
    /// (`RUPOK_WHISPER_MODEL=/path/ggml-tiny.bin`); skipped otherwise, so CI
    /// never downloads a model.
    #[cfg(feature = "whisper")]
    #[test]
    fn whisper_transcribes_when_a_model_is_available() {
        let engine = Engine::default();
        // Silence never reaches the model, so this runs without one.
        assert_eq!(
            engine
                .transcribe(Path::new("/nonexistent"), &vec![0.0; 16_000], Some("en"))
                .unwrap(),
            ""
        );
        let Some(path) = std::env::var_os("RUPOK_WHISPER_MODEL") else {
            eprintln!("skipping: set RUPOK_WHISPER_MODEL to a ggml model to run whisper");
            return;
        };
        let tone: Vec<i16> = (0..16_000)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
            .collect();
        let (s, rate) = parse_wav(&wav(&tone, 16_000, 1)).unwrap();
        let out = engine.transcribe(Path::new(&path), &resample_to_16k(&s, rate), Some("en"));
        assert!(out.is_ok(), "{out:?}");
    }

    #[cfg(feature = "whisper")]
    #[test]
    fn missing_model_is_an_error_not_a_panic() {
        let engine = Engine::default();
        let loud: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        assert!(
            engine
                .transcribe(Path::new("/nonexistent/ggml-tiny.bin"), &loud, Some("en"))
                .is_err()
        );
    }
}
