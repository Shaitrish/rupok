//! End-to-end tests of the maths recognizer on synthetic ink.

use super::synth::{self, Synth, SynthStyle};
use super::*;

fn pts(v: &[(f32, f32)]) -> Vec<StrokePoint> {
    v.iter()
        .enumerate()
        .map(|(i, &(x, y))| StrokePoint {
            x,
            y,
            p: 0.5,
            dt: i as u32 * 8,
        })
        .collect()
}

fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<StrokePoint> {
    let v: Vec<(f32, f32)> = (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
        })
        .collect();
    pts(&v)
}

#[test]
fn clean_ink_reads_back_exactly() {
    let rec = MathRecognizer::new();
    let synth = Synth::new();
    let mut wrong = Vec::new();
    for (i, e) in synth::REQUIRED.iter().enumerate() {
        let s = synth.render(e, i as u64, &SynthStyle::clean()).unwrap();
        let got = rec.recognize(&s.strokes);
        let top = got.first().map(|c| c.latex.clone()).unwrap_or_default();
        if top != s.latex {
            wrong.push((s.latex, top));
        }
    }
    assert!(wrong.len() <= 1, "{wrong:?}");
}

#[test]
fn hand_built_plus_and_equals() {
    let rec = MathRecognizer::new();
    // "1+1=2"-ish without a "2": 1 + 1 = 1
    let strokes = vec![
        line(0.0, 0.0, 0.0, 40.0),
        line(15.0, 20.0, 40.0, 20.0),
        line(27.5, 8.0, 27.5, 32.0),
        line(55.0, 0.0, 55.0, 40.0),
        line(70.0, 14.0, 95.0, 14.0),
        line(70.0, 26.0, 95.0, 26.0),
        line(110.0, 0.0, 110.0, 40.0),
    ];
    let c = rec.recognize(&strokes);
    assert_eq!(c[0].latex, "1+1=1");
    assert_eq!(c[0].symbols.len(), 5);
    assert_eq!(c[0].symbols[1].strokes, vec![1, 2]);
    assert_eq!(c[0].symbols[3].strokes, vec![4, 5]);
}

#[test]
fn incremental_matches_batch_and_pop_undoes() {
    let mut rec = MathRecognizer::new();
    let s = Synth::new()
        .render("a^2+b^2=c^2", 3, &SynthStyle::standard())
        .unwrap();
    let batch = rec.recognize(&s.strokes);
    for st in &s.strokes {
        rec.push_stroke(st);
        let _ = rec.candidates();
    }
    assert_eq!(rec.candidates(), batch);
    let n = rec.stroke_count();
    let before = rec.recognize(&s.strokes[..n - 1]);
    assert!(rec.pop_stroke());
    assert_eq!(rec.candidates(), before);
    rec.clear();
    assert!(rec.candidates().is_empty());
    assert!(!rec.pop_stroke());
}

#[test]
fn exponent_written_last() {
    let rec = MathRecognizer::new();
    let s = Synth::new().render("x^2", 1, &SynthStyle::clean()).unwrap();
    // The "2" is the last ink; write it first instead.
    let mut strokes = s.strokes.clone();
    let two = strokes.pop().unwrap();
    strokes.insert(0, two);
    assert_eq!(rec.recognize(&strokes)[0].latex, "x^2");
}

#[test]
fn user_template_teaches_a_new_form() {
    let mut rec = MathRecognizer::new();
    // A "θ" written as a square with a cross bar: nothing built in looks so.
    let square = |ox: f32| {
        vec![
            pts(&[
                (ox, 0.0),
                (ox + 20.0, 0.0),
                (ox + 20.0, 40.0),
                (ox, 40.0),
                (ox, 0.0),
            ]),
            line(ox - 4.0, 20.0, ox + 24.0, 20.0),
        ]
    };
    rec.add_user_template("θ", &square(0.0)).unwrap();
    assert_eq!(rec.user_template_count(), 1);
    let got = rec.recognize(&square(100.0));
    assert_eq!(got[0].latex, "\\theta");
    assert_eq!(
        rec.add_user_template("nope", &square(0.0)),
        Err(MathError::UnknownSymbol("nope".into()))
    );
    assert_eq!(
        rec.add_user_template("x", &[]),
        Err(MathError::EmptyTemplate)
    );
    rec.clear_user_templates(Some("\\theta")).unwrap();
    assert_eq!(rec.user_template_count(), 0);
}

#[test]
fn hostile_input_does_not_panic() {
    let rec = MathRecognizer::new();
    assert!(rec.recognize(&[]).is_empty());
    assert!(rec.recognize(&[vec![]]).is_empty());
    let nan = StrokePoint {
        x: f32::NAN,
        y: 1.0,
        p: 0.5,
        dt: 0,
    };
    let _ = rec.recognize(&[vec![nan; 5], line(0.0, 0.0, 0.0, 10.0)]);
    let _ = rec.recognize(&[pts(&[(5.0, 5.0)])]);
    let _ = rec.recognize(&vec![pts(&[(5.0, 5.0)]); 9]);
    let huge = vec![line(0.0, 0.0, 1e30, 1e30), line(-1e30, 0.0, 1e30, 0.0)];
    let _ = rec.recognize(&huge);
    let many: Vec<Vec<StrokePoint>> = (0..400)
        .map(|i| line(i as f32 * 3.0, 0.0, i as f32 * 3.0 + 2.0, 9.0))
        .collect();
    let _ = rec.recognize(&many);
}

#[test]
fn confidence_is_a_probability_and_sorted() {
    let rec = MathRecognizer::new();
    let s = Synth::new()
        .render("y=mx+c", 5, &SynthStyle::standard())
        .unwrap();
    let c = rec.recognize(&s.strokes);
    assert!(!c.is_empty());
    let sum: f32 = c.iter().map(|x| x.confidence).sum();
    assert!(sum <= 1.0 + 1e-4);
    for w in c.windows(2) {
        assert!(w[0].confidence >= w[1].confidence - 1e-6);
    }
    for x in &c {
        for s in &x.symbols {
            assert!((0.0..=1.0).contains(&s.confidence));
        }
    }
}

/// The accuracy bar: 350 synthetic expressions (70 expressions × 5 seeds,
/// standard perturbation). The brief asks for ≥ 85 % on simple ones; this
/// guards a little above what is measured so regressions show. The full
/// report: `cargo run -p rupok-ink --release --example math_eval`.
#[test]
fn synthetic_accuracy() {
    let rec = MathRecognizer::new();
    let rep = synth::evaluate(&rec, synth::SIMPLE, 0..5, &SynthStyle::standard());
    assert!(rep.samples >= 280);
    assert!(
        rep.simple_rate() >= 0.90,
        "simple {:.3}; failures {:?}",
        rep.simple_rate(),
        &rep.failures[..rep.failures.len().min(20)]
    );
    let rep = synth::evaluate(&rec, synth::STRUCTURED, 0..5, &SynthStyle::standard());
    assert!(
        rep.structured_rate() >= 0.85,
        "structured {:.3}; failures {:?}",
        rep.structured_rate(),
        rep.failures
    );
}

#[test]
#[ignore]
fn debug_dump() {
    let e = std::env::var("EXPR").unwrap_or("\\frac{1}{2}".into());
    let seed: u64 = std::env::var("SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let style = match std::env::var("STYLE").as_deref() {
        Ok("standard") => SynthStyle::standard(),
        Ok("hard") => SynthStyle::hard(),
        _ => SynthStyle::clean(),
    };
    let s = Synth::new().render(&e, seed, &style).unwrap();
    println!("truth {} symbols {:?}", s.latex, s.symbols);
    let raw: Vec<Vec<geom::Pt>> = s
        .strokes
        .iter()
        .map(|st| st.iter().map(|p| geom::Pt::new(p.x, p.y)).collect())
        .collect();
    let rec = MathRecognizer::new();
    let p = segment::prepare(&raw);
    println!("unit {} slant {} seq {:?}", p.unit, p.slant_key, p.seq);
    for seg in segment::segment(&p, &rec.lib, None, 4) {
        print!("cost {:.2}:", seg.cost);
        for g in &seg.groups {
            print!(
                " {:?}{}({:.3})",
                g.strokes,
                symbols::sym(g.guesses[0].sym).name,
                g.guesses[0].dist
            );
        }
        println!();
    }
    for (name, ids) in &s.symbols {
        let refs: Vec<&[geom::Pt]> = ids.iter().map(|&i| p.strokes[i].as_slice()).collect();
        let g = rec.lib.classify(&refs, 4);
        let tops: Vec<String> = g
            .iter()
            .take(4)
            .map(|x| format!("{}{:.4}", symbols::sym(x.sym).name, x.dist))
            .collect();
        println!(
            "truth {name} {ids:?}: {}  admissible {:?}",
            tops.join(" "),
            segment::admissible(&p, ids)
        );
        if segment::admissible(&p, ids).is_none() {
            for &i in ids {
                println!("   stroke {i}: box {:?} dot {}", p.boxes[i], p.is_dot(i));
            }
        }
    }
    for c in rec.recognize(&s.strokes) {
        println!("{:.3} {}", c.confidence, c.latex);
    }
}
