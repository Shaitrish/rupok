//! Geometry helpers: points, boxes, resampling, crossings and slant.

/// A 2-D point (y grows downwards, as on screen).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Pt) -> f32 {
        (self.x - o.x).hypot(self.y - o.y)
    }

    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// Axis-aligned box. `EMPTY` grows with [`BBox::add`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BBox {
    pub const EMPTY: BBox = BBox {
        x0: f32::INFINITY,
        y0: f32::INFINITY,
        x1: f32::NEG_INFINITY,
        y1: f32::NEG_INFINITY,
    };

    pub fn of(pts: &[Pt]) -> BBox {
        let mut b = BBox::EMPTY;
        for &p in pts {
            b.add(p);
        }
        b
    }

    pub fn of_strokes<'a>(strokes: impl IntoIterator<Item = &'a [Pt]>) -> BBox {
        let mut b = BBox::EMPTY;
        for s in strokes {
            for &p in s {
                b.add(p);
            }
        }
        b
    }

    pub fn add(&mut self, p: Pt) {
        self.x0 = self.x0.min(p.x);
        self.y0 = self.y0.min(p.y);
        self.x1 = self.x1.max(p.x);
        self.y1 = self.y1.max(p.y);
    }

    pub fn union(self, o: BBox) -> BBox {
        BBox {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    pub fn is_empty(self) -> bool {
        self.x0 > self.x1 || self.y0 > self.y1
    }

    pub fn w(self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }

    pub fn h(self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }

    pub fn cx(self) -> f32 {
        0.5 * (self.x0 + self.x1)
    }

    pub fn cy(self) -> f32 {
        0.5 * (self.y0 + self.y1)
    }

    /// The larger side.
    pub fn size(self) -> f32 {
        self.w().max(self.h())
    }

    pub fn diag(self) -> f32 {
        self.w().hypot(self.h())
    }

    /// Chebyshev gap between boxes (0 when they touch or overlap).
    pub fn gap(self, o: BBox) -> f32 {
        let dx = (o.x0 - self.x1).max(self.x0 - o.x1).max(0.0);
        let dy = (o.y0 - self.y1).max(self.y0 - o.y1).max(0.0);
        dx.max(dy)
    }

    /// Length of the overlap of the x ranges (negative: the gap).
    pub fn x_overlap(self, o: BBox) -> f32 {
        self.x1.min(o.x1) - self.x0.max(o.x0)
    }

    pub fn translate(self, dx: f32, dy: f32) -> BBox {
        BBox {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }
}

pub fn path_len(s: &[Pt]) -> f32 {
    s.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// Append `n` points spread evenly along the polyline by arc length (both
/// ends included). One point is the stroke's centroid; a stroke of zero
/// length repeats its first point.
pub fn resample_into(s: &[Pt], n: usize, out: &mut Vec<Pt>) {
    if n == 0 || s.is_empty() {
        return;
    }
    if n == 1 {
        let k = s.len() as f32;
        let (sx, sy) = s.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
        out.push(Pt::new(sx / k, sy / k));
        return;
    }
    let len = path_len(s);
    let start = out.len();
    out.push(s[0]);
    if len <= f32::EPSILON {
        out.extend(std::iter::repeat_n(s[0], n - 1));
        return;
    }
    let interval = len / (n - 1) as f32;
    let mut acc = 0.0;
    let mut prev = s[0];
    let mut i = 1;
    while i < s.len() && out.len() - start < n - 1 {
        let cur = s[i];
        let d = prev.dist(cur);
        if d > 0.0 && acc + d >= interval {
            let q = prev.lerp(cur, (interval - acc) / d);
            out.push(q);
            prev = q;
            acc = 0.0;
        } else {
            acc += d;
            prev = cur;
            i += 1;
        }
    }
    while out.len() - start < n {
        out.push(s[s.len() - 1]);
    }
    let last = out.len() - 1;
    out[last] = s[s.len() - 1];
}

fn orient(a: Pt, b: Pt, c: Pt) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Whether segments `ab` and `cd` intersect (touching counts; collinear
/// overlap does not).
pub fn segments_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let d1 = orient(c, d, a);
    let d2 = orient(c, d, b);
    let d3 = orient(a, b, c);
    let d4 = orient(a, b, d);
    if d1 == 0.0 && d2 == 0.0 {
        return false;
    }
    d1 * d2 <= 0.0 && d3 * d4 <= 0.0
}

/// Whether two strokes cross. Both are coarsened to at most 24 points first.
pub fn strokes_cross(a: &[Pt], b: &[Pt]) -> bool {
    if a.len() < 2 || b.len() < 2 {
        return false;
    }
    let mut ra = Vec::with_capacity(24);
    let mut rb = Vec::with_capacity(24);
    resample_into(a, a.len().clamp(2, 24), &mut ra);
    resample_into(b, b.len().clamp(2, 24), &mut rb);
    let (ba, bb) = (BBox::of(&ra), BBox::of(&rb));
    if ba.gap(bb) > 0.0 {
        return false;
    }
    ra.windows(2).any(|s| {
        rb.windows(2)
            .any(|t| segments_cross(s[0], s[1], t[0], t[1]))
    })
}

/// Whether some point of `a` lies within `tol` of some segment of `b` (both
/// coarsened to at most 24 points).
pub fn strokes_near(a: &[Pt], b: &[Pt], tol: f32) -> bool {
    if a.is_empty() || b.is_empty() || BBox::of(a).gap(BBox::of(b)) > tol {
        return false;
    }
    let mut ra = Vec::with_capacity(24);
    let mut rb = Vec::with_capacity(24);
    resample_into(a, a.len().clamp(1, 24), &mut ra);
    resample_into(b, b.len().clamp(1, 24), &mut rb);
    let seg_dist = |p: Pt, s: Pt, e: Pt| {
        let (dx, dy) = (e.x - s.x, e.y - s.y);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 {
            (((p.x - s.x) * dx + (p.y - s.y) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        p.dist(Pt::new(s.x + t * dx, s.y + t * dy))
    };
    let near = |xs: &[Pt], ys: &[Pt]| {
        xs.iter().any(|&p| {
            if ys.len() == 1 {
                p.dist(ys[0]) <= tol
            } else {
                ys.windows(2).any(|w| seg_dist(p, w[0], w[1]) <= tol)
            }
        })
    };
    near(&ra, &rb) || near(&rb, &ra)
}

/// Writing slant as a shear factor `k` (x' = x + k·(y − y_ref) uprights the
/// ink), estimated from near-vertical chords. 0 when there is too little
/// vertical ink to tell.
pub fn estimate_slant(strokes: &[Vec<Pt>], unit: f32) -> f32 {
    let chord = (0.15 * unit).max(1e-3);
    let max_s = 0.58; // tan 30°: steeper chords count as "vertical"
    let (mut sw, mut sk) = (0.0f32, 0.0f32);
    for s in strokes {
        let mut i = 0;
        while i < s.len() {
            let mut j = i + 1;
            while j < s.len() && s[i].dist(s[j]) < chord {
                j += 1;
            }
            if j >= s.len() {
                break;
            }
            let (dx, dy) = (s[j].x - s[i].x, s[j].y - s[i].y);
            if dy.abs() > 1e-6 {
                let slope = dx / dy;
                if slope.abs() < max_s {
                    let w = s[i].dist(s[j]) * (1.0 - slope.abs() / max_s);
                    sw += w;
                    sk -= w * slope;
                }
            }
            i = j;
        }
    }
    if sw < 1.2 * unit {
        return 0.0;
    }
    (sk / sw).clamp(-0.4, 0.4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_keeps_ends_and_count() {
        let s = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(10.0, 5.0)];
        for n in 1..40 {
            let mut out = Vec::new();
            resample_into(&s, n, &mut out);
            assert_eq!(out.len(), n);
            if n > 1 {
                assert_eq!(out[0], s[0]);
                assert_eq!(out[n - 1], s[2]);
            }
        }
        let mut out = Vec::new();
        resample_into(&[Pt::new(3.0, 4.0)], 5, &mut out);
        assert_eq!(out, vec![Pt::new(3.0, 4.0); 5]);
    }

    #[test]
    fn crossing() {
        let a = [Pt::new(0.0, 0.0), Pt::new(10.0, 10.0)];
        let b = [Pt::new(0.0, 10.0), Pt::new(10.0, 0.0)];
        let c = [Pt::new(20.0, 0.0), Pt::new(30.0, 0.0)];
        assert!(strokes_cross(&a, &b));
        assert!(!strokes_cross(&a, &c));
    }

    #[test]
    fn slant_of_italic_strokes() {
        // Three strokes leaning right by 0.25 (top is right of the bottom).
        let strokes: Vec<Vec<Pt>> = (0..3)
            .map(|i| {
                let x = i as f32 * 30.0;
                (0..=10)
                    .map(|k| {
                        let y = k as f32 * 4.0;
                        Pt::new(x + 0.25 * (40.0 - y), y)
                    })
                    .collect()
            })
            .collect();
        let k = estimate_slant(&strokes, 40.0);
        assert!((k - 0.25).abs() < 0.02, "{k}");
        assert_eq!(estimate_slant(&strokes[..1], 400.0), 0.0);
    }
}
