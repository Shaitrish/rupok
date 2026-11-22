//! Compact, versioned point encoding (spec: `spec/rupok-format.md#stroke-points`).
//!
//! Layout (v1):
//! ```text
//! u8      version (=1)
//! varint  n                         number of points
//! repeat n:
//!   zigzag-varint  dx               x quantised to 1/16 world unit, delta from previous
//!   zigzag-varint  dy
//!   u8             p                pressure * 255
//!   varint         ddt              ms delta from previous point (monotonic)
//! ```
//! Typical cost is 4–6 bytes per point.

use crate::StrokePoint;

pub const VERSION: u8 = 1;
pub const QUANT: f32 = 16.0;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("empty buffer")]
    Empty,
    #[error("unsupported point codec version {0}")]
    Version(u8),
    #[error("truncated point data")]
    Truncated,
    #[error("point count {0} exceeds limit")]
    TooMany(u64),
}

/// Hard cap so a malicious file cannot request a huge allocation.
pub const MAX_POINTS: u64 = 1 << 20;

pub fn encode_points(points: &[StrokePoint]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + points.len() * 5);
    out.push(VERSION);
    put_varint(&mut out, points.len() as u64);
    let (mut px, mut py, mut pdt) = (0i64, 0i64, 0u32);
    for p in points {
        let qx = (p.x * QUANT).round() as i64;
        let qy = (p.y * QUANT).round() as i64;
        put_varint(&mut out, zigzag(qx - px));
        put_varint(&mut out, zigzag(qy - py));
        out.push((p.p.clamp(0.0, 1.0) * 255.0).round() as u8);
        let dt = p.dt.max(pdt);
        put_varint(&mut out, u64::from(dt - pdt));
        (px, py, pdt) = (qx, qy, dt);
    }
    out
}

pub fn decode_points(buf: &[u8]) -> Result<Vec<StrokePoint>, CodecError> {
    let (&version, mut rest) = buf.split_first().ok_or(CodecError::Empty)?;
    if version != VERSION {
        return Err(CodecError::Version(version));
    }
    let n = get_varint(&mut rest)?;
    if n > MAX_POINTS {
        return Err(CodecError::TooMany(n));
    }
    let mut out = Vec::with_capacity(n as usize);
    let (mut px, mut py, mut pdt) = (0i64, 0i64, 0u64);
    for _ in 0..n {
        px += unzigzag(get_varint(&mut rest)?);
        py += unzigzag(get_varint(&mut rest)?);
        let (&p, r) = rest.split_first().ok_or(CodecError::Truncated)?;
        rest = r;
        pdt += get_varint(&mut rest)?;
        out.push(StrokePoint {
            x: px as f32 / QUANT,
            y: py as f32 / QUANT,
            p: f32::from(p) / 255.0,
            dt: pdt.min(u64::from(u32::MAX)) as u32,
        });
    }
    Ok(out)
}

#[inline]
fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

#[inline]
fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(buf: &mut &[u8]) -> Result<u64, CodecError> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let (&b, rest) = buf.split_first().ok_or(CodecError::Truncated)?;
        *buf = rest;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(CodecError::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_within_quantisation() {
        let pts: Vec<_> = (0..500)
            .map(|i| {
                let f = i as f32;
                StrokePoint {
                    x: 1000.0 + f.sin() * 50.3,
                    y: -20.0 + f * 0.77,
                    p: (f / 500.0),
                    dt: i * 7,
                }
            })
            .collect();
        let bytes = encode_points(&pts);
        let back = decode_points(&bytes).unwrap();
        assert_eq!(back.len(), pts.len());
        for (a, b) in pts.iter().zip(&back) {
            assert!((a.x - b.x).abs() <= 0.5 / QUANT + 1e-3);
            assert!((a.y - b.y).abs() <= 0.5 / QUANT + 1e-3);
            assert!((a.p - b.p).abs() <= 1.0 / 255.0);
            assert_eq!(a.dt, b.dt);
        }
        assert!(
            bytes.len() < pts.len() * 7,
            "encoding too large: {}",
            bytes.len()
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(decode_points(&[]), Err(CodecError::Empty));
        assert_eq!(decode_points(&[9, 0]), Err(CodecError::Version(9)));
        assert_eq!(decode_points(&[1, 3, 0]), Err(CodecError::Truncated));
        assert!(matches!(
            decode_points(&[1, 0xff, 0xff, 0xff, 0xff, 0x0f]),
            Err(CodecError::TooMany(_))
        ));
    }

    #[test]
    fn non_monotonic_dt_is_clamped() {
        let pts = [
            StrokePoint {
                x: 0.0,
                y: 0.0,
                p: 0.5,
                dt: 10,
            },
            StrokePoint {
                x: 1.0,
                y: 0.0,
                p: 0.5,
                dt: 5,
            },
        ];
        let back = decode_points(&encode_points(&pts)).unwrap();
        assert_eq!(back[1].dt, 10);
    }
}
