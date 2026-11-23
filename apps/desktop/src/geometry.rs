//! Overlay geometry and the screenshot wire format. Pure functions, so they
//! are tested without a display.

/// A rectangle in physical pixels (desktop coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn center(&self) -> (i32, i32) {
        (self.x + (self.w / 2) as i32, self.y + (self.h / 2) as i32)
    }
}

/// Where the floating overlay toolbar goes: centred horizontally near the
/// bottom of `monitor`, `margin` physical pixels above the edge, and always
/// fully on that monitor even when it is wider than the screen.
pub fn toolbar_position(monitor: Rect, toolbar_w: u32, toolbar_h: u32, margin: u32) -> (i32, i32) {
    let x = monitor.x + (monitor.w.saturating_sub(toolbar_w) / 2) as i32;
    let bottom = monitor.h.saturating_sub(toolbar_h);
    let y = monitor.y + bottom.saturating_sub(margin) as i32;
    (x, y)
}

/// Length of the screenshot header: width and height as little-endian `u32`.
pub const FRAME_HEADER: usize = 8;

/// Pack an RGBA screenshot for the webview: `[w: u32 LE][h: u32 LE][rgba…]`.
/// Raw pixels avoid an encode/decode round trip; the webview composites the
/// ink on top and encodes once. Returns `None` if `rgba` is not `w × h × 4`.
pub fn pack_frame(w: u32, h: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let expected = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }
    let mut out = Vec::with_capacity(FRAME_HEADER + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    Some(out)
}

/// Convert X11 `ZPixmap` pixels (32 bits per pixel, blue-green-red-pad in
/// memory on little-endian servers) to RGBA with full alpha, in place.
pub fn bgrx_to_rgba(px: &mut [u8]) {
    for p in px.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };

    #[test]
    fn toolbar_sits_bottom_centre() {
        assert_eq!(toolbar_position(FHD, 600, 80, 24), (660, 976));
    }

    #[test]
    fn toolbar_on_a_secondary_monitor() {
        let right = Rect {
            x: 1920,
            y: -200,
            w: 2560,
            h: 1440,
        };
        assert_eq!(
            toolbar_position(right, 600, 80, 24),
            (1920 + 980, -200 + 1336)
        );
    }

    #[test]
    fn toolbar_wider_than_monitor_stays_on_it() {
        let tiny = Rect {
            x: 100,
            y: 0,
            w: 400,
            h: 60,
        };
        assert_eq!(toolbar_position(tiny, 600, 80, 24), (100, 0));
    }

    #[test]
    fn centre_of_rect() {
        assert_eq!(
            Rect {
                x: -1920,
                y: 0,
                w: 1920,
                h: 1080
            }
            .center(),
            (-960, 540)
        );
    }

    #[test]
    fn frame_has_header_and_checks_length() {
        let f = pack_frame(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(&f[..FRAME_HEADER], &[2, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&f[FRAME_HEADER..], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(pack_frame(2, 2, vec![0; 8]).is_none());
        assert!(pack_frame(u32::MAX, u32::MAX, vec![]).is_none());
    }

    #[test]
    fn bgrx_becomes_opaque_rgba() {
        let mut px = vec![10, 20, 30, 0, 1, 2, 3, 99];
        bgrx_to_rgba(&mut px);
        assert_eq!(px, vec![30, 20, 10, 255, 3, 2, 1, 255]);
    }
}
