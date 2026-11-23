//! Phase 2 templates: synthetic hand-drawn diagrams that must be read, and
//! near misses (letters, crosses at the wrong angle, uneven ticks, circles
//! inside circles, …) that must stay ink. `stress_phase2` prints the rates
//! the thresholds were tuned against; `stress_phase2_quick` asserts them on
//! a smaller sample in every test run.

use super::super::registry::{Context, Neighbour, Proposal, Registry};
use super::super::{family, graph, logic};
use super::*;

fn all() -> Options {
    Options {
        families: family::ALL,
        ..Options::default()
    }
}

fn raw(points: Vec<P>, gap_ms: f64) -> Neighbour {
    Neighbour {
        id: "n".into(),
        bounds: bbox(&points),
        points,
        shape: None,
        geometry: None,
        gap_ms,
    }
}

fn shaped(r: Recognized, gap_ms: f64) -> Neighbour {
    let points = outline(&r.geometry);
    Neighbour {
        id: "s".into(),
        bounds: bbox(&points),
        points,
        shape: Some(r.kind.as_str().into()),
        geometry: Some(r.geometry),
        gap_ms,
    }
}

/// Rough outline points of a geometry (only bounds and proximity matter).
fn outline(g: &Geometry) -> Vec<P> {
    match g {
        Geometry::Segment { a, b } => densify(&[*a, *b], 4.0),
        Geometry::Ellipse { c, rx, ry, rot } => ellipse_path(*c, *rx, *ry, *rot, 0.0, 1.0),
        Geometry::Rect { c, w, h, rot } => rotate(
            &poly_path(
                &[
                    [c[0] - w / 2.0, c[1] - h / 2.0],
                    [c[0] + w / 2.0, c[1] - h / 2.0],
                    [c[0] + w / 2.0, c[1] + h / 2.0],
                    [c[0] - w / 2.0, c[1] + h / 2.0],
                ],
                0.0,
            ),
            *c,
            *rot,
        ),
        Geometry::Polygon { pts } => poly_path(pts, 0.0),
        Geometry::Template { x, y, w, h, .. } => {
            poly_path(&[[*x, *y], [x + w, *y], [x + w, y + h], [*x, y + h]], 0.0)
        }
    }
}

fn ctx(neighbours: Vec<Neighbour>) -> Context {
    Context {
        opts: all(),
        neighbours,
    }
}

fn run(stroke: &[P], c: &Context) -> Option<Proposal> {
    Registry::standard().recognize(&sp(stroke), c, true)
}

fn by(stroke: &[P], c: &Context) -> Option<ShapeKind> {
    run(stroke, c).map(|p| p.recognized.kind)
}

fn line_shape(a: P, b: P) -> Recognized {
    Recognized {
        kind: ShapeKind::Line,
        geometry: Geometry::Segment { a, b },
        confidence: 1.0,
    }
}

fn h(path: &[P], rng: &mut Rng) -> Vec<P> {
    hand(path, rng, 0.006, 0.6)
}

// ------------------------------------------------------------------ axes

fn axes_plus(rng: &mut Rng, o: P, l: f32, r: f32, t: f32, b: f32, tilt: f32) -> (Vec<P>, Vec<P>) {
    let hx = rotate(&[[o[0] - l, o[1]], [o[0] + r, o[1]]], o, tilt);
    let vy = rotate(&[[o[0], o[1] + b], [o[0], o[1] - t]], o, tilt);
    (h(&hx, rng), h(&vy, rng))
}

#[test]
fn a_plus_becomes_axes_with_labels() {
    let mut rng = Rng(11);
    let (x, y) = axes_plus(&mut rng, [400.0, 400.0], 250.0, 350.0, 280.0, 200.0, 0.03);
    let p = run(&y, &ctx(vec![raw(x.clone(), 400.0)])).expect("axes");
    assert_eq!(
        (p.recognized.kind, p.absorbs.as_slice()),
        (ShapeKind::Axes, &[0usize][..])
    );
    let f = graph::Frame::of(&p.recognized.geometry).unwrap();
    assert!(dist(f.o, [400.0, 400.0]) < 8.0, "{f:?}");
    assert!((f.ux - f.uy).abs() < 1e-3 && f.ux > 30.0);
    let texts: Vec<&str> = p.labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["x", "y"]);
    // The first axis as a snapped line works the same.
    let p = run(
        &y,
        &ctx(vec![shaped(
            line_shape([150.0, 400.0], [750.0, 400.0]),
            400.0,
        )]),
    );
    assert_eq!(p.map(|p| p.recognized.kind), Some(ShapeKind::Axes));
    // Too late, diagonal, a "T", too small, or the family off: no axes.
    assert_eq!(
        by(&y, &ctx(vec![raw(x.clone(), 6000.0)])),
        Some(ShapeKind::Line)
    );
    let (dx, dy) = axes_plus(&mut rng, [400.0, 400.0], 250.0, 250.0, 250.0, 250.0, 0.78);
    assert_ne!(by(&dy, &ctx(vec![raw(dx, 300.0)])), Some(ShapeKind::Axes));
    let tx = h(&[[150.0, 200.0], [650.0, 200.0]], &mut rng);
    let ty = h(&[[400.0, 200.0], [400.0, 600.0]], &mut rng);
    assert_ne!(by(&ty, &ctx(vec![raw(tx, 300.0)])), Some(ShapeKind::Axes));
    let (sx, sy) = axes_plus(&mut rng, [400.0, 400.0], 30.0, 30.0, 30.0, 30.0, 0.0);
    assert_eq!(by(&sy, &ctx(vec![raw(sx, 300.0)])), None);
    let mut off = ctx(vec![raw(x, 400.0)]);
    off.opts.families = family::ALL & !family::GRAPHS;
    assert_ne!(by(&y, &off), Some(ShapeKind::Axes));
}

#[test]
fn an_l_becomes_first_quadrant_axes() {
    let mut rng = Rng(12);
    let l = h(&[[200.0, 120.0], [200.0, 520.0], [700.0, 520.0]], &mut rng);
    let p = run(&l, &ctx(vec![])).expect("axes");
    assert_eq!(p.recognized.kind, ShapeKind::Axes);
    let f = graph::Frame::of(&p.recognized.geometry).unwrap();
    assert!(dist(f.o, [200.0, 520.0]) < 10.0);
    // Drawn the other way round too.
    let mut back = l.clone();
    back.reverse();
    assert_eq!(by(&back, &ctx(vec![])), Some(ShapeKind::Axes));
    // Facing another way, too small, or not square: not axes.
    let gamma = h(&[[700.0, 120.0], [200.0, 120.0], [200.0, 520.0]], &mut rng);
    assert_ne!(by(&gamma, &ctx(vec![])), Some(ShapeKind::Axes));
    let small = h(&[[200.0, 400.0], [200.0, 520.0], [300.0, 520.0]], &mut rng);
    assert_ne!(by(&small, &ctx(vec![])), Some(ShapeKind::Axes));
    let vee = h(&[[200.0, 120.0], [400.0, 520.0], [700.0, 160.0]], &mut rng);
    assert_ne!(by(&vee, &ctx(vec![])), Some(ShapeKind::Axes));
}

// ---------------------------------------------------------------- curves

fn axes_neighbour() -> (Neighbour, graph::Frame) {
    let g = Geometry::Template {
        kind: "axes",
        x: 100.0,
        y: 100.0,
        w: 800.0,
        h: 700.0,
        rot: 0.0,
        v: vec![300.0 / 800.0, 400.0 / 700.0, 50.0 / 800.0, 50.0 / 700.0],
    };
    let f = graph::Frame::of(&g).unwrap();
    (
        shaped(
            Recognized {
                kind: ShapeKind::Axes,
                geometry: g,
                confidence: 1.0,
            },
            60_000.0,
        ),
        f,
    )
}

fn curve(f: &graph::Frame, fun: &dyn Fn(f32) -> f32, x0: f32, x1: f32) -> Vec<P> {
    (0..=120)
        .map(|i| {
            let x = x0 + (x1 - x0) * i as f32 / 120.0;
            [f.o[0] + x * f.ux, f.o[1] - fun(x) * f.uy]
        })
        .collect()
}

#[test]
fn curves_over_axes_are_fitted_and_labelled() {
    let mut rng = Rng(21);
    let (ax, f) = axes_neighbour();
    type Case = (Box<dyn Fn(f32) -> f32>, f32, f32, ShapeKind, &'static str);
    let cases: Vec<Case> = vec![
        (
            Box::new(|x| x * x),
            -2.6,
            2.6,
            ShapeKind::GraphQuad,
            "y = x²",
        ),
        (
            Box::new(|x| 2.0 * x + 1.0),
            -3.0,
            2.5,
            ShapeKind::GraphLine,
            "y = 2x + 1",
        ),
        (
            Box::new(|x| 2.0 * x.sin()),
            -5.5,
            9.5,
            ShapeKind::GraphSine,
            "y = 2 sin(x)",
        ),
        (
            Box::new(|x| 2f32.powf(x)),
            -5.0,
            2.8,
            ShapeKind::GraphExp,
            "y = 2ˣ",
        ),
        (
            Box::new(|x| -0.5 * x * x + 3.0),
            -3.0,
            3.0,
            ShapeKind::GraphQuad,
            "y = −0.5x² + 3",
        ),
    ];
    for (fun, x0, x1, kind, label) in cases {
        let ink = hand(&curve(&f, &*fun, x0, x1), &mut rng, 0.004, 0.8);
        let p = run(&ink, &ctx(vec![ax.clone()])).unwrap_or_else(|| panic!("{label}"));
        assert_eq!(p.recognized.kind, kind, "{label}");
        assert_eq!(p.labels[0].text, label);
        assert_eq!(p.join, Some(0));
        assert!(p.absorbs.is_empty());
    }
}

#[test]
fn squiggles_loops_and_writing_on_axes_stay_ink() {
    let mut rng = Rng(22);
    let (ax, f) = axes_neighbour();
    let c = ctx(vec![ax]);
    // A closed loop, a scribble, a retraced x axis, an upright line.
    let lp = hand(
        &ellipse_path([500.0, 300.0], 120.0, 80.0, 0.0, 0.0, 1.03),
        &mut rng,
        0.01,
        0.5,
    );
    assert_ne!(
        by(&lp, &c).map(|k| k.as_str().starts_with("graph")),
        Some(true)
    );
    let squiggle: Vec<P> = (0..200)
        .map(|i| {
            let t = i as f32 / 200.0;
            [
                250.0 + 500.0 * t,
                300.0 + 60.0 * (t * 37.0).sin() + 40.0 * (t * 11.0).cos(),
            ]
        })
        .collect();
    assert!(!by(&squiggle, &c).is_some_and(|k| k.as_str().starts_with("graph")));
    let retrace = h(&[[150.0, 500.0], [850.0, 500.0]], &mut rng);
    assert!(!by(&retrace, &c).is_some_and(|k| k.as_str().starts_with("graph")));
    let up = h(&[[600.0, 150.0], [602.0, 700.0]], &mut rng);
    assert!(!by(&up, &c).is_some_and(|k| k.as_str().starts_with("graph")));
    let _ = f;
}

#[test]
fn refit_recovers_the_parameters_of_a_clean_curve() {
    let (_, f) = axes_neighbour();
    let pts = curve(&f, &|x| 0.5 * x * x - 2.0 * x - 1.0, -2.0, 6.0);
    let fit = graph::refit(&f, &pts, ShapeKind::GraphQuad).unwrap();
    assert_eq!(fit.params, vec![0.5, -2.0, -1.0]);
    let pts = curve(&f, &|x| 1.5 * (2.0 * x + 0.5).sin() - 1.0, -4.0, 4.0);
    let fit = graph::refit(&f, &pts, ShapeKind::GraphSine).unwrap();
    for (a, b) in fit.params.iter().zip([1.5, 2.0, 0.5, -1.0]) {
        assert!((a - b).abs() < 0.02, "{:?}", fit.params);
    }
}

// ----------------------------------------------------------- number line

fn number_line(rng: &mut Rng, xs: &[f32], y: f32) -> (Vec<P>, Vec<Vec<P>>) {
    let line = h(&[[100.0, y], [900.0, y + 2.0]], rng);
    let ticks = xs
        .iter()
        .map(|&x| h(&[[x + rng.range(-2.0, 2.0), y - 16.0], [x, y + 15.0]], rng))
        .collect();
    (line, ticks)
}

fn deferred(stroke: &[P], c: &Context) -> Option<Proposal> {
    Registry::standard().recognize_deferred(stroke, c)
}

#[test]
fn a_line_with_even_ticks_becomes_a_number_line() {
    let mut rng = Rng(31);
    let (_line, ticks) = number_line(&mut rng, &[300.0, 400.0, 500.0, 600.0, 700.0], 400.0);
    let mut n = vec![shaped(line_shape([100.0, 401.0], [900.0, 401.0]), 8000.0)];
    n.extend(ticks[..4].iter().map(|t| raw(t.clone(), 1500.0)));
    let p = deferred(&ticks[4], &ctx(n.clone())).expect("number line");
    assert_eq!(p.recognized.kind, ShapeKind::NumberLine);
    assert_eq!(p.absorbs.len(), 5);
    let labels: Vec<&str> = p.labels.iter().map(|l| l.text.as_str()).collect();
    // Zero at the middle tick drawn (x = 500), filled across the line.
    assert_eq!(labels, ["−3", "−2", "−1", "0", "1", "2", "3"]);
    let zero = p.labels.iter().find(|l| l.text == "0").unwrap();
    assert!((zero.at[0] - 500.0).abs() < 4.0);
    // Not when the pen-up pass runs (deferred only).
    assert_ne!(by(&ticks[4], &ctx(n.clone())), Some(ShapeKind::NumberLine));
    // Three ticks, uneven ticks, or a word on the line: no number line.
    assert!(deferred(&ticks[4], &ctx(n[..3].to_vec())).is_none());
    let (_, uneven) = number_line(&mut rng, &[300.0, 380.0, 520.0, 560.0, 700.0], 400.0);
    let mut nu = vec![n[0].clone()];
    nu.extend(uneven[..4].iter().map(|t| raw(t.clone(), 1500.0)));
    assert!(deferred(&uneven[4], &ctx(nu)).is_none());
    let mut nw = n.clone();
    nw.push(raw(letter("a", 60.0, [520.0, 370.0]), 900.0));
    assert!(deferred(&ticks[4], &ctx(nw)).is_none());
}

// ------------------------------------------------------- tables and venn

fn rect_shape(c: P, w: f32, hh: f32) -> Recognized {
    Recognized {
        kind: ShapeKind::Rectangle,
        geometry: Geometry::Rect {
            c,
            w,
            h: hh,
            rot: 0.0,
        },
        confidence: 1.0,
    }
}

#[test]
fn lines_across_a_rectangle_make_a_table() {
    let mut rng = Rng(41);
    let r = shaped(rect_shape([500.0, 400.0], 400.0, 300.0), 3000.0);
    let across = h(&[[296.0, 402.0], [706.0, 398.0]], &mut rng);
    let p = run(&across, &ctx(vec![r.clone()])).expect("table");
    assert_eq!(p.recognized.kind, ShapeKind::Table);
    let Geometry::Template { v, .. } = &p.recognized.geometry else {
        panic!()
    };
    assert_eq!(v, &vec![1.0, 0.5]);
    // A second line (upright) adds a column.
    let t = shaped(p.recognized, 1000.0);
    let down = h(&[[401.0, 255.0], [399.0, 548.0]], &mut rng);
    let p = run(&down, &ctx(vec![t])).expect("table");
    let Geometry::Template { v, .. } = &p.recognized.geometry else {
        panic!()
    };
    assert_eq!(v.len(), 4);
    // Short of the edges or slanted: just a line.
    let short = h(&[[380.0, 400.0], [620.0, 400.0]], &mut rng);
    assert_ne!(by(&short, &ctx(vec![r.clone()])), Some(ShapeKind::Table));
    let slant = h(&[[300.0, 300.0], [700.0, 480.0]], &mut rng);
    assert_ne!(by(&slant, &ctx(vec![r])), Some(ShapeKind::Table));
}

fn circle_shape(c: P, r: f32) -> Recognized {
    Recognized {
        kind: ShapeKind::Circle,
        geometry: Geometry::Ellipse {
            c,
            rx: r,
            ry: r,
            rot: 0.0,
        },
        confidence: 1.0,
    }
}

#[test]
fn overlapping_circles_make_a_venn_diagram() {
    let mut rng = Rng(51);
    let a = shaped(circle_shape([400.0, 400.0], 150.0), 2000.0);
    let b_ink = hand(
        &ellipse_path([590.0, 408.0], 140.0, 138.0, 0.0, 0.3, 1.03),
        &mut rng,
        0.006,
        0.6,
    );
    let p = run(&b_ink, &ctx(vec![a.clone()])).expect("venn");
    assert_eq!(p.recognized.kind, ShapeKind::Venn);
    assert_eq!(p.extra.len(), 1);
    let (Geometry::Ellipse { c: c1, rx: r1, .. }, Geometry::Ellipse { c: c2, rx: r2, .. }) =
        (&p.recognized.geometry, &p.extra[0].geometry)
    else {
        panic!()
    };
    assert_eq!(r1, r2);
    assert!((c1[1] - c2[1]).abs() < 1e-3, "level");
    // A third circle overlapping both.
    let b = shaped(circle_shape([590.0, 400.0], 145.0), 1000.0);
    let c_ink = hand(
        &ellipse_path([495.0, 250.0], 145.0, 143.0, 0.0, 0.0, 1.03),
        &mut rng,
        0.006,
        0.6,
    );
    let p = run(&c_ink, &ctx(vec![a.clone(), b])).expect("venn of three");
    assert_eq!((p.absorbs.len(), p.extra.len()), (2, 2));
    // Inside, far away or much smaller: no Venn diagram.
    for (c, r) in [
        ([410.0, 400.0], 60.0),
        ([900.0, 400.0], 150.0),
        ([500.0, 400.0], 60.0),
    ] {
        let ink = hand(&ellipse_path(c, r, r, 0.0, 0.0, 1.03), &mut rng, 0.006, 0.5);
        assert_ne!(
            by(&ink, &ctx(vec![a.clone()])),
            Some(ShapeKind::Venn),
            "{c:?} {r}"
        );
    }
}

// ------------------------------------------------------------------ gates

/// AND's "D": back at x0 from y0 to y0+hh, round front `depth` deep.
fn d_shape(x0: f32, y0: f32, hh: f32, depth: f32) -> Vec<P> {
    let mut p = vec![[x0, y0 + hh], [x0, y0]];
    let flat = (depth - hh / 2.0).max(0.0);
    p.push([x0 + flat, y0]);
    let rx = depth - flat;
    for i in 0..=40 {
        let a = -PI / 2.0 + PI * i as f32 / 40.0;
        p.push([x0 + flat + rx * a.cos(), y0 + hh / 2.0 + hh / 2.0 * a.sin()]);
    }
    p.push([x0, y0 + hh]);
    p.push([x0, y0 + hh - 6.0]);
    p
}

/// OR's shield.
fn shield(x0: f32, y0: f32, hh: f32, len: f32) -> Vec<P> {
    let q = |a: P, c: P, b: P, n: usize| -> Vec<P> {
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                [
                    u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0],
                    u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1],
                ]
            })
            .collect()
    };
    let (top, bot, tip) = ([x0, y0], [x0, y0 + hh], [x0 + len, y0 + hh / 2.0]);
    let mut p = q(top, [x0 + 0.55 * len, y0], tip, 30);
    p.extend(q(tip, [x0 + 0.55 * len, y0 + hh], bot, 30));
    p.extend(q(bot, [x0 + 0.35 * len, y0 + hh / 2.0], top, 30));
    p.push([x0 + 3.0, y0 + 4.0]);
    p
}

fn bubble(c: P, r: f32) -> Vec<P> {
    ellipse_path(c, r, r, 0.0, 0.0, 1.05)
}

#[test]
fn gates_need_a_body_and_context() {
    let mut rng = Rng(61);
    let d = h(&d_shape(300.0, 300.0, 160.0, 160.0), &mut rng);
    let pin = h(&[[240.0, 340.0], [302.0, 340.0]], &mut rng);
    // D alone: stays ink (it is a letter).
    assert_eq!(by(&d, &ctx(vec![])), None);
    // D then a pin at its back: AND.
    let p = run(&pin, &ctx(vec![raw(d.clone(), 500.0)])).expect("and");
    assert_eq!(p.recognized.kind, ShapeKind::AndGate);
    assert_eq!(p.absorbs, vec![0]);
    // Pin first, then the D.
    let p = run(&d, &ctx(vec![raw(pin.clone(), 500.0)])).expect("and");
    assert_eq!(
        (p.recognized.kind, p.onto_stroke),
        (ShapeKind::AndGate, true)
    );
    // D + bubble at the front: NAND.
    let b = h(&bubble([474.0, 380.0], 13.0), &mut rng);
    assert_eq!(
        by(&b, &ctx(vec![raw(d.clone(), 500.0)])),
        Some(ShapeKind::NandGate)
    );
    // Shield + pin: OR; + bubble: NOR; + back curve: XOR (two parts).
    let s = h(&shield(300.0, 300.0, 160.0, 180.0), &mut rng);
    let pin_or = h(&[[240.0, 340.0], [318.0, 340.0]], &mut rng);
    assert_eq!(
        by(&pin_or, &ctx(vec![raw(s.clone(), 500.0)])),
        Some(ShapeKind::OrGate)
    );
    let bo = h(&bubble([494.0, 380.0], 13.0), &mut rng);
    assert_eq!(
        by(&bo, &ctx(vec![raw(s.clone(), 500.0)])),
        Some(ShapeKind::NorGate)
    );
    let arc: Vec<P> = (0..=30)
        .map(|i| {
            let t = i as f32 / 30.0;
            [278.0 + 50.0 * (t * PI).sin() * 0.5, 300.0 + 160.0 * t]
        })
        .collect();
    let p = run(&h(&arc, &mut rng), &ctx(vec![raw(s.clone(), 500.0)])).expect("xor");
    assert_eq!((p.recognized.kind, p.extra.len()), (ShapeKind::XorGate, 1));
    // Triangle + bubble: NOT.
    let tri = Recognized {
        kind: ShapeKind::Triangle,
        geometry: Geometry::Polygon {
            pts: vec![[300.0, 300.0], [450.0, 380.0], [300.0, 460.0]],
        },
        confidence: 1.0,
    };
    let bn = h(&bubble([463.0, 380.0], 12.0), &mut rng);
    assert_eq!(
        by(&bn, &ctx(vec![shaped(tri.clone(), 400.0)])),
        Some(ShapeKind::NotGate)
    );
    // A triangle with a far stroke, or a "Do" written big: not gates.
    let o = h(&letter("o", 70.0, [470.0, 420.0]), &mut rng);
    assert_ne!(
        by(&o, &ctx(vec![raw(d.clone(), 300.0)])).map(|k| logic::gate_kind(k).is_some()),
        Some(true)
    );
    let far = h(&bubble([700.0, 380.0], 12.0), &mut rng);
    assert_ne!(
        by(&far, &ctx(vec![shaped(tri, 400.0)])),
        Some(ShapeKind::NotGate)
    );
    // Hold-to-snap asks for it: a D alone becomes AND.
    let mut held = ctx(vec![]);
    held.opts.deliberate = true;
    assert_eq!(by(&d, &held), Some(ShapeKind::AndGate));
}

// --------------------------------------------------------------- geometry

#[test]
fn an_arc_in_a_corner_becomes_an_angle_mark() {
    let mut rng = Rng(71);
    let v = [300.0, 500.0];
    let l1 = shaped(line_shape(v, [700.0, 500.0]), 3000.0);
    let l2 = shaped(
        line_shape(
            v,
            [
                300.0 + 400.0 * (PI / 4.0).cos(),
                500.0 - 400.0 * (PI / 4.0).sin(),
            ],
        ),
        2000.0,
    );
    let arc: Vec<P> = (0..=24)
        .map(|i| {
            let a = -PI / 4.0 * i as f32 / 24.0;
            [v[0] + 80.0 * a.cos(), v[1] + 80.0 * a.sin()]
        })
        .collect();
    let p = run(&h(&arc, &mut rng), &ctx(vec![l1.clone(), l2.clone()])).expect("angle");
    assert_eq!(p.recognized.kind, ShapeKind::Angle);
    assert_eq!(p.labels[0].text, "45°");
    // Far from the corner: just ink.
    let far: Vec<P> = arc.iter().map(|q| [q[0] + 250.0, q[1] - 150.0]).collect();
    assert_ne!(
        by(&h(&far, &mut rng), &ctx(vec![l1.clone(), l2.clone()])),
        Some(ShapeKind::Angle)
    );
    // A right-angle mark at a square corner.
    let up = shaped(line_shape(v, [300.0, 100.0]), 2000.0);
    let mark = h(&[[340.0, 500.0], [340.0, 460.0], [300.0, 460.0]], &mut rng);
    let p = run(&mark, &ctx(vec![l1.clone(), up])).expect("right angle");
    assert_eq!(p.recognized.kind, ShapeKind::RightAngle);
    // The same mark at a 45° corner is not one.
    assert_ne!(by(&mark, &ctx(vec![l1, l2])), Some(ShapeKind::RightAngle));
}

#[test]
fn lines_near_lines_snap_parallel_or_perpendicular() {
    let mut rng = Rng(72);
    let base = shaped(line_shape([100.0, 100.0], [500.0, 160.0]), 2000.0);
    let phi = (60.0f32).atan2(400.0);
    let near = h(&[[100.0, 300.0], [500.0, 372.0]], &mut rng);
    let p = run(&near, &ctx(vec![base.clone()])).expect("line");
    let Geometry::Segment { a, b } = p.recognized.geometry else {
        panic!()
    };
    assert!(((b[1] - a[1]).atan2(b[0] - a[0]) - phi).abs() < 1e-3);
    // Ten degrees off: left as drawn.
    let off = h(
        &[[100.0, 300.0], [500.0, 300.0 + 400.0 * (phi + 0.17).tan()]],
        &mut rng,
    );
    let p = run(&off, &ctx(vec![base])).expect("line");
    let Geometry::Segment { a, b } = p.recognized.geometry else {
        panic!()
    };
    assert!(((b[1] - a[1]).atan2(b[0] - a[0]) - phi).abs() > 0.1);
}

// ---------------------------------------------------------------- science

fn resistor_path(x0: f32, y: f32, len: f32, teeth: usize, amp: f32) -> Vec<P> {
    let lead = 0.22 * len;
    let mut p = vec![[x0, y], [x0 + lead, y]];
    let zig = len - 2.0 * lead;
    for k in 0..teeth {
        let t = (k as f32 + 0.5) / teeth as f32;
        p.push([x0 + lead + t * zig, y + if k % 2 == 0 { -amp } else { amp }]);
    }
    p.push([x0 + len - lead, y]);
    p.push([x0 + len, y]);
    p
}

#[test]
fn circuit_symbols() {
    let mut rng = Rng(81);
    let r = h(&resistor_path(200.0, 400.0, 360.0, 6, 22.0), &mut rng);
    assert_eq!(by(&r, &ctx(vec![])), Some(ShapeKind::Resistor));
    // Off by default.
    assert_ne!(
        Registry::standard()
            .recognize(&sp(&r), &Context::default(), true)
            .map(|p| p.recognized.kind),
        Some(ShapeKind::Resistor)
    );
    // A zig-zag without leads (scratch-out) or a wave: not a resistor.
    let bare = h(&resistor_path(200.0, 400.0, 360.0, 6, 22.0)[1..9], &mut rng);
    assert_ne!(by(&bare, &ctx(vec![])), Some(ShapeKind::Resistor));
    let wave: Vec<P> = (0..=200)
        .map(|i| {
            let t = i as f32 / 200.0;
            [
                200.0 + 360.0 * t,
                400.0
                    + if (0.2..0.8).contains(&t) {
                        22.0 * ((t - 0.2) * 5.0 * TAU).sin()
                    } else {
                        0.0
                    },
            ]
        })
        .collect();
    assert_ne!(by(&wave, &ctx(vec![])), Some(ShapeKind::Resistor));
    // Cell: long plate, then a short one beside it.
    let long = h(&[[400.0, 300.0], [400.0, 420.0]], &mut rng);
    let short = h(&[[432.0, 330.0], [432.0, 390.0]], &mut rng);
    let p = run(&short, &ctx(vec![raw(long.clone(), 600.0)])).expect("cell");
    assert_eq!((p.recognized.kind, p.extra.len()), (ShapeKind::Cell, 1));
    // Equal plates (‖ or =) are not a cell.
    let same = h(&[[432.0, 300.0], [432.0, 420.0]], &mut rng);
    assert_ne!(
        by(&same, &ctx(vec![raw(long, 600.0)])),
        Some(ShapeKind::Cell)
    );
    // Bulb: a circle, then an X.
    let c = shaped(circle_shape([600.0, 400.0], 60.0), 1500.0);
    let d1 = h(&[[558.0, 358.0], [642.0, 442.0]], &mut rng);
    let d2 = h(&[[642.0, 358.0], [558.0, 442.0]], &mut rng);
    let p = run(&d2, &ctx(vec![c.clone(), raw(d1.clone(), 500.0)])).expect("bulb");
    assert_eq!(p.recognized.kind, ShapeKind::Bulb);
    assert_eq!(p.absorbs.len(), 2);
    // A "+" in a circle is not a bulb.
    let p1 = h(&[[540.0, 400.0], [660.0, 400.0]], &mut rng);
    let p2 = h(&[[600.0, 340.0], [600.0, 460.0]], &mut rng);
    assert_ne!(
        by(&p2, &ctx(vec![c, raw(p1, 500.0)])),
        Some(ShapeKind::Bulb)
    );
    // Switch: "_/" then "_".
    let bl = h(&[[200.0, 400.0], [300.0, 400.0], [380.0, 350.0]], &mut rng);
    let lead = h(&[[384.0, 401.0], [480.0, 400.0]], &mut rng);
    let p = run(&lead, &ctx(vec![raw(bl.clone(), 700.0)])).expect("switch");
    assert_eq!(p.recognized.kind, ShapeKind::Switch);
    let far = h(&[[520.0, 401.0], [600.0, 400.0]], &mut rng);
    assert_ne!(
        by(&far, &ctx(vec![raw(bl, 700.0)])),
        Some(ShapeKind::Switch)
    );
}

#[test]
fn benzene_and_equilibrium() {
    let mut rng = Rng(91);
    let hex = Recognized {
        kind: ShapeKind::Hexagon,
        geometry: Geometry::Polygon {
            pts: regular_verts([500.0, 400.0], 120.0, 6, 0.0),
        },
        confidence: 1.0,
    };
    let ring = hand(
        &ellipse_path([502.0, 398.0], 70.0, 68.0, 0.0, 0.0, 1.04),
        &mut rng,
        0.006,
        0.5,
    );
    let p = run(&ring, &ctx(vec![shaped(hex.clone(), 1000.0)])).expect("benzene");
    assert_eq!((p.recognized.kind, p.extra.len()), (ShapeKind::Benzene, 1));
    // Kekulé: one inner line parallel to a side gives three.
    let v = regular_verts([500.0, 400.0], 120.0, 6, 0.0);
    let pull = |q: P| [q[0] + (500.0 - q[0]) * 0.2, q[1] + (400.0 - q[1]) * 0.2];
    let (a, b) = (pull(v[0]), pull(v[1]));
    let inner = h(
        &[
            [a[0] + 0.15 * (b[0] - a[0]), a[1] + 0.15 * (b[1] - a[1])],
            [b[0] + 0.15 * (a[0] - b[0]), b[1] + 0.15 * (a[1] - b[1])],
        ],
        &mut rng,
    );
    let p = run(&inner, &ctx(vec![shaped(hex.clone(), 1000.0)])).expect("kekulé");
    assert_eq!(p.extra.len(), 3);
    // A circle off centre is not benzene.
    let off = hand(
        &ellipse_path([560.0, 400.0], 70.0, 68.0, 0.0, 0.0, 1.04),
        &mut rng,
        0.006,
        0.5,
    );
    assert_ne!(
        by(&off, &ctx(vec![shaped(hex, 1000.0)])),
        Some(ShapeKind::Benzene)
    );
    // ⇌: an arrow right, then a harpoon left just below it.
    let right = shaped(
        Recognized {
            kind: ShapeKind::Arrow,
            geometry: Geometry::Segment {
                a: [300.0, 300.0],
                b: [500.0, 300.0],
            },
            confidence: 1.0,
        },
        800.0,
    );
    let left = h(&[[500.0, 330.0], [300.0, 330.0], [330.0, 348.0]], &mut rng);
    let p = run(&left, &ctx(vec![right.clone()])).expect("equilibrium");
    assert_eq!(
        (p.recognized.kind, p.extra.len()),
        (ShapeKind::Equilibrium, 1)
    );
    // Same direction: not ⇌.
    let same = h(&[[300.0, 330.0], [500.0, 330.0], [470.0, 348.0]], &mut rng);
    assert_ne!(by(&same, &ctx(vec![right])), Some(ShapeKind::Equilibrium));
}

// -------------------------------------------------------------- flowchart

fn stadium_path(c: P, w: f32, hh: f32) -> Vec<P> {
    let r = hh / 2.0;
    let half = w / 2.0 - r;
    let mut p = vec![[c[0] - half, c[1] - r], [c[0] + half, c[1] - r]];
    for i in 1..=30 {
        let a = -PI / 2.0 + PI * i as f32 / 30.0;
        p.push([c[0] + half + r * a.cos(), c[1] + r * a.sin()]);
    }
    p.push([c[0] - half, c[1] + r]);
    for i in 1..=30 {
        let a = PI / 2.0 + PI * i as f32 / 30.0;
        p.push([c[0] - half + r * a.cos(), c[1] + r * a.sin()]);
    }
    p.push([c[0] - half + 10.0, c[1] - r]);
    p
}

#[test]
fn flowchart_terminator_and_input_output() {
    let mut rng = Rng(101);
    let st = h(&stadium_path([500.0, 400.0], 320.0, 120.0), &mut rng);
    assert_eq!(by(&st, &ctx(vec![])), Some(ShapeKind::Terminator));
    // An oval stays an ellipse.
    let ov = hand(
        &ellipse_path([500.0, 400.0], 160.0, 70.0, 0.0, 0.0, 1.03),
        &mut rng,
        0.005,
        0.5,
    );
    assert_eq!(by(&ov, &ctx(vec![])), Some(ShapeKind::Ellipse));
    let pg = h(
        &poly_path(
            &[
                [360.0, 300.0],
                [700.0, 300.0],
                [640.0, 460.0],
                [300.0, 460.0],
            ],
            0.05,
        ),
        &mut rng,
    );
    assert_eq!(by(&pg, &ctx(vec![])), Some(ShapeKind::Parallelogram));
    // Off: the parallelogram is not snapped at all.
    let mut off = ctx(vec![]);
    off.opts.families = family::DEFAULT & !family::FLOWCHART;
    assert_eq!(by(&pg, &off), None);
}

// ----------------------------------------------------------------- stress

/// Rates over `n` random hands: (positives recognised as intended, near
/// misses that stayed clear of the family).
fn rates(n: usize, seed: u64) -> Vec<(&'static str, f32, f32)> {
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    let mut tally = |name: &'static str,
                     pos: &mut dyn FnMut(&mut Rng) -> bool,
                     neg: &mut dyn FnMut(&mut Rng) -> bool,
                     rng: &mut Rng| {
        let (mut p, mut q) = (0, 0);
        for _ in 0..n {
            p += usize::from(pos(rng));
            q += usize::from(neg(rng));
        }
        out.push((name, p as f32 / n as f32, q as f32 / n as f32));
    };
    // Axes from a "+": random sizes, origins and tilts; near miss: a cross
    // at 30–60° or a "T".
    tally(
        "axes +",
        &mut |r| {
            let o = [r.range(300.0, 600.0), r.range(300.0, 600.0)];
            let (l, rr, t, b, tilt) = (
                r.range(40.0, 300.0),
                r.range(200.0, 400.0),
                r.range(200.0, 400.0),
                r.range(40.0, 300.0),
                r.range(-0.12, 0.12),
            );
            let (x, y) = axes_plus(r, o, l, rr, t, b, tilt);
            by(&y, &ctx(vec![raw(x, r.range(100.0, 3000.0) as f64)])) == Some(ShapeKind::Axes)
        },
        &mut |r| {
            let o = [r.range(300.0, 600.0), r.range(300.0, 600.0)];
            let tilt = r.range(0.5, 1.05);
            let (x, y) = if r.next() < 0.5 {
                axes_plus(r, o, 250.0, 250.0, 250.0, 250.0, tilt)
            } else {
                let x = h(&[[o[0] - 250.0, o[1]], [o[0] + 250.0, o[1]]], r);
                let y = h(&[[o[0], o[1]], [o[0], o[1] + 300.0]], r);
                (x, y)
            };
            by(&y, &ctx(vec![raw(x, 500.0)])) != Some(ShapeKind::Axes)
        },
        &mut rng,
    );
    // Curves over axes (each family); near miss: scribbles, loops, words.
    let (ax, f) = axes_neighbour();
    tally(
        "curve fit",
        &mut |r| {
            let k = (r.next() * 4.0) as usize;
            let (a, b) = (
                r.range(0.5, 2.0) * if r.next() < 0.5 { -1.0 } else { 1.0 },
                r.range(-1.5, 1.5),
            );
            let (fun, x0, x1, kind): (Box<dyn Fn(f32) -> f32>, f32, f32, ShapeKind) = match k {
                0 => (
                    Box::new(move |x| a * x + b),
                    -3.0,
                    3.0,
                    ShapeKind::GraphLine,
                ),
                1 => (
                    Box::new(move |x| 0.5 * a * x * x + b),
                    -2.5,
                    2.5,
                    ShapeKind::GraphQuad,
                ),
                2 => (
                    Box::new(move |x| a.abs() * 1.5 * (x * 1.2).sin() + b * 0.5),
                    -5.0,
                    9.0,
                    ShapeKind::GraphSine,
                ),
                _ => (
                    Box::new(move |x| a.abs() * 1.6f32.powf(x) - 2.0),
                    -5.0,
                    3.5,
                    ShapeKind::GraphExp,
                ),
            };
            let wob = r.range(0.002, 0.006);
            let ink = hand(&curve(&f, &*fun, x0, x1), r, wob, 0.8);
            by(&ink, &ctx(vec![ax.clone()])) == Some(kind)
        },
        &mut |r| {
            let ink: Vec<P> = match (r.next() * 3.0) as usize {
                0 => {
                    let (f1, f2) = (r.range(20.0, 45.0), r.range(5.0, 15.0));
                    (0..200)
                        .map(|i| {
                            let t = i as f32 / 200.0;
                            [
                                250.0 + 500.0 * t,
                                300.0 + 50.0 * (t * f1).sin() + 40.0 * (t * f2).cos(),
                            ]
                        })
                        .collect()
                }
                1 => hand(
                    &ellipse_path(
                        [500.0, 300.0],
                        r.range(60.0, 150.0),
                        r.range(50.0, 120.0),
                        0.0,
                        0.0,
                        1.03,
                    ),
                    r,
                    0.008,
                    0.5,
                ),
                _ => letter(
                    "a",
                    r.range(40.0, 90.0),
                    [r.range(250.0, 700.0), r.range(200.0, 600.0)],
                ),
            };
            !by(&ink, &ctx(vec![ax.clone()])).is_some_and(|k| k.as_str().starts_with("graph"))
        },
        &mut rng,
    );
    // Number lines with 4–9 ticks; near miss: uneven ticks.
    tally(
        "number line",
        &mut |r| {
            let k = 4 + (r.next() * 6.0) as usize;
            let g = r.range(50.0, 90.0);
            let xs: Vec<f32> = (0..k)
                .map(|i| 500.0 + (i as f32 - k as f32 / 2.0) * g + r.range(-4.0, 4.0))
                .collect();
            let (line, ticks) = number_line(r, &xs, 400.0);
            let mut n = vec![raw(line, 9000.0)];
            n.extend(ticks[..k - 1].iter().map(|t| raw(t.clone(), 2000.0)));
            deferred(&ticks[k - 1], &ctx(n))
                .is_some_and(|p| p.recognized.kind == ShapeKind::NumberLine)
        },
        &mut |r| {
            let xs: Vec<f32> = [300.0, 360.0, 470.0, 520.0, 650.0]
                .iter()
                .map(|x| x + r.range(-10.0, 10.0))
                .collect();
            let (line, ticks) = number_line(r, &xs, 400.0);
            let mut n = vec![raw(line, 9000.0)];
            n.extend(ticks[..4].iter().map(|t| raw(t.clone(), 2000.0)));
            deferred(&ticks[4], &ctx(n)).is_none()
        },
        &mut rng,
    );
    // Gates (AND by pin, NAND by bubble); near miss: a big "D" then "o".
    tally(
        "gates",
        &mut |r| {
            let hh = r.range(90.0, 220.0);
            let dd = r.range(0.8, 1.1);
            let d = h(&d_shape(300.0, 300.0, hh, hh * dd), r);
            let b = h(
                &bubble([300.0 + hh * dd + 0.1 * hh, 300.0 + hh / 2.0], 0.09 * hh),
                r,
            );
            matches!(by(&b, &ctx(vec![raw(d, 400.0)])), Some(ShapeKind::NandGate))
        },
        &mut |r| {
            let hh = r.range(90.0, 220.0);
            let d = h(&d_shape(300.0, 300.0, hh, hh * 0.75), r);
            let o = h(
                &letter(
                    "o",
                    hh * 0.55,
                    [300.0 + hh * 0.75 + 0.3 * hh, 300.0 + 0.72 * hh],
                ),
                r,
            );
            !by(&o, &ctx(vec![raw(d, 300.0)])).is_some_and(|k| logic::gate_kind(k).is_some())
        },
        &mut rng,
    );
    // Venn of two; near miss: a circle inside a circle.
    tally(
        "venn",
        &mut |r| {
            let r0 = r.range(90.0, 180.0);
            let a = shaped(circle_shape([400.0, 400.0], r0), 2000.0);
            let d = r0 * r.range(0.6, 1.4);
            let th = r.range(-0.2, 0.2);
            let ink = hand(
                &ellipse_path(
                    [400.0 + d * th.cos(), 400.0 + d * th.sin()],
                    r0 * r.range(0.9, 1.1),
                    r0,
                    0.0,
                    0.0,
                    1.03,
                ),
                r,
                0.006,
                0.5,
            );
            by(&ink, &ctx(vec![a])) == Some(ShapeKind::Venn)
        },
        &mut |r| {
            let r0 = r.range(120.0, 180.0);
            let a = shaped(circle_shape([400.0, 400.0], r0), 2000.0);
            let ink = hand(
                &ellipse_path(
                    [400.0 + r.range(-20.0, 20.0), 400.0],
                    r0 * r.range(0.3, 0.6),
                    r0 * 0.5,
                    0.0,
                    0.0,
                    1.03,
                ),
                r,
                0.006,
                0.5,
            );
            by(&ink, &ctx(vec![a])) != Some(ShapeKind::Venn)
        },
        &mut rng,
    );
    // Resistors; near miss: a scratch-out zig-zag (no leads).
    tally(
        "resistor",
        &mut |r| {
            let p = resistor_path(
                200.0,
                400.0,
                r.range(200.0, 450.0),
                4 + (r.next() * 4.0) as usize,
                r.range(15.0, 35.0),
            );
            let p = rotate(&p, [300.0, 400.0], r.range(-0.12, 0.12));
            by(&h(&p, r), &ctx(vec![])) == Some(ShapeKind::Resistor)
        },
        &mut |r| {
            let z = zigzag(
                [400.0, 400.0],
                r.range(100.0, 300.0),
                r.range(30.0, 80.0),
                6 + (r.next() * 6.0) as usize,
                r.next() < 0.5,
            );
            by(&h(&z, r), &ctx(vec![])) != Some(ShapeKind::Resistor)
        },
        &mut rng,
    );
    // Every family on, nothing around: letters (any size) and zig-zag
    // scribbles stay ink.
    tally(
        "letters, all on",
        &mut |r| {
            let name = ["o", "0", "D", "a", "৪", "ব", "8"][(r.next() * 7.0) as usize % 7];
            let ink = h(&letter(name, r.range(40.0, 280.0), [400.0, 400.0]), r);
            by(&ink, &ctx(vec![])).is_none_or(|k| {
                k.as_str() != "and-gate" && k.as_str() != "resistor" && k.as_str() != "axes"
            })
        },
        &mut |r| {
            let z = zigzag(
                [400.0, 400.0],
                r.range(60.0, 300.0),
                r.range(20.0, 80.0),
                5 + (r.next() * 8.0) as usize,
                r.next() < 0.5,
            );
            by(&h(&z, r), &ctx(vec![])).is_none()
        },
        &mut rng,
    );
    out
}

#[test]
fn stress_phase2_quick() {
    for (name, pos, neg) in rates(30, 7) {
        assert!(pos >= 0.85, "{name}: only {:.0}% recognised", pos * 100.0);
        assert!(
            neg >= 1.0,
            "{name}: {:.0}% near misses taken",
            (1.0 - neg) * 100.0
        );
    }
}

#[test]
#[ignore = "tuning harness: cargo test -p rupok-ink --release stress_phase2 -- --ignored --nocapture"]
fn stress_phase2() {
    for (name, pos, neg) in rates(200, 99) {
        println!(
            "{name:>14}: {:5.1}% recognised, {:5.1}% near misses stay ink",
            pos * 100.0,
            neg * 100.0
        );
    }
}
