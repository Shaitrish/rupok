//! In-memory view of one page optimised for interaction: an R*-tree over item
//! bounds plus exact geometric tests (eraser, hit-test, lasso, marquee).
//!
//! The scene is a cache derived from the [`crate::Lesson`]; it is updated
//! incrementally for local edits and rebuilt after remote merges.

use std::collections::HashMap;

use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};
use rupok_ink::Aabb;

use crate::model::{Item, ItemData, ItemId};

type Entry = GeomWithData<Rectangle<[f32; 2]>, u32>;

#[derive(Default)]
pub struct Scene {
    items: Vec<Option<Item>>,
    by_id: HashMap<ItemId, u32>,
    tree: RTree<Entry>,
    free: Vec<u32>,
}

fn rect(b: Aabb) -> Rectangle<[f32; 2]> {
    Rectangle::from_corners([b[0], b[1]], [b[2], b[3]])
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_items(items: Vec<Item>) -> Self {
        let mut s = Self::new();
        let entries: Vec<Entry> = items
            .into_iter()
            .enumerate()
            .map(|(i, it)| {
                let key = i as u32;
                let b = it.data.aabb();
                s.by_id.insert(it.id.clone(), key);
                s.items.push(Some(it));
                Entry::new(rect(b), key)
            })
            .collect();
        s.tree = RTree::bulk_load(entries);
        s
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Item> {
        self.by_id
            .get(id)
            .and_then(|&k| self.items[k as usize].as_ref())
    }

    /// All items in stacking order (bottom first).
    pub fn ordered(&self) -> Vec<&Item> {
        let mut v: Vec<&Item> = self.items.iter().flatten().collect();
        v.sort_by(|a, b| a.z.total_cmp(&b.z).then_with(|| a.id.cmp(&b.id)));
        v
    }

    pub fn insert(&mut self, item: Item) {
        self.remove(&item.id);
        let b = item.data.aabb();
        let key = match self.free.pop() {
            Some(k) => {
                self.items[k as usize] = Some(item.clone());
                k
            }
            None => {
                self.items.push(Some(item.clone()));
                (self.items.len() - 1) as u32
            }
        };
        self.by_id.insert(item.id, key);
        self.tree.insert(Entry::new(rect(b), key));
    }

    pub fn remove(&mut self, id: &str) -> Option<Item> {
        let key = self.by_id.remove(id)?;
        let item = self.items[key as usize].take()?;
        self.tree.remove(&Entry::new(rect(item.data.aabb()), key));
        self.free.push(key);
        Some(item)
    }

    /// Ids whose bounds intersect `b`.
    pub fn query(&self, b: Aabb) -> Vec<&Item> {
        let env = AABB::from_corners([b[0], b[1]], [b[2], b[3]]);
        self.tree
            .locate_in_envelope_intersecting(env)
            .filter_map(|e| self.items[e.data as usize].as_ref())
            .collect()
    }

    /// Topmost item under a point (with a touch tolerance in world units).
    pub fn hit_test(&self, x: f32, y: f32, tol: f32) -> Option<&Item> {
        let mut hits: Vec<&Item> = self
            .query([x - tol, y - tol, x + tol, y + tol])
            .into_iter()
            .filter(|it| match &it.data {
                ItemData::Stroke(s) => stroke_near(s, x, y, tol),
                _ => true,
            })
            .collect();
        hits.sort_by(|a, b| b.z.total_cmp(&a.z).then_with(|| b.id.cmp(&a.id)));
        hits.into_iter().next()
    }

    /// Strokes touched by an eraser moving from `a` to `b` with radius `r`.
    pub fn erase_hits(&self, a: [f32; 2], b: [f32; 2], r: f32) -> Vec<ItemId> {
        let bb = [
            a[0].min(b[0]) - r,
            a[1].min(b[1]) - r,
            a[0].max(b[0]) + r,
            a[1].max(b[1]) + r,
        ];
        self.query(bb)
            .into_iter()
            .filter(|it| match &it.data {
                ItemData::Stroke(s) => {
                    let (ox, oy) = (s.offset[0], s.offset[1]);
                    let pts = &s.points;
                    if pts.len() == 1 {
                        let rr = r + rupok_ink::point_radius(pts[0].p, s.width);
                        return seg_seg_dist(
                            a,
                            b,
                            [pts[0].x + ox, pts[0].y + oy],
                            [pts[0].x + ox, pts[0].y + oy],
                        ) <= rr;
                    }
                    pts.windows(2).any(|w| {
                        let rr = r + rupok_ink::point_radius(w[0].p.max(w[1].p), s.width);
                        seg_seg_dist(a, b, [w[0].x + ox, w[0].y + oy], [w[1].x + ox, w[1].y + oy])
                            <= rr
                    })
                }
                _ => false,
            })
            .map(|it| it.id.clone())
            .collect()
    }

    /// Items fully enclosed by a lasso polygon (world coordinates).
    pub fn lasso(&self, poly: &[[f32; 2]]) -> Vec<ItemId> {
        if poly.len() < 3 {
            return Vec::new();
        }
        let mut bb = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for p in poly {
            bb = [
                bb[0].min(p[0]),
                bb[1].min(p[1]),
                bb[2].max(p[0]),
                bb[3].max(p[1]),
            ];
        }
        self.query(bb)
            .into_iter()
            .filter(|it| match &it.data {
                ItemData::Stroke(s) => {
                    // Mostly-inside rule: ≥ 80% of points inside feels right on boards.
                    let n = s.points.len().max(1);
                    let inside = s
                        .points
                        .iter()
                        .filter(|p| point_in_poly(p.x + s.offset[0], p.y + s.offset[1], poly))
                        .count();
                    inside * 5 >= n * 4
                }
                other => {
                    let b = other.aabb();
                    [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
                        .iter()
                        .all(|c| point_in_poly(c[0], c[1], poly))
                }
            })
            .map(|it| it.id.clone())
            .collect()
    }

    /// Union bounds of a set of items.
    pub fn bounds_of(&self, ids: &[ItemId]) -> Option<Aabb> {
        ids.iter()
            .filter_map(|id| self.get(id))
            .map(|it| it.data.aabb())
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    }
}

fn stroke_near(s: &crate::model::StrokeData, x: f32, y: f32, tol: f32) -> bool {
    let p = [x - s.offset[0], y - s.offset[1]];
    if s.points.len() == 1 {
        let q = s.points[0];
        return ((q.x - p[0]).powi(2) + (q.y - p[1]).powi(2)).sqrt()
            <= tol + rupok_ink::point_radius(q.p, s.width);
    }
    s.points.windows(2).any(|w| {
        seg_point_dist([w[0].x, w[0].y], [w[1].x, w[1].y], p)
            <= tol + rupok_ink::point_radius(w[0].p.max(w[1].p), s.width)
    })
}

fn seg_point_dist(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 <= f32::EPSILON {
        0.0
    } else {
        (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / l2).clamp(0.0, 1.0)
    };
    ((a[0] + t * vx - p[0]).powi(2) + (a[1] + t * vy - p[1]).powi(2)).sqrt()
}

fn seg_seg_dist(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> f32 {
    if segments_intersect(a, b, c, d) {
        return 0.0;
    }
    seg_point_dist(a, b, c)
        .min(seg_point_dist(a, b, d))
        .min(seg_point_dist(c, d, a))
        .min(seg_point_dist(c, d, b))
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn segments_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let (d1, d2, d3, d4) = (
        cross(c, d, a),
        cross(c, d, b),
        cross(a, b, c),
        cross(a, b, d),
    );
    ((d1 > 0.0) != (d2 > 0.0))
        && ((d3 > 0.0) != (d4 > 0.0))
        && d1 != 0.0
        && d2 != 0.0
        && d3 != 0.0
        && d4 != 0.0
}

fn point_in_poly(x: f32, y: f32, poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (pi, pj) = (poly[i], poly[j]);
        if (pi[1] > y) != (pj[1] > y) && x < (pj[0] - pi[0]) * (y - pi[1]) / (pj[1] - pi[1]) + pi[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BoxGeom, ObjectData, StrokeData};
    use rupok_ink::StrokePoint;

    fn stroke(id: &str, z: f64, x0: f32, y: f32) -> Item {
        Item {
            group: None,
            id: id.into(),
            z,
            created: z,
            data: ItemData::Stroke(StrokeData {
                tool: "pen".into(),
                color: 0,
                width: 4.0,
                opacity: 1.0,
                points: (0..=10)
                    .map(|i| StrokePoint {
                        x: x0 + i as f32 * 10.0,
                        y,
                        p: 0.5,
                        dt: 0,
                    })
                    .collect(),
                offset: [0.0, 0.0],
                shape: None,
                geom: None,
            }),
        }
    }

    #[test]
    fn eraser_hits_only_crossed_strokes() {
        let s = Scene::from_items(vec![
            stroke("a", 1.0, 0.0, 0.0),
            stroke("b", 2.0, 0.0, 100.0),
        ]);
        let hits = s.erase_hits([50.0, -20.0], [50.0, 20.0], 3.0);
        assert_eq!(hits, vec!["a".to_string()]);
    }

    #[test]
    fn hit_test_prefers_topmost() {
        let mut s = Scene::from_items(vec![stroke("low", 1.0, 0.0, 0.0)]);
        s.insert(stroke("high", 5.0, 0.0, 0.0));
        assert_eq!(s.hit_test(30.0, 1.0, 2.0).unwrap().id, "high");
        assert!(s.hit_test(30.0, 40.0, 2.0).is_none());
    }

    #[test]
    fn remove_and_reinsert_reuses_slots() {
        let mut s = Scene::new();
        s.insert(stroke("a", 1.0, 0.0, 0.0));
        s.remove("a");
        assert!(s.is_empty());
        assert!(s.query([-100.0, -100.0, 200.0, 200.0]).is_empty());
        s.insert(stroke("b", 1.0, 0.0, 0.0));
        assert_eq!(s.len(), 1);
        assert_eq!(s.items.len(), 1);
    }

    #[test]
    fn lasso_selects_enclosed_items() {
        let obj = Item {
            group: None,
            id: "o".into(),
            z: 3.0,
            created: 3.0,
            data: ItemData::Object(ObjectData {
                geom: BoxGeom {
                    x: 10.0,
                    y: 10.0,
                    w: 20.0,
                    h: 20.0,
                    rot: 0.0,
                },
                bundle: "x".into(),
                title: "x".into(),
                step: 0,
                state: None,
            }),
        };
        let s = Scene::from_items(vec![
            stroke("a", 1.0, 0.0, 0.0),
            stroke("far", 2.0, 500.0, 500.0),
            obj,
        ]);
        let poly = [[-10.0, -10.0], [150.0, -10.0], [150.0, 50.0], [-10.0, 50.0]];
        let mut sel = s.lasso(&poly);
        sel.sort();
        assert_eq!(sel, vec!["a".to_string(), "o".to_string()]);
    }
}
