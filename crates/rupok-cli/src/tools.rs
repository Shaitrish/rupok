//! The MCP tool surface as plain, synchronous Rust: parameter types (with the
//! JSON Schema the agent sees) and a [`Tools`] struct whose methods sandbox
//! every path through the [`Workspace`] and then call [`crate::ops`].
//!
//! The rmcp layer in [`crate::mcp`] is a thin wrapper around this, and the
//! tests drive it directly.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rupok_core::Pattern;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ops::{self, OpError, Result};
use crate::workspace::Workspace;

// ------------------------------------------------------------------ params

/// Background pattern of a page.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[schemars(inline)]
pub enum PatternArg {
    Plain,
    Grid,
    Dots,
    Lines,
}

impl From<PatternArg> for Pattern {
    fn from(p: PatternArg) -> Self {
        match p {
            PatternArg::Plain => Pattern::Plain,
            PatternArg::Grid => Pattern::Grid,
            PatternArg::Dots => Pattern::Dots,
            PatternArg::Lines => Pattern::Lines,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLessonParams {
    /// Path of the new lesson file, relative to the workspace, ending in `.rpk`
    /// (e.g. `science/water-cycle.rpk`). Missing folders are created. Fails if
    /// the file already exists.
    pub path: String,
    /// Lesson title shown to the teacher and students.
    pub title: String,
    /// Number of empty 1920×1080 slide pages to start with. Default 1, max 200.
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub pages: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LessonPathParams {
    /// Lesson file path relative to the workspace (as returned by list_lessons),
    /// e.g. `science/water-cycle.rpk`. `get_lesson_outline` also takes `"live"`
    /// for the lesson open on the paired board.
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddPageParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// Optional page title (shown in the page sorter, not drawn on the slide).
    #[serde(default)]
    pub title: Option<String>,
    /// 1-based position the new page will occupy (1 = new first page).
    /// Omit to append after the last page.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub index: Option<u32>,
    /// Background pattern. Default `plain`.
    #[serde(default)]
    pub pattern: Option<PatternArg>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddSmartObjectParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// 1-based page number to place the object on.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// A complete, self-contained HTML document (inline CSS/JS). Load libraries
    /// with `rupoklib://<library>/<file>` URLs, never from CDNs. Follow
    /// read_object_spec.
    pub html: String,
    /// Short human-readable title, e.g. "Water cycle diagram". Also used to
    /// derive the bundle id.
    pub title: String,
    /// Object width in slide pixels (the iframe viewport width). Default 1280.
    /// Use 1920 (with height 1080) for a full-slide object.
    #[serde(default)]
    pub width: Option<f64>,
    /// Object height in slide pixels (the iframe viewport height). Default 720.
    #[serde(default)]
    pub height: Option<f64>,
    /// Number of build steps the presenter can click through inside the object
    /// (the object listens with `rupok.steps.onChange`). Default 0 = static.
    #[serde(default)]
    #[schemars(range(max = 500))]
    pub steps: Option<u32>,
    /// Left edge in slide pixels (0 = left of the 1920-wide slide). Default:
    /// centred horizontally.
    #[serde(default)]
    pub x: Option<f64>,
    /// Top edge in slide pixels (0 = top of the 1080-high slide). Default:
    /// centred vertically.
    #[serde(default)]
    pub y: Option<f64>,
    /// Plain hostnames the object may contact, e.g. `["api.open-meteo.com"]`.
    /// Only when live data is essential; lessons must work offline. Default:
    /// no network.
    #[serde(default)]
    pub network_hosts: Option<Vec<String>>,
}

/// Common options of a media item.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddMediaParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// 1-based page number.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// A YouTube link (watch, youtu.be, shorts, embed) or a page on a trusted
    /// education site: PhET, GeoGebra, Desmos, published Google Slides/Docs/
    /// Sheets/Forms, Wikipedia, Khan Academy. Other sites are refused. Give
    /// either `url` or `file`.
    #[serde(default)]
    pub url: Option<String>,
    /// A video or audio file in the workspace (mp4, webm, mp3, m4a, ogg,
    /// wav; at most 200 MB). It is stored inside the lesson, so it plays
    /// offline. Give either `url` or `file`.
    #[serde(default)]
    pub file: Option<String>,
    /// Title shown to the teacher and in the lesson outline. Default: from
    /// the link or the file name.
    #[serde(default)]
    pub title: Option<String>,
    /// Start playing at this second (also read from a YouTube `t=` link).
    #[serde(default)]
    pub start: Option<f64>,
    /// Stop at this second.
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub muted: Option<bool>,
    #[serde(default, rename = "loop")]
    pub looped: Option<bool>,
    /// Start playing when the page is shown.
    #[serde(default)]
    pub autoplay: Option<bool>,
    /// Left edge in slide pixels. Default: centred.
    #[serde(default)]
    pub x: Option<f64>,
    /// Top edge in slide pixels. Default: centred.
    #[serde(default)]
    pub y: Option<f64>,
    /// Width in slide pixels. Default 1280 (audio 800).
    #[serde(default)]
    pub width: Option<f64>,
    /// Height in slide pixels. Default 720 (audio 450).
    #[serde(default)]
    pub height: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddYoutubeParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board.
    pub path: String,
    /// 1-based page number.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// The YouTube link (watch, youtu.be, shorts or embed; `t=` sets the start).
    pub url: String,
    /// Title. Default: "YouTube video" (the board looks up the real title).
    #[serde(default)]
    pub title: Option<String>,
    /// Start at this second.
    #[serde(default)]
    pub start: Option<f64>,
    /// Stop at this second.
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub muted: Option<bool>,
    #[serde(default, rename = "loop")]
    pub looped: Option<bool>,
    /// Start playing when the page is shown.
    #[serde(default)]
    pub autoplay: Option<bool>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// Width in slide pixels. Default 1280.
    #[serde(default)]
    pub width: Option<f64>,
    /// Height in slide pixels. Default 720.
    #[serde(default)]
    pub height: Option<f64>,
}

impl From<AddYoutubeParams> for AddMediaParams {
    fn from(p: AddYoutubeParams) -> Self {
        Self {
            path: p.path,
            page: p.page,
            url: Some(p.url),
            file: None,
            title: p.title,
            start: p.start,
            end: p.end,
            muted: p.muted,
            looped: p.looped,
            autoplay: p.autoplay,
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddModelParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board.
    pub path: String,
    /// 1-based page number.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// A glTF 2.0 model in the workspace: `.glb`, or `.gltf` with every
    /// buffer and texture embedded. At most 18 MB.
    pub file: String,
    /// Title. Default: from the file name.
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// Width in slide pixels. Default 1280.
    #[serde(default)]
    pub width: Option<f64>,
    /// Height in slide pixels. Default 720.
    #[serde(default)]
    pub height: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateSmartObjectParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// Bundle id of the object (from add_smart_object or get_lesson_outline),
    /// e.g. `water-cycle-diagram-k3x9`.
    pub bundle_id: String,
    /// The full replacement HTML document.
    pub html: String,
    /// New number of build steps. Omit to keep the current value.
    #[serde(default)]
    #[schemars(range(max = 500))]
    pub steps: Option<u32>,
    /// Replacement list of allowed network hostnames. Omit to keep the current
    /// list; pass `[]` to remove network access.
    #[serde(default)]
    pub network_hosts: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddTextParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// 1-based page number.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// The text. Use `\n` for line breaks.
    pub text: String,
    /// Left edge in slide pixels (the slide is 1920 wide).
    pub x: f64,
    /// Top edge in slide pixels (the slide is 1080 high).
    pub y: f64,
    /// Box width in slide pixels; long lines wrap. Default: fits the longest line.
    #[serde(default)]
    pub w: Option<f64>,
    /// Font size in pixels. Default 32. Keep it 24 or larger for projectors.
    #[serde(default)]
    pub size: Option<f64>,
    /// Text colour as `#rrggbb`. Default `#000000`.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceStrokesWithTextParams {
    /// Lesson file path relative to the workspace.
    pub path: String,
    /// 1-based page number the strokes are on.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// Ids of the handwritten ink strokes to replace (from get_lesson_outline).
    pub stroke_ids: Vec<String>,
    /// What the handwriting says. Use `\n` for line breaks; write maths as
    /// plain Unicode (x² + 3x − 4 = 0).
    pub text: String,
    /// Font size in pixels. Default: matches the handwriting's line height.
    #[serde(default)]
    pub size: Option<f64>,
    /// Text colour as `#rrggbb`. Default: the ink's colour.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteItemParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// 1-based page number the item is on.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// Item id from get_lesson_outline.
    pub item_id: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPageBackgroundParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board (the class sees the change at once).
    pub path: String,
    /// 1-based page number.
    #[schemars(range(min = 1))]
    pub page: u32,
    /// Background pattern.
    pub pattern: PatternArg,
    /// Background colour as `#rrggbb`. Omit to keep the current colour.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPageDesignParams {
    /// Lesson file path relative to the workspace, or `"live"` for the lesson
    /// open on the paired board.
    pub path: String,
    /// 1-based page number. Default: the page the board shows (live) or page 1 (file).
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
    /// Give every page this page's resulting look (colour, pattern, background).
    #[serde(default)]
    pub all_pages: bool,
    /// Page colour: white, paper, chalkboard, blackboard or #rrggbb.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub pattern: Option<PatternArg>,
    /// Background as SVG markup, drawn for the 1920×1080 page (use viewBox="0 0 1920 1080").
    /// No scripts, event handlers, external references or animation.
    #[serde(default)]
    pub background_svg: Option<String>,
    /// Background as PNG or JPEG bytes, base64.
    #[serde(default)]
    pub background_image: Option<String>,
    /// fit, fill, stretch, tile or center. Default: stretch for SVG, fill for photos.
    #[serde(default)]
    pub background_fit: Option<String>,
    /// 0 to 1.
    #[serde(default)]
    pub background_opacity: Option<f64>,
    /// Wash the image with the page colour (50 %) so ink stays readable.
    #[serde(default)]
    pub background_dim: Option<bool>,
    #[serde(default)]
    pub remove_background: bool,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetBrandingParams {
    /// Lesson file path relative to the workspace, or `"live"`.
    pub path: String,
    /// 1-based page for `scope: "page"` and `hidden`. Default: the shown page (live) or page 1.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
    /// `lesson` (every page, default) or `page` (this page's own branding).
    #[serde(default)]
    pub scope: Option<String>,
    /// Logo as SVG markup (no scripts, external references or animation).
    #[serde(default)]
    pub logo_svg: Option<String>,
    /// Logo as PNG or JPEG bytes, base64.
    #[serde(default)]
    pub logo_image: Option<String>,
    /// top-left, top-right, bottom-left or bottom-right.
    #[serde(default)]
    pub logo_corner: Option<String>,
    /// Logo top-left as fractions (0..1) of the page width and height.
    #[serde(default)]
    pub logo_x: Option<f64>,
    #[serde(default)]
    pub logo_y: Option<f64>,
    /// Logo width as a fraction of the page width, 0.02 to 0.6 (0.08–0.12 is a small corner logo).
    #[serde(default)]
    pub logo_width: Option<f64>,
    #[serde(default)]
    pub logo_opacity: Option<f64>,
    #[serde(default)]
    pub remove_logo: bool,
    /// Header/footer text parts (each up to 120 characters). They may use placeholders that
    /// fill themselves in: {lesson}, {page}, {pages}, {date}, {teacher}, {school}, {subject},
    /// {course}, {chapter}.
    #[serde(default)]
    pub school: Option<String>,
    #[serde(default)]
    pub teacher: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    /// Show the day's date.
    #[serde(default)]
    pub date: Option<bool>,
    /// top (header) or bottom (footer).
    #[serde(default)]
    pub text_position: Option<String>,
    /// left, center or right.
    #[serde(default)]
    pub text_align: Option<String>,
    /// Hide (true) or show again (false) the branding on `page` (a title slide).
    #[serde(default)]
    pub hidden: Option<bool>,
    /// Remove the branding (the lesson's, or with scope page the page's own).
    #[serde(default)]
    pub remove: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateSmartObjectParams {
    /// The HTML document to check.
    pub html: String,
    /// Number of build steps the object will declare. Default 0.
    #[serde(default)]
    pub steps: Option<u32>,
    /// Hostnames the object would be allowed to contact. Default: none.
    #[serde(default)]
    pub network_hosts: Option<Vec<String>>,
    /// Width the object will be placed at. Default 1280.
    #[serde(default)]
    pub width: Option<f64>,
    /// Height the object will be placed at. Default 720.
    #[serde(default)]
    pub height: Option<f64>,
}

// ------------------------------------------------------------------ results

#[derive(Clone, Debug, Serialize)]
pub struct LessonList {
    pub workspace: String,
    pub lessons: Vec<ops::LessonListing>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectSpec {
    /// File the spec was read from, or `embedded` for the built-in summary.
    pub source: String,
    pub text: String,
}

/// JSON body returned to the agent when a tool fails.
pub fn error_json(e: &OpError) -> Value {
    match e {
        OpError::Diagnostics(d) => json!({
            "error": e.to_string(),
            "saved": false,
            "diagnostics": d,
        }),
        _ => json!({ "error": e.to_string() }),
    }
}

// ------------------------------------------------------------------ tools

/// The tool implementations, sandboxed to one workspace directory.
#[derive(Debug)]
pub struct Tools {
    ws: Workspace,
    /// Serialises read-modify-write cycles so concurrent calls cannot lose edits.
    lock: Mutex<()>,
}

impl Tools {
    pub fn new(ws: Workspace) -> Self {
        Self {
            ws,
            lock: Mutex::new(()),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn lesson(&self, path: &str) -> Result<PathBuf> {
        Ok(self.ws.resolve_lesson(path)?)
    }

    pub fn list_lessons(&self) -> Result<LessonList> {
        Ok(LessonList {
            workspace: self.ws.root().display().to_string(),
            lessons: ops::list_lessons(self.ws.root()),
        })
    }

    pub fn create_lesson(&self, p: CreateLessonParams) -> Result<ops::CreatedLesson> {
        let _g = self.guard();
        let abs = self.ws.resolve_new_lesson(&p.path)?;
        let mut out = ops::create_lesson(&abs, &p.title, p.pages.unwrap_or(1))?;
        out.path = self.ws.display(&abs);
        Ok(out)
    }

    pub fn get_lesson_outline(&self, p: LessonPathParams) -> Result<ops::LessonOutline> {
        let _g = self.guard();
        ops::outline(&self.lesson(&p.path)?)
    }

    pub fn add_page(&self, p: AddPageParams) -> Result<ops::AddedPage> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        ops::add_page(
            &abs,
            ops::NewPage {
                title: p.title,
                index: p.index.map(|i| i as usize),
                pattern: p.pattern.map(Into::into),
            },
        )
    }

    /// Read a media or model file from the workspace (never outside it).
    pub fn read_workspace_file(&self, user: &str, max: usize) -> Result<(Vec<u8>, String)> {
        let abs = self.ws.resolve_existing(user)?;
        let len = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        if len > max as u64 {
            return Err(OpError::Invalid(format!(
                "{user} is {:.1} MB; the limit is {} MB",
                len as f64 / 1_048_576.0,
                max >> 20
            )));
        }
        let bytes = std::fs::read(&abs).map_err(|err| OpError::Io {
            path: user.to_string(),
            err,
        })?;
        let name = abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((bytes, name))
    }

    /// The media request of `p`, with the file read from the workspace.
    pub fn media_input(&self, p: &AddMediaParams) -> Result<rupok_format::media::MediaInput> {
        let (file, file_name) = match &p.file {
            Some(f) => {
                let (bytes, name) =
                    self.read_workspace_file(f, rupok_format::media::MAX_MEDIA_BYTES)?;
                (Some(bytes), Some(name))
            }
            None => (None, None),
        };
        Ok(rupok_format::media::MediaInput {
            url: p.url.clone(),
            file,
            file_name,
            title: p.title.clone().filter(|t| !t.trim().is_empty()),
            start: p.start,
            end: p.end,
            muted: p.muted.unwrap_or(false),
            looped: p.looped.unwrap_or(false),
            autoplay: p.autoplay.unwrap_or(false),
        })
    }

    pub fn add_media(&self, p: AddMediaParams) -> Result<ops::AddedMedia> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let input = self.media_input(&p)?;
        ops::add_media(
            &abs,
            input,
            ops::Placement {
                page: Some(p.page as usize),
                x: p.x,
                y: p.y,
                w: p.width,
                h: p.height,
            },
        )
    }

    pub fn add_model(&self, p: AddModelParams) -> Result<ops::AddedObject> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let (bytes, name) =
            self.read_workspace_file(&p.file, rupok_format::model3d::MAX_MODEL_BYTES)?;
        ops::add_model(
            &abs,
            &bytes,
            &name,
            p.title.as_deref().unwrap_or(""),
            ops::Placement {
                page: Some(p.page as usize),
                x: p.x,
                y: p.y,
                w: p.width,
                h: p.height,
            },
        )
    }

    pub fn add_smart_object(&self, p: AddSmartObjectParams) -> Result<ops::AddedObject> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let w = p.width.unwrap_or(ops::DEFAULT_OBJECT_W);
        let h = p.height.unwrap_or(ops::DEFAULT_OBJECT_H);
        let bundle = ops::html_bundle(
            &p.html,
            &p.title,
            w,
            h,
            p.steps.unwrap_or(0),
            p.network_hosts.unwrap_or_default(),
        );
        ops::add_object(
            &abs,
            bundle,
            ops::Placement {
                page: Some(p.page as usize),
                x: p.x,
                y: p.y,
                w: Some(w),
                h: Some(h),
            },
        )
    }

    pub fn update_smart_object(&self, p: UpdateSmartObjectParams) -> Result<ops::UpdatedObject> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        ops::update_object_html(&abs, &p.bundle_id, &p.html, p.steps, p.network_hosts)
    }

    pub fn add_text(&self, p: AddTextParams) -> Result<ops::AddedText> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let color = p.color.as_deref().map(ops::parse_color).transpose()?;
        ops::add_text(
            &abs,
            ops::NewText {
                page: p.page as usize,
                text: p.text,
                x: p.x,
                y: p.y,
                w: p.w,
                size: p.size,
                color,
            },
        )
    }

    pub fn replace_strokes_with_text(
        &self,
        p: ReplaceStrokesWithTextParams,
    ) -> Result<ops::ReplacedWithText> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let color = p.color.as_deref().map(ops::parse_color).transpose()?;
        ops::replace_strokes_with_text(&abs, p.page as usize, &p.stroke_ids, &p.text, p.size, color)
    }

    pub fn delete_item(&self, p: DeleteItemParams) -> Result<ops::DeletedItem> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        ops::delete_item(&abs, p.page as usize, &p.item_id)
    }

    pub fn set_page_background(&self, p: SetPageBackgroundParams) -> Result<ops::PageBackground> {
        let _g = self.guard();
        let abs = self.lesson(&p.path)?;
        let color = p.color.as_deref().map(ops::parse_color).transpose()?;
        ops::set_page_background(&abs, p.page as usize, p.pattern.into(), color)
    }

    /// Run a page design op (`set_page_design`, `set_branding`) on a lesson
    /// file: the same code the live board runs, saved when it changed.
    pub fn design_op(&self, path: &str, op: rupok_live::Op) -> Result<serde_json::Value> {
        self.doc_op(path, op)
    }

    /// Run a live operation (pages, items, stamps, page design…) on a lesson
    /// file with exactly the code the board runs, and save the file when it
    /// changed. "The current page" is page 1. Board-app operations (modes,
    /// the view, the library, settings) are refused with a clear error.
    pub fn doc_op(&self, path: &str, op: rupok_live::Op) -> Result<serde_json::Value> {
        if op.board_only() {
            return Err(ops::OpError::Invalid(format!(
                "`{}` works only on the live board: pass path \"live\" (a board must be paired)",
                op.name()
            )));
        }
        let _g = self.guard();
        let abs = self.lesson(path)?;
        let (mut pkg, mut lesson) = ops::load(&abs)?;
        let page_id = lesson.page_id_at(0)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
        let ctx = rupok_live::BoardCtx {
            pkg: &mut pkg,
            lesson: &mut lesson,
            page_id: &page_id,
            selection: &[],
            read_only: false,
        };
        let out = rupok_live::apply(ctx, &op, "agent", now).map_err(|e| match e {
            rupok_live::LiveError::NotFound(m) => ops::OpError::NotFound(m),
            rupok_live::LiveError::Diagnostics(d) => ops::OpError::Diagnostics(d),
            rupok_live::LiveError::Unsupported(name) => ops::OpError::Invalid(format!(
                "`{name}` works only on the live board: pass path \"live\" (a board must be paired)"
            )),
            e => ops::OpError::Invalid(e.to_string()),
        })?;
        if out.changed {
            ops::save(&abs, &mut pkg, &lesson)?;
        }
        Ok(out.result)
    }

    pub fn validate_smart_object(
        &self,
        p: ValidateSmartObjectParams,
    ) -> Result<ops::ValidationReport> {
        let title = ops::html_title(&p.html).unwrap_or_else(|| "Smart Object".into());
        let b = ops::html_bundle(
            &p.html,
            &title,
            p.width.unwrap_or(ops::DEFAULT_OBJECT_W),
            p.height.unwrap_or(ops::DEFAULT_OBJECT_H),
            p.steps.unwrap_or(0),
            p.network_hosts.unwrap_or_default(),
        );
        Ok(ops::ValidationReport::new(b.validate()))
    }

    pub fn read_object_spec(&self) -> ObjectSpec {
        let (src, text) = crate::spec::object_spec(self.ws.root());
        ObjectSpec {
            source: src
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "embedded".into()),
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::PathError;

    const VALID_HTML: &str = r#"<!doctype html><html><head><title>Fraction bars</title>
<style>body{margin:0;font:32px system-ui}</style></head><body>
<div id="bars"></div>
<script type="module">
import * as d3 from "rupoklib://d3/d3.min.js";
rupok.steps.onChange(step => { document.getElementById("bars").textContent = "Step " + step; });
</script></body></html>"#;

    const CDN_HTML: &str = r#"<!doctype html><script src="https://cdn.jsdelivr.net/npm/chart.js"></script><canvas></canvas>"#;

    fn tools() -> (tempfile::TempDir, Tools) {
        let dir = tempfile::tempdir().unwrap();
        let t = Tools::new(Workspace::new(dir.path()).unwrap());
        (dir, t)
    }

    fn lp(path: &str) -> LessonPathParams {
        LessonPathParams { path: path.into() }
    }

    fn smart(path: &str, page: u32, html: &str) -> AddSmartObjectParams {
        AddSmartObjectParams {
            path: path.into(),
            page,
            html: html.into(),
            title: "Fraction bars".into(),
            width: None,
            height: None,
            steps: Some(3),
            x: None,
            y: None,
            network_hosts: None,
        }
    }

    #[test]
    fn full_lesson_workflow() {
        let (dir, t) = tools();
        let path = "maths/fractions.rpk";

        // create
        let created = t
            .create_lesson(CreateLessonParams {
                path: path.into(),
                title: "Fractions".into(),
                pages: Some(2),
            })
            .unwrap();
        assert_eq!(created.path, path);
        assert_eq!(created.page_ids.len(), 2);
        assert!(dir.path().join(path).is_file());
        let o = t.get_lesson_outline(lp(path)).unwrap();
        assert_eq!(o.title, "Fractions");
        assert_eq!(o.page_count, 2);
        assert!(o.pages.iter().all(|p| p.items.is_empty()));

        // refuses to overwrite
        assert!(
            t.create_lesson(CreateLessonParams {
                path: path.into(),
                title: "X".into(),
                pages: None
            })
            .is_err()
        );

        // add page (inserted as page 1)
        let added = t
            .add_page(AddPageParams {
                path: path.into(),
                title: Some("Warm-up".into()),
                index: Some(1),
                pattern: Some(PatternArg::Grid),
            })
            .unwrap();
        assert_eq!((added.page, added.page_count), (1, 3));
        let o = t.get_lesson_outline(lp(path)).unwrap();
        assert_eq!(o.page_count, 3);
        assert_eq!(o.pages[0].title, "Warm-up");
        assert_eq!(o.pages[0].pattern, "grid");
        assert_eq!(o.pages[0].id, added.page_id);

        // add a valid smart object on page 2, centred by default
        let obj = t.add_smart_object(smart(path, 2, VALID_HTML)).unwrap();
        assert!(
            obj.bundle_id.starts_with("fraction-bars-"),
            "{}",
            obj.bundle_id
        );
        assert_eq!(
            obj.geometry,
            ops::Geometry {
                x: 320.0,
                y: 180.0,
                w: 1280.0,
                h: 720.0,
                rot: 0.0
            }
        );
        assert!(
            obj.warnings
                .iter()
                .all(|d| d.severity == rupok_format::Severity::Warning)
        );
        let o = t.get_lesson_outline(lp(path)).unwrap();
        let items = &o.pages[1].items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, obj.item_id);
        assert_eq!(items[0].kind, "object");
        assert_eq!(items[0].bundle_id.as_deref(), Some(obj.bundle_id.as_str()));
        assert_eq!(items[0].steps, Some(3));
        assert_eq!(items[0].title.as_deref(), Some("Fraction bars"));

        // an invalid object (CDN script, no permission) is refused and nothing changes
        let before = std::fs::read(dir.path().join(path)).unwrap();
        let err = t.add_smart_object(smart(path, 2, CDN_HTML)).unwrap_err();
        let OpError::Diagnostics(d) = &err else {
            panic!("expected diagnostics, got {err:?}")
        };
        assert!(
            d.iter()
                .any(|d| d.code == "network" && d.severity == rupok_format::Severity::Error)
        );
        assert_eq!(
            std::fs::read(dir.path().join(path)).unwrap(),
            before,
            "lesson must be untouched"
        );
        let ej = error_json(&err);
        assert_eq!(ej["saved"], false);
        assert_eq!(ej["diagnostics"][0]["code"], "network");
        // ...but allowed once the host is permitted
        let mut with_host = smart(path, 2, CDN_HTML);
        with_host.network_hosts = Some(vec!["cdn.jsdelivr.net".into()]);
        with_host.steps = None;
        let cdn_obj = t.add_smart_object(with_host).unwrap();
        t.delete_item(DeleteItemParams {
            path: path.into(),
            page: 2,
            item_id: cdn_obj.item_id,
        })
        .unwrap();

        // update: fewer steps, new html; invalid update is refused
        let upd_html = VALID_HTML.replace("Step ", "Stage ");
        let err = t
            .update_smart_object(UpdateSmartObjectParams {
                path: path.into(),
                bundle_id: obj.bundle_id.clone(),
                html: CDN_HTML.into(),
                steps: None,
                network_hosts: None,
            })
            .unwrap_err();
        assert!(matches!(err, OpError::Diagnostics(_)));
        let upd = t
            .update_smart_object(UpdateSmartObjectParams {
                path: path.into(),
                bundle_id: obj.bundle_id.clone(),
                html: upd_html.clone(),
                steps: Some(2),
                network_hosts: None,
            })
            .unwrap();
        assert_eq!((upd.steps, upd.placements), (2, 1));
        assert_eq!(upd.libs, vec!["d3".to_string()]);
        let o = t.get_lesson_outline(lp(path)).unwrap();
        assert_eq!(o.pages[1].items[0].steps, Some(2));
        let pkg = ops::read_package(&dir.path().join(path)).unwrap();
        assert_eq!(pkg.objects[&obj.bundle_id].entry_html().unwrap(), upd_html);
        assert!(
            t.update_smart_object(UpdateSmartObjectParams {
                path: path.into(),
                bundle_id: "nope".into(),
                html: VALID_HTML.into(),
                steps: None,
                network_hosts: None,
            })
            .is_err()
        );

        // add text, then delete it
        let txt = t
            .add_text(AddTextParams {
                path: path.into(),
                page: 2,
                text: "Halves and quarters".into(),
                x: 100.0,
                y: 60.0,
                w: None,
                size: Some(48.0),
                color: Some("#1a73e8".into()),
            })
            .unwrap();
        assert!(txt.warnings.is_empty(), "{:?}", txt.warnings);
        let o = t.get_lesson_outline(lp(path)).unwrap();
        let text_item = o.pages[1]
            .items
            .iter()
            .find(|i| i.id == txt.item_id)
            .unwrap();
        assert_eq!(text_item.kind, "text");
        assert_eq!(text_item.text.as_deref(), Some("Halves and quarters"));
        assert_eq!(text_item.font_size, Some(48.0));
        assert_eq!(text_item.color.as_deref(), Some("#1a73e8"));
        assert_eq!(o.pages[1].items.len(), 2);

        let small = t
            .add_text(AddTextParams {
                path: path.into(),
                page: 3,
                text: "tiny".into(),
                x: 0.0,
                y: 0.0,
                w: None,
                size: Some(12.0),
                color: None,
            })
            .unwrap();
        assert!(small.warnings.iter().any(|d| d.code == "font_size"));

        let del = t
            .delete_item(DeleteItemParams {
                path: path.into(),
                page: 2,
                item_id: txt.item_id.clone(),
            })
            .unwrap();
        assert_eq!(del.kind, "text");
        let o = t.get_lesson_outline(lp(path)).unwrap();
        assert_eq!(o.pages[1].items.len(), 1);
        assert!(o.pages[1].items.iter().all(|i| i.id != txt.item_id));
        assert!(matches!(
            t.delete_item(DeleteItemParams {
                path: path.into(),
                page: 2,
                item_id: txt.item_id
            }),
            Err(OpError::NotFound(_))
        ));

        // background
        let bg = t
            .set_page_background(SetPageBackgroundParams {
                path: path.into(),
                page: 3,
                pattern: PatternArg::Dots,
                color: Some("#fffbe6".into()),
            })
            .unwrap();
        assert_eq!(bg.background, "#fffbe6");
        let o = t.get_lesson_outline(lp(path)).unwrap();
        assert_eq!(
            (o.pages[2].pattern, o.pages[2].background.as_str()),
            ("dots", "#fffbe6")
        );

        // bad page numbers are friendly errors
        assert!(matches!(
            t.add_smart_object(smart(path, 9, VALID_HTML)),
            Err(OpError::Invalid(_))
        ));
        assert!(matches!(
            t.add_smart_object(smart(path, 0, VALID_HTML)),
            Err(OpError::Invalid(_))
        ));

        // list + info
        let list = t.list_lessons().unwrap();
        assert_eq!(list.lessons.len(), 1);
        assert_eq!(list.lessons[0].path, path);
        assert_eq!(list.lessons[0].title.as_deref(), Some("Fractions"));
        assert_eq!(list.lessons[0].pages, Some(3));
        let info = ops::info(&dir.path().join(path)).unwrap();
        assert_eq!(info.page_count, 3);
        assert_eq!(info.pages[1].items.object, 1);
        // two bundles: the object and the (deleted, retained for replay) CDN one
        assert_eq!(info.objects.len(), 2);
        assert!(info.objects.iter().all(|o| o.valid));
        assert_eq!(
            info.objects
                .iter()
                .find(|o| o.id == obj.bundle_id)
                .unwrap()
                .placements,
            1
        );

        // the saved package is still a valid lesson with our generator
        let pkg = ops::read_package(&dir.path().join(path)).unwrap();
        assert_eq!(pkg.manifest.unwrap().generator, ops::GENERATOR);
    }

    #[test]
    fn tools_refuse_paths_outside_the_workspace() {
        let (_dir, t) = tools();
        let r = t.create_lesson(CreateLessonParams {
            path: "../escape.rpk".into(),
            title: "x".into(),
            pages: None,
        });
        assert!(matches!(r, Err(OpError::Path(PathError::Traversal(_)))));
        let r = t.create_lesson(CreateLessonParams {
            path: "/tmp/escape.rpk".into(),
            title: "x".into(),
            pages: None,
        });
        assert!(matches!(r, Err(OpError::Path(_))));
        let r = t.create_lesson(CreateLessonParams {
            path: "notes.md".into(),
            title: "x".into(),
            pages: None,
        });
        assert!(matches!(r, Err(OpError::Path(PathError::NotLesson(_)))));
        assert!(t.get_lesson_outline(lp("missing.rpk")).is_err());
    }

    #[test]
    fn replace_strokes_with_text_swaps_ink_for_one_text_box() {
        use rupok_core::{ItemData, StrokeData};
        let (dir, t) = tools();
        t.create_lesson(CreateLessonParams {
            path: "hw.rpk".into(),
            title: "Handwriting".into(),
            pages: None,
        })
        .unwrap();
        let abs = dir.path().join("hw.rpk");
        let (mut pkg, mut lesson) = ops::load(&abs).unwrap();
        let page = lesson.page_id_at(0).unwrap();
        let stroke = |x0: f32| {
            ItemData::Stroke(StrokeData {
                tool: "pen".into(),
                color: 0x3e63ddff,
                width: 4.0,
                opacity: 1.0,
                points: (0..10)
                    .map(|i| rupok_ink::StrokePoint {
                        x: x0 + i as f32 * 5.0,
                        y: 200.0 + (i % 2) as f32 * 60.0,
                        p: 0.5,
                        dt: i * 8,
                    })
                    .collect(),
                offset: [0.0, 0.0],
                shape: None,
                geom: None,
                brush: None,
            })
        };
        let a = lesson.add_item(&page, &stroke(100.0), 1.0).unwrap();
        let b = lesson.add_item(&page, &stroke(200.0), 2.0).unwrap();
        ops::save(&abs, &mut pkg, &lesson).unwrap();
        let params = |ids: Vec<String>, text: &str| ReplaceStrokesWithTextParams {
            path: "hw.rpk".into(),
            page: 1,
            stroke_ids: ids,
            text: text.into(),
            size: None,
            color: None,
        };
        let r = t
            .replace_strokes_with_text(params(vec![a.clone(), b.clone()], "Photosynthesis"))
            .unwrap();
        assert_eq!(r.removed, 2);
        let (_, lesson) = ops::load(&abs).unwrap();
        let items = lesson.items(&page).unwrap();
        assert_eq!(items.len(), 1);
        let ItemData::Text(text) = &items[0].data else {
            panic!("not text")
        };
        assert_eq!(
            (text.text.as_str(), text.color),
            ("Photosynthesis", 0x3e63ddff)
        );
        assert!(text.size > 40.0, "sized to the handwriting: {}", text.size);
        // One change in the history: the ink before it is still there.
        let before = lesson.loro().oplog_vv();
        assert!(!before.is_empty());
        // Bad input never saves anything.
        assert!(t.replace_strokes_with_text(params(vec![], "x")).is_err());
        assert!(
            t.replace_strokes_with_text(params(vec!["nope".into()], "x"))
                .is_err()
        );
        assert!(
            t.replace_strokes_with_text(params(vec![r.item_id.clone()], "x"))
                .is_err()
        );
        assert!(t.replace_strokes_with_text(params(vec![a], "  ")).is_err());
    }

    #[test]
    fn validate_and_spec_tools() {
        let (_dir, t) = tools();
        let ok = t
            .validate_smart_object(ValidateSmartObjectParams {
                html: VALID_HTML.into(),
                steps: Some(3),
                network_hosts: None,
                width: None,
                height: None,
            })
            .unwrap();
        assert!(ok.valid, "{:?}", ok.diagnostics);
        let bad = t
            .validate_smart_object(ValidateSmartObjectParams {
                html: CDN_HTML.into(),
                steps: None,
                network_hosts: None,
                width: None,
                height: None,
            })
            .unwrap();
        assert!(!bad.valid && bad.errors >= 1);
        let spec = t.read_object_spec();
        assert!(spec.text.contains("rupoklib://"));
    }

    #[test]
    fn media_and_model_tools() {
        let (dir, t) = tools();
        let path = "bio.rpk";
        t.create_lesson(CreateLessonParams {
            path: path.into(),
            title: "Bio".into(),
            pages: Some(1),
        })
        .unwrap();
        let yt = t
            .add_media(
                AddYoutubeParams {
                    path: path.into(),
                    page: 1,
                    url: "https://youtu.be/dQw4w9WgXcQ?t=30".into(),
                    title: Some("Cells".into()),
                    start: None,
                    end: Some(90.0),
                    muted: None,
                    looped: None,
                    autoplay: Some(true),
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                }
                .into(),
            )
            .unwrap();
        assert_eq!(
            (yt.kind.as_str(), yt.src.as_str()),
            ("youtube", "https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        );
        let mut mp4 = vec![0, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypisom-video");
        std::fs::write(dir.path().join("clip.mp4"), &mp4).unwrap();
        let clip = t
            .add_media(AddMediaParams {
                path: path.into(),
                page: 1,
                file: Some("clip.mp4".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!((clip.kind.as_str(), clip.title.as_str()), ("video", "Clip"));
        let bad = t.add_media(AddMediaParams {
            path: path.into(),
            page: 1,
            url: Some("https://example.com/x".into()),
            ..Default::default()
        });
        assert!(bad.is_err());
        // Files outside the workspace are refused.
        assert!(
            t.add_media(AddMediaParams {
                path: path.into(),
                page: 1,
                file: Some("../x.mp4".into()),
                ..Default::default()
            })
            .is_err()
        );
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&[0; 12]);
        std::fs::write(dir.path().join("heart.glb"), &glb).unwrap();
        let m = t
            .add_model(AddModelParams {
                path: path.into(),
                page: 1,
                file: "heart.glb".into(),
                title: None,
                x: None,
                y: None,
                width: None,
                height: None,
            })
            .unwrap();
        assert!(m.bundle_id.starts_with("heart"));
        let o = t.get_lesson_outline(lp(path)).unwrap();
        let items = &o.pages[0].items;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].media.as_ref().unwrap().end, Some(90.0));
        assert_eq!(items[0].title.as_deref(), Some("Cells"));
        assert!(items[1].media.as_ref().unwrap().src.ends_with(".mp4"));
        assert_eq!(items[2].title.as_deref(), Some("Heart"));
    }

    #[test]
    fn param_schemas_are_objects_with_descriptions() {
        let s = serde_json::to_value(schemars::schema_for!(AddSmartObjectParams)).unwrap();
        assert_eq!(s["type"], "object");
        assert!(
            s["properties"]["html"]["description"]
                .as_str()
                .unwrap()
                .contains("rupoklib://")
        );
        let req: Vec<&str> = s["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            req.contains(&"path")
                && req.contains(&"page")
                && req.contains(&"html")
                && req.contains(&"title")
        );
        assert!(!req.contains(&"width"));
        let bg = serde_json::to_value(schemars::schema_for!(SetPageBackgroundParams)).unwrap();
        let pat = bg["properties"]["pattern"].to_string();
        assert!(pat.contains("grid") && pat.contains("dots"), "{pat}");
    }

    #[test]
    fn page_design_and_branding_on_a_file() {
        let (dir, t) = tools();
        t.create_lesson(CreateLessonParams {
            path: "d.rpk".into(),
            title: "Forces".into(),
            pages: Some(3),
        })
        .unwrap();
        let svg = r##"<svg viewBox="0 0 1920 1080"><rect width="1920" height="1080" fill="#17191c"/></svg>"##;
        let op = crate::mcp_live::op_set_page_design(&SetPageDesignParams {
            path: "d.rpk".into(),
            all_pages: true,
            color: Some("blackboard".into()),
            background_svg: Some(svg.into()),
            ..Default::default()
        });
        let out = t.design_op("d.rpk", op).unwrap();
        assert_eq!(out["pages_changed"], 3);
        let op = crate::mcp_live::op_set_branding(&SetBrandingParams {
            path: "d.rpk".into(),
            logo_svg: Some(
                r#"<svg viewBox="0 0 10 10"><circle cx="5" cy="5" r="5"/></svg>"#.into(),
            ),
            logo_corner: Some("bottom-right".into()),
            class: Some("Physics · {lesson} · {page}/{pages}".into()),
            ..Default::default()
        });
        t.design_op("d.rpk", op).unwrap();
        let bytes = std::fs::read(dir.path().join("d.rpk")).unwrap();
        let pkg = rupok_format::Package::read(&bytes).unwrap();
        let lesson = pkg.lesson().unwrap();
        assert!(
            lesson
                .pages()
                .iter()
                .all(|p| p.bg_color == 0x17191cff && p.background.is_some())
        );
        let b = lesson.branding().unwrap();
        assert_eq!(b.class, "Physics · {lesson} · {page}/{pages}");
        assert_eq!(pkg.assets.len(), 2);
        // Unsafe SVG: refused, the file unchanged.
        let op = crate::mcp_live::op_set_page_design(&SetPageDesignParams {
            path: "d.rpk".into(),
            background_svg: Some("<svg><script>x</script></svg>".into()),
            ..Default::default()
        });
        assert!(t.design_op("d.rpk", op).is_err());
        assert_eq!(std::fs::read(dir.path().join("d.rpk")).unwrap(), bytes);
    }
}
