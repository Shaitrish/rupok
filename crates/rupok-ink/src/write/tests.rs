use super::layout::{Layout, Pause, Variation};
use super::parse::parse_line;
use super::*;
use crate::math::geom::BBox;

fn opts() -> WriteOptions {
    WriteOptions {
        x_height: 20.0,
        ..WriteOptions::default()
    }
}

fn bbox_of(strokes: &[PenStroke]) -> BBox {
    let mut b = BBox::EMPTY;
    for s in strokes {
        for p in &s.points {
            b.add(Pt::new(p.x, p.y));
        }
    }
    b
}

#[test]
fn every_symbol_an_ai_writes_has_a_glyph() {
    let g = GlyphSet::new();
    let text: String = (33u8..127).map(char::from).collect();
    for c in text.chars() {
        assert!(g.has(c), "missing {c:?}");
    }
    for c in "αβγδεθλμπσφωΔΣΩΠΘ×÷·±∓≤≥≠≈≡→←∞°∂∇∫∠∴⊥−…′".chars()
    {
        assert!(g.has(c), "missing {c:?}");
    }
    let w = write(
        &["$\\alpha + \\beta \\le \\pi \\times 2 \\ne \\infty$, 30° ∴ x ≈ 1"],
        &g,
        &opts(),
    );
    assert!(w.missing.is_empty(), "{:?}", w.missing);
    let w = write(&["ক"], &g, &opts());
    assert_eq!(w.missing, vec!['ক'], "unknown characters are reported");
}

#[test]
fn layout_boxes_follow_maths_structure() {
    let g = GlyphSet::new();
    let mut lay = Layout {
        glyphs: &g,
        var: Variation {
            jitter: 0.0,
            prefer_user: false,
            connect: false,
        },
        rng: crate::math::synth::Rng::new(1),
        missing: Vec::new(),
        drift: 0.0,
    };
    // Fraction: numerator ink above the bar, denominator below.
    let row = lay.row(&parse_line("$\\frac{12}{3}$"), 1.0);
    let bar = row
        .strokes
        .iter()
        .find(|s| s.pts.len() == 3 && (s.pts[0].y - s.pts[2].y).abs() < 1e-3)
        .expect("a fraction bar");
    let bar_y = bar.pts[0].y;
    let bar_w = bar.pts[2].x - bar.pts[0].x;
    let n = row
        .strokes
        .iter()
        .position(|s| std::ptr::eq(s, bar))
        .unwrap();
    let num = BBox::of_strokes(row.strokes[..n].iter().map(|s| s.pts.as_slice()));
    let den = BBox::of_strokes(row.strokes[n + 1..].iter().map(|s| s.pts.as_slice()));
    assert!(
        num.y1 < bar_y && den.y0 > bar_y,
        "{num:?} / {bar_y} / {den:?}"
    );
    assert!(bar_w >= num.w() && bar_w >= den.w(), "the bar covers both");
    assert!(bar_y < 0.0 && bar_y > -XH, "the bar sits on the maths axis");

    // Superscript: raised and smaller.
    let base = lay.row(&parse_line("x"), 1.0).ink();
    let sq = lay.row(&parse_line("$x^2$"), 1.0);
    let two = BBox::of_strokes(sq.strokes[2..].iter().map(|s| s.pts.as_slice()));
    assert!(two.y1 < base.y1 - 4.0, "raised: {two:?} vs {base:?}");
    assert!(two.h() < 21.0 * 0.8, "smaller");
    assert!(two.x0 > base.x1 - 1.0, "to the right of the base");
    // Subscript: lowered.
    let sb = lay.row(&parse_line("$x_2$"), 1.0);
    let two = BBox::of_strokes(sb.strokes[2..].iter().map(|s| s.pts.as_slice()));
    assert!(two.y1 > 2.0, "lowered: {two:?}");

    // Root: the sign's overbar spans the radicand, which sits under it.
    let r = lay.row(&parse_line("$\\sqrt{49}$"), 1.0);
    let sign = &r.strokes[0];
    let bar_end = sign.pts.last().unwrap();
    let body = BBox::of_strokes(r.strokes[1..].iter().map(|s| s.pts.as_slice()));
    assert!(bar_end.x >= body.x1, "overbar reaches past the radicand");
    assert!(bar_end.y < body.y0, "overbar above the radicand");
    assert!(sign.pts[0].x < body.x0);

    // Spaced operators take more room than plain juxtaposition.
    let tight = lay.row(&parse_line("ab"), 1.0).w;
    let spaced = lay.row(&parse_line("$a+b$"), 1.0).w;
    let plus = lay.row(&parse_line("+"), 1.0).w;
    assert!(spaced > tight + plus);
}

#[test]
fn lines_stack_and_wrap() {
    let g = GlyphSet::new();
    let w = write(
        &["$2x + 3 = 7$", "$2x = 4$", "$x = \\frac{4}{2} = 2$"],
        &g,
        &opts(),
    );
    assert_eq!(w.lines.len(), 3);
    for pair in w.lines.windows(2) {
        assert!(pair[1].baseline > pair[0].baseline);
        assert!(pair[1].y0 >= pair[0].y1 - 1.0, "no overlap: {pair:?}");
    }
    assert!(w.width > 0.0 && w.height > 0.0);
    let b = bbox_of(&w.strokes);
    assert!(
        b.x0 >= -0.01 && b.y0 >= -0.01,
        "starts at the origin: {b:?}"
    );
    assert!(
        b.x1 <= w.width + 0.5 && b.y1 <= w.height + 0.5,
        "{b:?} in {}×{}",
        w.width,
        w.height
    );
    // Every stroke knows its step.
    assert!(w.strokes.iter().any(|s| s.line == 2));
    let starts = w.line_starts(3);
    assert!(starts[0] < starts[1] && starts[1] < starts[2]);

    // A long line wraps at spaces within max_width.
    let long = "Subtract three from both sides of the equation to keep it balanced";
    let narrow = WriteOptions {
        max_width: Some(300.0),
        ..opts()
    };
    let w = write(&[long], &g, &narrow);
    assert!(w.lines.len() >= 3, "{}", w.lines.len());
    assert!(w.lines.iter().all(|l| l.line == 0));
    assert!(w.width <= 300.0 + 40.0, "{}", w.width);
}

#[test]
fn deterministic_with_seed() {
    let g = GlyphSet::new();
    let lines = ["$x^2 - 5x + 6 = 0$", "$(x-2)(x-3) = 0$"];
    let a = write(&lines, &g, &opts());
    let b = write(&lines, &g, &opts());
    assert_eq!(a, b);
    let c = write(&lines, &g, &WriteOptions { seed: 99, ..opts() });
    assert_ne!(a.strokes, c.strokes, "another seed varies the hand");
    // ...but only gently: the block stays about the same size.
    assert!((a.width - c.width).abs() < a.width * 0.15);
    // Zero jitter gives the same ink whatever the seed (apart from timing-free shape).
    let z = |seed| {
        write(
            &lines,
            &g,
            &WriteOptions {
                jitter: 0.0,
                seed,
                ..opts()
            },
        )
    };
    assert_eq!(z(1).strokes, z(2).strokes);
}

#[test]
fn timing_is_monotonic_and_paced() {
    let g = GlyphSet::new();
    let w = write(&["Divide by 2:", "$x = 2$"], &g, &opts());
    let mut prev_end = -1.0;
    for s in &w.strokes {
        assert!(s.start_ms >= prev_end, "strokes never overlap in time");
        assert!(!s.points.is_empty());
        assert_eq!(s.points[0].dt, 0);
        for p in s.points.windows(2) {
            assert!(p[1].dt > p[0].dt, "dt strictly increasing");
        }
        for p in &s.points {
            assert!((0.0..=1.0).contains(&p.p));
        }
        prev_end = s.end_ms();
    }
    assert_eq!(w.duration_ms, prev_end);
    // A line break pauses longer than a stroke break.
    let first_of_line2 = w.strokes.iter().position(|s| s.line == 1).unwrap();
    let gap_line = w.strokes[first_of_line2].start_ms - w.strokes[first_of_line2 - 1].end_ms();
    let gap_stroke = w.strokes[1].start_ms - w.strokes[0].end_ms();
    assert!(gap_line > gap_stroke * 2.0, "{gap_line} vs {gap_stroke}");
    // Speed scales the whole performance.
    let fast = write(
        &["Divide by 2:", "$x = 2$"],
        &g,
        &WriteOptions {
            speed: 2.0,
            ..opts()
        },
    );
    assert!(fast.duration_ms < w.duration_ms * 0.65);
    // Realistic pace: a dozen characters take a few seconds, not minutes.
    assert!(
        w.duration_ms > 1500.0 && w.duration_ms < 9000.0,
        "{}",
        w.duration_ms
    );
}

#[test]
fn teacher_glyphs_are_used_when_present() {
    let mut g = GlyphSet::new();
    // A teacher's "x" as a single zigzag, unlike the Hershey two-stroke x.
    let zig = vec![vec![
        Pt::new(0.0, -14.0),
        Pt::new(10.0, 0.0),
        Pt::new(10.0, -14.0),
        Pt::new(0.0, 0.0),
    ]];
    assert!(g.add_user('x', &zig));
    let teacher = write(&["x"], &g, &opts());
    let neat = write(
        &["x"],
        &g,
        &WriteOptions {
            prefer_user: false,
            ..opts()
        },
    );
    assert_eq!(teacher.strokes.len(), 1);
    assert_eq!(neat.strokes.len(), 2);
}

#[test]
fn normalise_maps_guides_to_writing_units() {
    let s = normalise_sample(
        &[vec![Pt::new(0.0, 100.0), Pt::new(10.0, 30.0)]],
        100.0,
        30.0,
    );
    assert_eq!(s[0][0], Pt::new(0.0, 0.0));
    assert!((s[0][1].y + XH).abs() < 1e-4);
    assert!(normalise_sample(&[vec![Pt::new(0.0, 0.0)]], 10.0, 20.0).is_empty());
}

#[test]
fn hostile_options_are_clamped() {
    let g = GlyphSet::new();
    let w = write(
        &["$x$"],
        &g,
        &WriteOptions {
            x_height: f32::NAN,
            slant_deg: 1e9,
            jitter: f32::INFINITY,
            speed: 0.0,
            seed: 0,
            max_width: Some(-1.0),
            line_pitch: f32::NAN,
            prefer_user: true,
            connect: true,
        },
    );
    assert!(!w.strokes.is_empty());
    let many: Vec<String> = (0..100).map(|i| i.to_string()).collect();
    assert!(write(&many, &g, &opts()).lines.len() <= MAX_LINES);
    let empty: [&str; 0] = [];
    assert!(write(&empty, &g, &opts()).strokes.is_empty());
}

// ------------------------------------------------- handwriting quality

#[test]
fn letters_advance_left_to_right_with_even_gaps() {
    let g = GlyphSet::new();
    for word in ["minimum", "Subtract", "quick", "x+y=12", "hello world"] {
        let w = write(&[word], &g, &opts());
        // Left edges of successive strokes' glyphs never jump backwards by
        // more than a letter: the writing moves left to right.
        let mut starts: Vec<f32> = Vec::new();
        for s in &w.strokes {
            let x0 = s.points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            starts.push(x0);
        }
        let xh = opts().x_height;
        let mut max_so_far = f32::NEG_INFINITY;
        for x in &starts {
            assert!(
                *x >= max_so_far - xh * 1.2,
                "{word}: a stroke starts {x} far left of {max_so_far}"
            );
            max_so_far = max_so_far.max(*x);
        }
    }
    // Letter gaps are even: ink-to-ink spacing within a word varies little.
    let lay_word = |word: &str| {
        let g = GlyphSet::new();
        let mut lay = Layout {
            glyphs: &g,
            var: Variation {
                jitter: 0.0,
                prefer_user: false,
                connect: false,
            },
            rng: crate::math::synth::Rng::new(1),
            missing: Vec::new(),
            drift: 0.0,
        };
        let nodes = parse_line(word);
        let mut gaps = Vec::new();
        let mut prev: Option<BBox> = None;
        let full = lay.row(&nodes, 1.0);
        let mut boxes: Vec<BBox> = Vec::new();
        for s in &full.strokes {
            let b = BBox::of(&s.pts);
            match boxes.last_mut() {
                Some(last) if s.pause == Pause::Stroke => *last = last.union(b),
                _ => boxes.push(b),
            }
        }
        for b in &boxes {
            if let Some(p) = prev {
                gaps.push(b.x0 - p.x1);
            }
            prev = Some(*b);
        }
        gaps
    };
    let gaps = lay_word("minimum");
    assert!(!gaps.is_empty());
    for g in &gaps {
        assert!(*g > 1.0 && *g < 7.0, "letter gap {g} in {gaps:?}");
    }
}

#[test]
fn writing_stays_inside_its_block_and_width() {
    let g = GlyphSet::new();
    for connect in [false, true] {
        let lines = [
            "Subtract 3 from both sides of the equation, then divide by two",
            "$x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$",
            "Check: $2(2) + 3 = 7$",
        ];
        let o = WriteOptions {
            max_width: Some(420.0),
            connect,
            ..opts()
        };
        let w = write(&lines, &g, &o);
        let b = bbox_of(&w.strokes);
        assert!(b.x0 >= -0.01 && b.y0 >= -0.01, "{b:?}");
        assert!(b.x1 <= w.width + 0.5, "ink {} past width {}", b.x1, w.width);
        assert!(
            b.y1 <= w.height + 0.5,
            "ink {} past height {}",
            b.y1,
            w.height
        );
        // Wrapping keeps lines near the asked width (one long word may exceed).
        assert!(w.width < 420.0 + 60.0, "{}", w.width);
        for s in &w.strokes {
            for p in &s.points {
                assert!(p.x.is_finite() && p.y.is_finite());
                assert!((0.0..=1.0).contains(&p.p));
            }
        }
    }
}

#[test]
fn pen_lifts_like_a_hand() {
    let g = GlyphSet::new();
    // `v` and `w` are one movement each; `n` and `a` retrace their stem.
    for c in ['v', 'w', 'n', 'a', 'd', 'm', 'u'] {
        let gl = g.builtin(c).unwrap();
        assert_eq!(gl.strokes.len(), 1, "{c}: {} strokes", gl.strokes.len());
    }
    // The dot of an i comes after its stem.
    let i = g.builtin('i').unwrap();
    assert_eq!(i.strokes.len(), 2);
    assert!(BBox::of(&i.strokes[1]).size() < 2.5, "dot last");
    // The joined style lifts the pen less often than neat print.
    let neat = write(&["minimum quantum"], &g, &opts());
    let joined = write(
        &["minimum quantum"],
        &g,
        &WriteOptions {
            connect: true,
            ..opts()
        },
    );
    assert!(
        joined.strokes.len() < neat.strokes.len(),
        "joined {} vs neat {}",
        joined.strokes.len(),
        neat.strokes.len()
    );
    // Same input, same ink, in both styles.
    assert_eq!(
        joined,
        write(
            &["minimum quantum"],
            &g,
            &WriteOptions {
                connect: true,
                ..opts()
            }
        )
    );
}

#[test]
fn neighbouring_letters_share_a_baseline() {
    // The baseline wanders slowly: neighbours differ far less than the
    // whole line's spread would allow with independent jitter.
    let g = GlyphSet::new();
    let w = write(&["nnnnnnnnnnnnnnnnnnnn"], &g, &opts());
    let bottoms: Vec<f32> = w
        .strokes
        .iter()
        .map(|s| {
            s.points
                .iter()
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max)
        })
        .collect();
    let xh = opts().x_height;
    for pair in bottoms.windows(2) {
        assert!((pair[1] - pair[0]).abs() < xh * 0.2, "{pair:?}");
    }
}
