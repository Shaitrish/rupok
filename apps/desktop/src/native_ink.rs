//! The native wet-ink layer (Windows only, experimental; off by default:
//! Settings → About → Performance lab → Native wet ink).
//!
//! A transparent, click-through window owned by the board window and laid
//! over its client area. It shows only the strokes being written: the board
//! (apps/board/src/nativeInk.ts) sends each pen sample as the webview
//! receives it, and the new piece of the stroke is drawn straight into a
//! DirectComposition surface with Direct2D. Nothing here waits for the
//! webview's frame: the pen's ink reaches the screen through DWM alone,
//! which is the path a native inking app has. Input still goes to the
//! webview (the window never takes a pointer), so every tool and gesture
//! works as before; at pen-up the board draws the finished stroke itself
//! and tells this layer to drop its copy.
//!
//! Drawing is incremental: a batch of samples redraws only the rectangle
//! around its new segments (a DirectComposition surface is updated by
//! rectangle, and DWM recomposes that rectangle only), whatever the screen's
//! size.
//!
//! Everything runs on the thread that made the layer (Tauri runs these
//! commands on the main thread, which owns the window). Any error is
//! returned to the board, which switches the layer off and draws wet ink
//! itself again (docs/performance.md, "Native wet ink").

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_LINE_JOIN_ROUND, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1CreateFactory, ID2D1Device, ID2D1DeviceContext, ID2D1Factory, ID2D1Factory1,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionSurface,
    IDCompositionTarget, IDCompositionVisual2,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, HTTRANSPARENT, LWA_ALPHA,
    RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_NCHITTEST, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Interface, PCWSTR, w};
use windows_numerics::Vector2;

/// More points than this in one stroke and the layer gives up (the board
/// draws the stroke itself): a redraw walks the whole stroke.
const MAX_POINTS: usize = 200_000;
/// Largest surface side (a window on an 8K screen).
const MAX_SIDE: i32 = 8192;

const CLASS: PCWSTR = w!("RupokWetInk");

/// One pen sample in the layer's pixels: position and radius.
#[derive(Clone, Copy)]
struct Pt {
    x: f32,
    y: f32,
    r: f32,
}

struct Stroke {
    seq: u32,
    color: D2D1_COLOR_F,
    /// The page on screen: ink is cut at its edges, as the engine cuts it.
    clip: D2D_RECT_F,
    pts: Vec<Pt>,
    /// Everything this stroke has drawn, for removing it.
    bounds: Option<RECT>,
}

pub struct Layer {
    thread: u32,
    owner: HWND,
    hwnd: HWND,
    size: (i32, i32),
    shown: bool,
    device: IDCompositionDesktopDevice,
    // Kept alive: the target shows the visual, the visual the surface.
    _target: IDCompositionTarget,
    visual: IDCompositionVisual2,
    surface: Option<IDCompositionSurface>,
    style: ID2D1StrokeStyle,
    strokes: Vec<Stroke>,
}

thread_local! {
    static LAYER: RefCell<Option<Layer>> = const { RefCell::new(None) };
}

type Res<T> = Result<T, String>;

fn win<T>(what: &str, r: windows::core::Result<T>) -> Res<T> {
    r.map_err(|e| format!("{what}: {e}"))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        // Never a target for the pointer: the board below gets every event.
        return LRESULT(HTTRANSPARENT as isize);
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

/// The owner's client area on screen: (x, y, width, height).
fn client_area(owner: HWND) -> Res<(i32, i32, i32, i32)> {
    let mut rc = RECT::default();
    win("GetClientRect", unsafe { GetClientRect(owner, &mut rc) })?;
    let mut origin = POINT::default();
    if !unsafe { ClientToScreen(owner, &mut origin) }.as_bool() {
        return Err("ClientToScreen failed".into());
    }
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    if w <= 0 || h <= 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("the board window is {w}×{h}"));
    }
    Ok((origin.x, origin.y, w, h))
}

fn union(a: Option<RECT>, b: RECT) -> RECT {
    match a {
        None => b,
        Some(a) => RECT {
            left: a.left.min(b.left),
            top: a.top.min(b.top),
            right: a.right.max(b.right),
            bottom: a.bottom.max(b.bottom),
        },
    }
}

fn intersects(a: &RECT, b: &RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

/// The pixels a run of points can touch (round caps included).
fn bounds_of(pts: &[Pt]) -> Option<RECT> {
    let mut out: Option<RECT> = None;
    for p in pts {
        if !(p.x.is_finite() && p.y.is_finite() && p.r.is_finite()) {
            continue;
        }
        let r = p.r.clamp(0.5, 512.0) + 2.0;
        out = Some(union(
            out,
            RECT {
                left: (p.x - r).floor() as i32,
                top: (p.y - r).floor() as i32,
                right: (p.x + r).ceil() as i32,
                bottom: (p.y + r).ceil() as i32,
            },
        ));
    }
    out
}

impl Layer {
    /// Make the layer over `owner`'s client area (hidden until a stroke starts).
    fn new(owner: HWND) -> Res<Layer> {
        let (x, y, w, h) = client_area(owner)?;
        unsafe {
            let module = win("GetModuleHandleW", GetModuleHandleW(PCWSTR::null()))?;
            let instance = HINSTANCE::from(module);
            // Registering twice fails harmlessly: the class is already there.
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: CLASS,
                ..Default::default()
            });
            // Layered + transparent: hit-testing skips the window entirely.
            // No redirection bitmap: its only content is the composition visual.
            let hwnd = win(
                "CreateWindowExW",
                CreateWindowExW(
                    WS_EX_NOREDIRECTIONBITMAP
                        | WS_EX_LAYERED
                        | WS_EX_TRANSPARENT
                        | WS_EX_NOACTIVATE
                        | WS_EX_TOOLWINDOW,
                    CLASS,
                    w!(""),
                    WS_POPUP,
                    x,
                    y,
                    w,
                    h,
                    Some(owner),
                    None,
                    Some(instance),
                    None,
                ),
            )?;
            let made = Self::graphics(owner, hwnd, (w, h));
            if made.is_err() {
                let _ = DestroyWindow(hwnd);
            }
            made
        }
    }

    unsafe fn graphics(owner: HWND, hwnd: HWND, size: (i32, i32)) -> Res<Layer> {
        unsafe {
            // A layered window shows once it has attributes. Its pixels come
            // from the composition visual either way, so a refusal is not fatal.
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
            let mut d3d: Option<ID3D11Device> = None;
            win(
                "D3D11CreateDevice",
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    Default::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d3d),
                    None,
                    None,
                ),
            )?;
            let d3d = d3d.ok_or("no Direct3D device")?;
            let dxgi: IDXGIDevice = win("IDXGIDevice", d3d.cast())?;
            let factory: ID2D1Factory1 = win(
                "D2D1CreateFactory",
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None),
            )?;
            let d2d: ID2D1Device = win("ID2D1Factory1::CreateDevice", factory.CreateDevice(&dxgi))?;
            // A composition device made from a Direct2D device hands out
            // Direct2D contexts from `IDCompositionSurface::BeginDraw`.
            let device: IDCompositionDesktopDevice =
                win("DCompositionCreateDevice2", DCompositionCreateDevice2(&d2d))?;
            let target = win(
                "CreateTargetForHwnd",
                device.CreateTargetForHwnd(hwnd, true),
            )?;
            let visual = win("CreateVisual", device.CreateVisual())?;
            win("SetRoot", target.SetRoot(&visual))?;
            let style = win(
                "CreateStrokeStyle",
                // The base interface's: plain caps and joins, no transform type.
                ID2D1Factory::CreateStrokeStyle(
                    &factory,
                    &D2D1_STROKE_STYLE_PROPERTIES {
                        startCap: D2D1_CAP_STYLE_ROUND,
                        endCap: D2D1_CAP_STYLE_ROUND,
                        dashCap: D2D1_CAP_STYLE_ROUND,
                        lineJoin: D2D1_LINE_JOIN_ROUND,
                        miterLimit: 1.0,
                        dashStyle: D2D1_DASH_STYLE_SOLID,
                        dashOffset: 0.0,
                    },
                    None,
                ),
            )?;
            let mut layer = Layer {
                thread: GetCurrentThreadId(),
                owner,
                hwnd,
                size: (0, 0),
                shown: false,
                device,
                _target: target,
                visual,
                surface: None,
                style,
                strokes: Vec::new(),
            };
            layer.resize(size)?;
            Ok(layer)
        }
    }

    fn on_thread(&self) -> Res<()> {
        if unsafe { GetCurrentThreadId() } == self.thread {
            Ok(())
        } else {
            Err("called from another thread".into())
        }
    }

    /// A fresh, transparent surface of the window's size.
    fn resize(&mut self, size: (i32, i32)) -> Res<()> {
        if self.size == size && self.surface.is_some() {
            return Ok(());
        }
        unsafe {
            let surface = win(
                "CreateSurface",
                self.device.CreateSurface(
                    size.0 as u32,
                    size.1 as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                ),
            )?;
            win("SetContent", self.visual.SetContent(&surface))?;
            self.surface = Some(surface);
            self.size = size;
        }
        // A new surface's pixels are undefined until drawn.
        self.redraw(RECT {
            left: 0,
            top: 0,
            right: size.0,
            bottom: size.1,
        })
    }

    /// Follow the board window (moved, resized, another screen).
    fn place(&mut self) -> Res<()> {
        let (x, y, w, h) = client_area(self.owner)?;
        unsafe {
            win(
                "SetWindowPos",
                SetWindowPos(self.hwnd, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER),
            )?;
        }
        self.resize((w, h))
    }

    fn show(&mut self, shown: bool) {
        if self.shown != shown {
            self.shown = shown;
            unsafe {
                let _ = ShowWindow(self.hwnd, if shown { SW_SHOWNOACTIVATE } else { SW_HIDE });
            }
        }
    }

    /// Draw `rect` of the surface again: cleared, then every stroke that
    /// touches it. Only this rectangle is updated and recomposed.
    fn redraw(&mut self, rect: RECT) -> Res<()> {
        let rect = RECT {
            left: rect.left.max(0),
            top: rect.top.max(0),
            right: rect.right.min(self.size.0),
            bottom: rect.bottom.min(self.size.1),
        };
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Ok(());
        }
        let surface = self.surface.as_ref().ok_or("no surface")?;
        unsafe {
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = win(
                "IDCompositionSurface::BeginDraw",
                surface.BeginDraw(Some(&rect), &mut offset),
            )?;
            // The context draws into an atlas: surface pixel (x, y) is at
            // (x + dx, y + dy), and nothing outside the rectangle is ours.
            let (dx, dy) = ((offset.x - rect.left) as f32, (offset.y - rect.top) as f32);
            let drawn = (|| -> Res<()> {
                let area = D2D_RECT_F {
                    left: offset.x as f32,
                    top: offset.y as f32,
                    right: (offset.x + rect.right - rect.left) as f32,
                    bottom: (offset.y + rect.bottom - rect.top) as f32,
                };
                dc.PushAxisAlignedClip(&area, D2D1_ANTIALIAS_MODE_ALIASED);
                dc.Clear(Some(&D2D1_COLOR_F::default()));
                for s in &self.strokes {
                    if !s.bounds.as_ref().is_some_and(|b| intersects(b, &rect)) {
                        continue;
                    }
                    let brush = match dc.CreateSolidColorBrush(&s.color, None) {
                        Ok(b) => b,
                        Err(e) => {
                            dc.PopAxisAlignedClip();
                            return Err(format!("CreateSolidColorBrush: {e}"));
                        }
                    };
                    let page = D2D_RECT_F {
                        left: s.clip.left + dx,
                        top: s.clip.top + dy,
                        right: s.clip.right + dx,
                        bottom: s.clip.bottom + dy,
                    };
                    dc.PushAxisAlignedClip(&page, D2D1_ANTIALIAS_MODE_ALIASED);
                    if let [p] = s.pts[..] {
                        dc.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: Vector2::new(p.x + dx, p.y + dy),
                                radiusX: p.r,
                                radiusY: p.r,
                            },
                            &brush,
                        );
                    }
                    for seg in s.pts.windows(2) {
                        let (a, b) = (seg[0], seg[1]);
                        // Only segments that reach the rectangle.
                        if bounds_of(seg).is_none_or(|sb| !intersects(&sb, &rect)) {
                            continue;
                        }
                        dc.DrawLine(
                            Vector2::new(a.x + dx, a.y + dy),
                            Vector2::new(b.x + dx, b.y + dy),
                            &brush,
                            a.r + b.r,
                            &self.style,
                        );
                    }
                    dc.PopAxisAlignedClip();
                }
                dc.PopAxisAlignedClip();
                Ok(())
            })();
            // Always closed, or the surface stays locked.
            let ended = surface.EndDraw();
            drawn?;
            win("IDCompositionSurface::EndDraw", ended)?;
            win("Commit", self.device.Commit())
        }
    }

    fn begin(&mut self, seq: u32, rgb: [u8; 3], clip: [f32; 4], p: [f32; 3]) -> Res<()> {
        self.on_thread()?;
        if self.strokes.len() >= 8 {
            return Err("too many strokes waiting for the board".into());
        }
        self.place()?;
        let pt = Pt {
            x: p[0],
            y: p[1],
            r: p[2],
        };
        let bounds = bounds_of(&[pt]);
        self.strokes.push(Stroke {
            seq,
            color: D2D1_COLOR_F {
                r: f32::from(rgb[0]) / 255.0,
                g: f32::from(rgb[1]) / 255.0,
                b: f32::from(rgb[2]) / 255.0,
                a: 1.0,
            },
            clip: D2D_RECT_F {
                left: clip[0],
                top: clip[1],
                right: clip[2],
                bottom: clip[3],
            },
            pts: vec![pt],
            bounds,
        });
        self.show(true);
        match bounds {
            Some(b) => self.redraw(b),
            None => Ok(()),
        }
    }

    fn points(&mut self, seq: u32, flat: &[f32]) -> Res<()> {
        self.on_thread()?;
        let Some(s) = self.strokes.iter_mut().find(|s| s.seq == seq) else {
            // Already handed over (a late batch): nothing to draw.
            return Ok(());
        };
        if s.pts.len() + flat.len() / 3 > MAX_POINTS {
            return Err("the stroke is too long for the native layer".into());
        }
        // The new segments start at the last point already drawn.
        let from = s.pts.len() - 1;
        s.pts.extend(
            flat.as_chunks::<3>()
                .0
                .iter()
                .map(|&[x, y, r]| Pt { x, y, r }),
        );
        let Some(dirty) = bounds_of(&s.pts[from..]) else {
            return Ok(());
        };
        s.bounds = Some(union(s.bounds, dirty));
        self.redraw(dirty)
    }

    /// The board shows the finished stroke: remove ours.
    fn end(&mut self, seq: u32) -> Res<()> {
        self.on_thread()?;
        // This stroke and any older one whose end was lost.
        let mut dirty: Option<RECT> = None;
        self.strokes.retain(|s| {
            if s.seq > seq {
                return true;
            }
            if let Some(b) = s.bounds {
                dirty = Some(union(dirty, b));
            }
            false
        });
        let drawn = match dirty {
            Some(d) => self.redraw(d),
            None => Ok(()),
        };
        if self.strokes.is_empty() {
            // Hidden between strokes: DWM has nothing to blend over the board.
            self.show(false);
        }
        drawn
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.visual.SetContent(None);
            let _ = self.device.Commit();
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Make the layer over the window `owner` (its raw `HWND`). Idempotent.
pub fn start(owner: isize) -> Res<()> {
    LAYER.with(|l| {
        let mut l = l.borrow_mut();
        if l.is_none() {
            *l = Some(Layer::new(HWND(owner as *mut core::ffi::c_void))?);
        }
        Ok(())
    })
}

/// Destroy the layer (switched off, or after an error).
pub fn stop() {
    LAYER.with(|l| {
        // `try_borrow_mut`: never panic if an error path re-enters.
        if let Ok(mut l) = l.try_borrow_mut() {
            *l = None;
        }
    });
}

fn with<T>(f: impl FnOnce(&mut Layer) -> Res<T>) -> Res<T> {
    let out = LAYER.with(|l| match l.try_borrow_mut() {
        Ok(mut l) => match l.as_mut() {
            Some(layer) => f(layer),
            None => Err("the native layer is not running".into()),
        },
        Err(_) => Err("the native layer is busy".into()),
    });
    if out.is_err() {
        // Fail safe: whatever went wrong, the layer goes and the board
        // draws its own wet ink again.
        stop();
    }
    out
}

/// A stroke starts at `point` (x, y, radius; pixels of the owner's client
/// area), in colour `rgb`, cut at `clip` (left, top, right, bottom).
pub fn begin(seq: u32, rgb: [u8; 3], clip: [f32; 4], point: [f32; 3]) -> Res<()> {
    with(|l| l.begin(seq, rgb, clip, point))
}

/// Whether the layer exists on this thread.
fn running() -> bool {
    LAYER.with(|l| l.try_borrow().is_ok_and(|l| l.is_some()))
}

/// More samples of stroke `seq`: flat `[x, y, radius, …]`. A batch that
/// arrives after the layer was stopped has nothing to draw on.
pub fn points(seq: u32, flat: &[f32]) -> Res<()> {
    if !running() {
        return Ok(());
    }
    with(|l| l.points(seq, flat))
}

/// The board shows stroke `seq` itself now. After a stop there is nothing
/// left to remove.
pub fn end(seq: u32) -> Res<()> {
    if !running() {
        return Ok(());
    }
    with(|l| l.end(seq))
}
