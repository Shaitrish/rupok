//! Tauri commands for offline voice input (`voice.rs`): model status,
//! download on demand (with consent in Settings), removal, and transcription
//! of 16 kHz mono PCM the board captured with getUserMedia.
//!
//! The commands exist in every desktop build so the capability file is the
//! same; without the `whisper` feature they report `built: false` and refuse.
//! Audio is never written to disk.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::voice::{self, ModelStore, VoiceError};

/// Progress of a model download, sent to the board.
#[cfg_attr(not(feature = "whisper"), allow(dead_code))]
const PROGRESS_EVENT: &str = "rupok://voice-model-progress";

#[derive(Default)]
pub struct VoiceState {
    downloading: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    #[cfg(feature = "whisper")]
    engine: Arc<voice::Engine>,
}

fn store<R: Runtime>(app: &AppHandle<R>) -> Result<ModelStore, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(ModelStore::new(dir.join("voice")))
}

#[derive(Serialize)]
pub struct ModelOut {
    id: &'static str,
    bytes: u64,
    installed: bool,
}

#[derive(Serialize)]
pub struct StatusOut {
    /// This build has whisper.cpp (feature `whisper`).
    built: bool,
    downloading: bool,
    models: Vec<ModelOut>,
}

#[tauri::command]
pub fn voice_status<R: Runtime>(
    app: AppHandle<R>,
    voice: State<'_, VoiceState>,
) -> Result<StatusOut, String> {
    let store = store(&app)?;
    Ok(StatusOut {
        built: voice::BUILT,
        downloading: voice.downloading.load(Ordering::Relaxed),
        models: voice::MODELS
            .iter()
            .map(|m| ModelOut {
                id: m.id,
                bytes: m.bytes,
                installed: store.installed(m),
            })
            .collect(),
    })
}

#[cfg_attr(not(feature = "whisper"), allow(dead_code))]
#[derive(Serialize, Clone)]
struct Progress {
    model: &'static str,
    done: u64,
    total: u64,
}

/// Download a model (the board asks for consent first and shows the size).
#[tauri::command]
pub async fn voice_model_download<R: Runtime>(
    app: AppHandle<R>,
    voice: State<'_, VoiceState>,
    model: String,
) -> Result<(), String> {
    let m = voice::model(&model).map_err(|e| e.to_string())?;
    #[cfg(not(feature = "whisper"))]
    {
        let _ = (&app, &voice, m);
        Err(VoiceError::NotBuilt.to_string())
    }
    #[cfg(feature = "whisper")]
    {
        use tauri::Emitter;
        if voice.downloading.swap(true, Ordering::SeqCst) {
            return Err("a voice model is already downloading".into());
        }
        voice.cancel.store(false, Ordering::SeqCst);
        let store = store(&app)?;
        let (cancel, busy) = (voice.cancel.clone(), voice.downloading.clone());
        let res = tauri::async_runtime::spawn_blocking(move || {
            let mut last = 0u64;
            let r = voice::download(&store, m, &cancel, |done, total| {
                // About every 1 MB, so the event channel stays quiet.
                if done - last >= 1 << 20 || done == total {
                    last = done;
                    let _ = app.emit(
                        PROGRESS_EVENT,
                        Progress {
                            model: m.id,
                            done,
                            total,
                        },
                    );
                }
            });
            busy.store(false, Ordering::SeqCst);
            r
        })
        .await
        .map_err(|e| e.to_string())?;
        res.map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn voice_model_cancel(voice: State<'_, VoiceState>) {
    voice.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn voice_model_remove<R: Runtime>(
    app: AppHandle<R>,
    voice: State<'_, VoiceState>,
    model: String,
) -> Result<(), String> {
    let m = voice::model(&model).map_err(|e| e.to_string())?;
    #[cfg(feature = "whisper")]
    voice.engine.unload();
    #[cfg(not(feature = "whisper"))]
    let _ = &voice;
    store(&app)?.remove(m).map_err(|e| e.to_string())
}

/// Transcribe the request body: little-endian 16-bit mono PCM at 16 kHz.
/// Headers: `x-voice-model` ("tiny" | "base" | "small"), `x-voice-lang`
/// (BCP 47, e.g. "bn-BD"; anything else auto-detects).
#[tauri::command]
pub async fn voice_transcribe<R: Runtime>(
    app: AppHandle<R>,
    voice: State<'_, VoiceState>,
    request: Request<'_>,
) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected PCM audio as the request body".into());
    };
    let samples = voice::pcm16_to_f32(bytes).map_err(|e| e.to_string())?;
    let get = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    let m = voice::model(&get("x-voice-model")).map_err(|e| e.to_string())?;
    let lang = voice::whisper_lang(&get("x-voice-lang"));
    let store = store(&app)?;
    if !store.installed(m) {
        return Err(VoiceError::ModelMissing.to_string());
    }
    #[cfg(not(feature = "whisper"))]
    {
        let _ = (&voice, samples, lang);
        Err(VoiceError::NotBuilt.to_string())
    }
    #[cfg(feature = "whisper")]
    {
        let engine = voice.engine.clone();
        let path = store.path(m);
        tauri::async_runtime::spawn_blocking(move || engine.transcribe(&path, &samples, lang))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }
}

/// Let the board window use the microphone. WebKitGTK (Linux) denies
/// getUserMedia unless the app enables media streams and answers the
/// permission request; only audio-only requests from the board are allowed.
/// WebView2 (Windows) and WKWebView (macOS) ask the user themselves.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "linux")]
    if let Some(w) = app.get_webview_window(crate::app::MAIN) {
        let _ = w.with_webview(|wv| {
            use webkit2gtk::glib::object::Cast;
            use webkit2gtk::{
                PermissionRequestExt, SettingsExt, UserMediaPermissionRequest,
                UserMediaPermissionRequestExt, WebViewExt,
            };
            let view = wv.inner();
            if let Some(s) = WebViewExt::settings(&view) {
                s.set_enable_media_stream(true);
            }
            view.connect_permission_request(|_, req| {
                match req.downcast_ref::<UserMediaPermissionRequest>() {
                    Some(um) if um.is_for_audio_device() && !um.is_for_video_device() => {
                        req.allow();
                        true
                    }
                    _ => false,
                }
            });
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}
