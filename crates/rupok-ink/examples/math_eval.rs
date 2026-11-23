//! Accuracy and speed report for the handwritten-maths recognizer.
//!
//! ```sh
//! cargo run -p rupok-ink --release --example math_eval            # full report
//! cargo run -p rupok-ink --release --example math_eval -- --quick # fewer seeds
//! ```
//!
//! The test set is synthetic (see `rupok_ink::math::synth`): every
//! expression in `SIMPLE` and `STRUCTURED` rendered with several seeds.

use std::time::Instant;

use rupok_ink::math::MathRecognizer;
use rupok_ink::math::synth::{self, Report, Synth, SynthStyle};

fn pct(a: usize, b: usize) -> String {
    format!("{:5.1}% ({a}/{b})", 100.0 * a as f32 / b.max(1) as f32)
}

fn print_report(name: &str, rep: &Report, verbose: bool) {
    println!("\n## {name}\n");
    println!("expressions         {}", rep.samples);
    println!("expression top-1    {}", pct(rep.correct, rep.samples));
    println!("  simple            {}", pct(rep.simple.0, rep.simple.1));
    println!(
        "  structured        {}",
        pct(rep.structured.0, rep.structured.1)
    );
    println!("expression in top-5 {}", pct(rep.in_n_best, rep.samples));
    println!("symbols grouped     {}", pct(rep.segmented, rep.symbols));
    println!(
        "symbols correct     {}",
        pct(rep.symbols_correct, rep.symbols)
    );
    println!("confusions (truth → read, count):");
    for ((t, r), n) in rep.confusions.iter().take(15) {
        println!("  {t} → {r}: {n}");
    }
    println!("calibration (top-1 confidence bucket: correct/count):");
    for (i, (n, ok)) in rep.by_confidence.iter().enumerate() {
        if *n > 0 {
            println!(
                "  {:.1}–{:.1}: {}",
                i as f32 / 10.0,
                (i + 1) as f32 / 10.0,
                pct(*ok, *n)
            );
        }
    }
    if verbose {
        println!("per expression:");
        for (e, ok, n) in &rep.per_expr {
            println!("  {:<28} {ok}/{n}", e);
        }
        println!("failures (truth ⇒ read):");
        for (t, g, seed) in rep.failures.iter().take(60) {
            println!("  {t}  ⇒  {g}   (seed {seed})");
        }
    }
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

fn main() {
    let quick = std::env::args().any(|a| a == "--quick");
    let speed_only = std::env::args().any(|a| a == "--speed");
    let verbose = std::env::args().any(|a| a == "--verbose");
    let seeds = if quick { 0..2 } else { 0..6 };

    let t = Instant::now();
    let rec = MathRecognizer::new();
    println!("# Maths recognizer report\n");
    println!(
        "init {:.1} ms, {} templates, ~{} KiB",
        t.elapsed().as_secs_f64() * 1e3,
        rec.template_count(),
        rec.template_bytes() / 1024
    );

    if !speed_only {
        accuracy(&rec, seeds, verbose);
    }
    speed(&rec);
}

fn accuracy(rec: &MathRecognizer, seeds: std::ops::Range<u64>, verbose: bool) {
    let rec = rec.clone();
    let mut all: Vec<&str> = synth::SIMPLE.to_vec();
    all.extend_from_slice(synth::STRUCTURED);
    let std_rep = synth::evaluate(&rec, &all, seeds.clone(), &SynthStyle::standard());
    print_report("standard perturbation", &std_rep, verbose);
    let req = synth::evaluate(
        &rec,
        synth::REQUIRED,
        seeds.clone(),
        &SynthStyle::standard(),
    );
    println!("\nexpressions named in the brief:");
    for (e, ok, n) in &req.per_expr {
        println!("  {:<28} {ok}/{n}", e);
    }
    println!("  total {}", pct(req.correct, req.samples));
    if verbose {
        for (t, g, seed) in &req.failures {
            println!("  {t}  ⇒  {g}   (seed {seed})");
        }
    }
    let hard = synth::evaluate(&rec, &all, seeds, &SynthStyle::hard());
    print_report("hard perturbation", &hard, verbose);
}

fn speed(rec: &MathRecognizer) {
    // Speed: ten-symbol expressions, whole-expression recognition.
    println!("\n## speed\n");
    let synth = Synth::new();
    let tens = [
        "2x^2+3x-4=0",
        "a^2+b^2=c^2+1",
        "y=3x+2-5x",
        "E=mc^2+12",
        "s=ut+at^2",
    ];
    let mut times = Vec::new();
    let mut strokes_n = 0;
    for seed in 0..40 {
        for e in &tens {
            let s = synth
                .render(e, seed, &SynthStyle::standard())
                .expect("render");
            strokes_n += s.strokes.len();
            // Best of 5 runs: the machine may be busy with other work.
            let mut best = f64::INFINITY;
            for _ in 0..5 {
                let t = Instant::now();
                let c = rec.recognize(&s.strokes);
                best = best.min(t.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(c);
            }
            times.push(best);
        }
    }
    let n = times.len();
    println!(
        "recognize, 9–11 symbols ({:.1} strokes avg, n={n}, best of 5 each): p50 {:.2} ms, p99 {:.2} ms, max {:.2} ms",
        strokes_n as f64 / n as f64,
        percentile(&mut times, 0.5),
        percentile(&mut times, 0.99),
        percentile(&mut times, 1.0)
    );
    // Live preview: push strokes one by one, recognise after each.
    let mut live = Vec::new();
    for seed in 0..20 {
        let s = synth
            .render("2x^2+3x-4=0", seed, &SynthStyle::standard())
            .expect("render");
        let mut best = vec![f64::INFINITY; s.strokes.len()];
        for _ in 0..5 {
            let mut r = rec.clone();
            for (k, st) in s.strokes.iter().enumerate() {
                r.push_stroke(st);
                let t = Instant::now();
                std::hint::black_box(r.candidates());
                best[k] = best[k].min(t.elapsed().as_secs_f64() * 1e3);
            }
        }
        live.extend(best);
    }
    println!(
        "live preview per stroke (cached groups, n={}, best of 5): p50 {:.2} ms, p99 {:.2} ms",
        live.len(),
        percentile(&mut live, 0.5),
        percentile(&mut live, 0.99)
    );
}
