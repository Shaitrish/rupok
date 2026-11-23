//! Hershey glyph records used to build the built-in templates.
//!
//! # Source and licence
//!
//! The records below are copied verbatim (one record per symbol, the glyph
//! number dropped) from the `.jhf` files of the `hershey-fonts` distribution
//! (<https://github.com/kamalmostafa/hershey-fonts>, files
//! `hershey-fonts/futural.jhf`, `greek.jhf` and `mathlow.jhf`). Only the glyph
//! data is used, not the GPL library code of that project. The glyph data is
//! under the Hershey font licence, which allows use and redistribution for
//! any purpose provided that these acknowledgements travel with the data:
//!
//! > The Hershey Fonts were originally created by Dr. A. V. Hershey while
//! > working at the U. S. National Bureau of Standards.
//! >
//! > The format of the Font data in this distribution was originally created
//! > by James Hurt, Cognition, Inc., 900 Technology Park Drive, Billerica,
//! > MA 01821 (mit-eddie!ci-dandelion!hurt).
//!
//! The licence also forbids redistributing the data in the original U.S.
//! NTIS format; the `.jhf` format used here is James Hurt's, not NTIS's.
//!
//! # Format
//!
//! A record is a 3-character vertex count followed by that many coordinate
//! pairs. Each coordinate is a character whose offset from `'R'` is the
//! value. The first pair holds the glyph's left and right bounds; `" R"`
//! lifts the pen. In these fonts y grows downwards, the baseline is at
//! y = 9, the x-height line at −5, capitals reach −12 and descenders 16.

use super::geom::Pt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// Roman simplex (`futural.jhf`).
    Futural,
    /// Greek simplex (`greek.jhf`).
    Greek,
    /// Maths symbols (`mathlow.jhf`).
    Mathlow,
}

pub struct HGlyph {
    /// Symbol name (see `symbols::SYMBOLS`).
    pub sym: &'static str,
    pub font: Font,
    pub data: &'static str,
}

const fn g(sym: &'static str, font: Font, data: &'static str) -> HGlyph {
    HGlyph { sym, font, data }
}

use Font::{Futural, Greek, Mathlow};

pub const GLYPHS: &[HGlyph] = &[
    g("0", Futural, r#" 18H\QFNGLJKOKRLWNZQ[S[VZXWYRYOXJVGSFQF"#),
    g("1", Futural, r#"  5H\NJPISFS["#),
    g("2", Futural, r#" 15H\LKLJMHNGPFTFVGWHXJXLWNUQK[Y["#),
    g("3", Futural, r#" 16H\MFXFRNUNWOXPYSYUXXVZS[P[MZLYKW"#),
    g("4", Futural, r#"  7H\UFKTZT RUFU["#),
    g("5", Futural, r#" 18H\WFMFLOMNPMSMVNXPYSYUXXVZS[P[MZLYKW"#),
    g(
        "6",
        Futural,
        r#" 24H\XIWGTFRFOGMJLOLTMXOZR[S[VZXXYUYTXQVOSNRNOOMQLT"#,
    ),
    g("7", Futural, r#"  6H\YFO[ RKFYF"#),
    g(
        "8",
        Futural,
        r#" 30H\PFMGLILKMMONSOVPXRYTYWXYWZT[P[MZLYKWKTLRNPQOUNWMXKXIWGTFPF"#,
    ),
    g(
        "9",
        Futural,
        r#" 24H\XMWPURRSQSNRLPKMKLLINGQFRFUGWIXMXRWWUZR[P[MZLX"#,
    ),
    g("a", Futural, r#" 18I\XMX[ RXPVNTMQMONMPLSLUMXOZQ[T[VZXX"#),
    g("b", Futural, r#" 18H[LFL[ RLPNNPMSMUNWPXSXUWXUZS[P[NZLX"#),
    g("c", Futural, r#" 15I[XPVNTMQMONMPLSLUMXOZQ[T[VZXX"#),
    g("d", Futural, r#" 18I\XFX[ RXPVNTMQMONMPLSLUMXOZQ[T[VZXX"#),
    g("e", Futural, r#" 18I[LSXSXQWOVNTMQMONMPLSLUMXOZQ[T[VZXX"#),
    g("f", Futural, r#"  9MYWFUFSGRJR[ ROMVM"#),
    g(
        "g",
        Futural,
        r#" 23I\XMX]W`VaTbQbOa RXPVNTMQMONMPLSLUMXOZQ[T[VZXX"#,
    ),
    g("h", Futural, r#" 11I\MFM[ RMQPNRMUMWNXQX["#),
    g("i", Futural, r#"  9NVQFRGSFREQF RRMR["#),
    g("j", Futural, r#" 12MWRFSGTFSERF RSMS^RaPbNb"#),
    g("k", Futural, r#"  9IZMFM[ RWMMW RQSX["#),
    g("l", Futural, r#"  3NVRFR["#),
    g("m", Futural, r#" 19CaGMG[ RGQJNLMOMQNRQR[ RRQUNWMZM\N]Q]["#),
    g("n", Futural, r#" 11I\MMM[ RMQPNRMUMWNXQX["#),
    g("o", Futural, r#" 18I\QMONMPLSLUMXOZQ[T[VZXXYUYSXPVNTMQM"#),
    g("p", Futural, r#" 18H[LMLb RLPNNPMSMUNWPXSXUWXUZS[P[NZLX"#),
    g("q", Futural, r#" 18I\XMXb RXPVNTMQMONMPLSLUMXOZQ[T[VZXX"#),
    g("r", Futural, r#"  9KXOMO[ ROSPPRNTMWM"#),
    g("s", Futural, r#" 18J[XPWNTMQMNNMPNRPSUTWUXWXXWZT[Q[NZMX"#),
    g("t", Futural, r#"  9MYRFRWSZU[W[ ROMVM"#),
    g("u", Futural, r#" 11I\MMMWNZP[S[UZXW RXMX["#),
    g("v", Futural, r#"  6JZLMR[ RXMR["#),
    g("w", Futural, r#" 12G]JMN[ RRMN[ RRMV[ RZMV["#),
    g("x", Futural, r#"  6J[MMX[ RXMM["#),
    g("y", Futural, r#" 10JZLMR[ RXMR[P_NaLbKb"#),
    g("z", Futural, r#"  9J[XMM[ RMMXM RM[X["#),
    g("A", Futural, r#"  9I[RFJ[ RRFZ[ RMTWT"#),
    g(
        "B",
        Futural,
        r#" 24G\KFK[ RKFTFWGXHYJYLXNWOTP RKPTPWQXRYTYWXYWZT[K["#,
    ),
    g("C", Futural, r#" 19H]ZKYIWGUFQFOGMILKKNKSLVMXOZQ[U[WZYXZV"#),
    g("E", Futural, r#" 12H[LFL[ RLFYF RLPTP RL[Y["#),
    g("F", Futural, r#"  9HZLFL[ RLFYF RLPTP"#),
    g("H", Futural, r#"  9G]KFK[ RYFY[ RKPYP"#),
    g("M", Futural, r#" 12F^JFJ[ RJFR[ RZFR[ RZFZ["#),
    g("N", Futural, r#"  9G]KFK[ RKFY[ RYFY["#),
    g(
        "O",
        Futural,
        r#" 22G]PFNGLIKKJNJSKVLXNZP[T[VZXXYVZSZNYKXIVGTFPF"#,
    ),
    g("P", Futural, r#" 14G\KFK[ RKFTFWGXHYJYMXOWPTQKQ"#),
    g("R", Futural, r#" 17G\KFK[ RKFTFWGXHYJYLXNWOTPKP RRPY["#),
    g(
        "S",
        Futural,
        r#" 21H\YIWGTFPFMGKIKKLMMNOOUQWRXSYUYXWZT[P[MZKX"#,
    ),
    g("T", Futural, r#"  6JZRFR[ RKFYF"#),
    g("V", Futural, r#"  6I[JFR[ RZFR["#),
    g("+", Futural, r#"  6E_RIR[ RIR[R"#),
    g("−", Futural, r#"  3E_IR[R"#),
    g("=", Futural, r#"  6E_IO[O RIU[U"#),
    g("<", Futural, r#"  4F^ZIJRZ["#),
    g(">", Futural, r#"  4F^JIZRJ["#),
    g("(", Futural, r#" 11KYVBTDRGPKOPOTPYR]T`Vb"#),
    g(")", Futural, r#" 11KYNBPDRGTKUPUTTYR]P`Nb"#),
    g("/", Futural, r#"  3G][BIb"#),
    g(".", Futural, r#"  6NVRVQWRXSWRV"#),
    g(",", Futural, r#"  8NVSWRXQWRVSWSYQ["#),
    g("′", Futural, r#"  8MWRHQGRFSGSIRKQL"#),
    g("|", Futural, r#"  3NVRBRb"#),
    g(
        "α",
        Greek,
        r#" 24H]QMONMPLRKUKXLZN[P[RZUWWTYPZM RQMSMTNUPWXXZY[Z["#,
    ),
    g(
        "β",
        Greek,
        r#" 31I\UFSGQIOMNPMTLZKb RUFWFYHYKXMWNUORO RROTPVRWTWWVYUZS[Q[OZNYMV"#,
    ),
    g(
        "θ",
        Greek,
        r#" 25G]PFNGLIKKJNJSKVLXNZP[T[VZXXYVZSZNYKXIVGTFPF ROPUP"#,
    ),
    g("λ", Greek, r#"  9JZKFMFOGPHX[ RRML["#),
    g(
        "μ",
        Greek,
        r#" 21H]OMIb RNQMVMYO[Q[SZUXWT RYMWTVXVZW[Y[[Y\W"#,
    ),
    g("π", Greek, r#" 13G]PML[ RUMVSWXX[ RIPKNNM[M"#),
    g("Δ", Greek, r#"  9I[RFJ[ RRFZ[ RJ[Z["#),
    g("Σ", Greek, r#" 10I[KFRPK[ RKFYF RK[Y["#),
    g("±", Mathlow, r#"  9F^RJR[ RJRZR RJ[Z["#),
    g("≠", Mathlow, r#"  9E_YIK[ RIO[O RIU[U"#),
    g(
        "∞",
        Mathlow,
        r#" 26F_\S[UYVWVUUTTQPPONNLNJOIQISJULVNVPUQTTPUOWNYN[O\Q\S"#,
    ),
    g("°", Mathlow, r#" 14KYQFOGNINKOMQNSNUMVKVIUGSFQF"#),
    g("·", Mathlow, r#"  6PURQRRSRSQRQ"#),
    g("÷", Mathlow, r#" 15E_RIQJRKSJRI RIR[R RRYQZR[SZRY"#),
];

/// Strokes of a record, in font units. `None` when the record is malformed.
pub fn parse(data: &str) -> Option<Vec<Vec<Pt>>> {
    let b = data.as_bytes();
    let count: usize = std::str::from_utf8(b.get(..3)?).ok()?.trim().parse().ok()?;
    let body = b.get(3..3 + 2 * count)?;
    let mut strokes: Vec<Vec<Pt>> = vec![Vec::new()];
    for pair in body.as_chunks::<2>().0.iter().skip(1) {
        if pair == b" R" {
            strokes.push(Vec::new());
            continue;
        }
        let x = pair[0] as f32 - b'R' as f32;
        let y = pair[1] as f32 - b'R' as f32;
        strokes.last_mut()?.push(Pt::new(x, y));
    }
    strokes.retain(|s| !s.is_empty());
    Some(strokes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_record_parses() {
        for gl in GLYPHS {
            let s = parse(gl.data).unwrap_or_else(|| panic!("{}", gl.sym));
            assert!(!s.is_empty(), "{}", gl.sym);
            assert!(super::super::symbols::find(gl.sym).is_some(), "{}", gl.sym);
        }
        // "1": a flag and a stem, one stroke from (-4,-8) to (1,9).
        let one = GLYPHS.iter().find(|g| g.sym == "1").unwrap();
        let s = parse(one.data).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0][0], Pt::new(-4.0, -8.0));
        assert_eq!(*s[0].last().unwrap(), Pt::new(1.0, 9.0));
        assert!(parse("").is_none());
        assert!(parse(" 9H\\").is_none());
    }
}
