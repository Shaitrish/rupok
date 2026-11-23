//! Render synthesised handwriting to an SVG, to look at the hand.
//!
//! `cargo run -p rupok-ink --example write_svg -- out.svg "line 1" "line 2" …`

use rupok_ink::write::{GlyphSet, WriteOptions, write};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "write.svg".into());
    let mut lines: Vec<String> = args.collect();
    if lines.is_empty() {
        lines = vec![
            "Subtract 3 from both sides:".into(),
            "$2x + 3 - 3 = 7 - 3$".into(),
            "$2x = 4$".into(),
            "Divide by 2: $x = \\frac{4}{2} = 2$".into(),
            "$\\sqrt{x^2 + 9} \\ne \\alpha_1 \\times \\pi r^2$".into(),
        ];
    }
    let g = GlyphSet::new();
    let w = write(
        &lines,
        &g,
        &WriteOptions {
            x_height: 28.0,
            ..WriteOptions::default()
        },
    );
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{:.0}' height='{:.0}' viewBox='-20 -20 {:.0} {:.0}'><rect x='-20' y='-20' width='100%' height='100%' fill='white'/>",
        w.width + 40.0,
        w.height + 40.0,
        w.width + 40.0,
        w.height + 40.0
    );
    for s in &w.strokes {
        let d: Vec<String> = s
            .points
            .iter()
            .map(|p| format!("{:.1},{:.1}", p.x, p.y))
            .collect();
        svg += &format!(
            "<polyline points='{}' fill='none' stroke='#1c2024' stroke-width='3.6' stroke-linecap='round' stroke-linejoin='round'/>",
            d.join(" ")
        );
    }
    svg += "</svg>";
    std::fs::write(&out, svg).expect("write the SVG");
    println!(
        "{} strokes, {:.1} s, {:.0}×{:.0}",
        w.strokes.len(),
        w.duration_ms / 1000.0,
        w.width,
        w.height
    );
}
