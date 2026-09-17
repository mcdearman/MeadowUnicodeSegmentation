//! Writes `src/Tables.mw` and `src/Cases.mw` from `unicode-segmentation`.
//!
//! ```text
//! cargo run --release -- <package root>
//! ```
//!
//! * **Tables** -- the break-property category of every code point, for
//!   graphemes, words and sentences, and the few other properties the rules
//!   consult, read off the crate's own `tables.rs` (see `build.rs`) and written
//!   as sorted ranges in Meadow string literals.
//! * **Cases** -- strings with the segmentations the crate gives them: every
//!   string in the official Unicode test files the crate carries, and
//!   sequences built at random from a character of every category. The
//!   expected values come from calling the crate, so the Meadow tests hold the
//!   port to it.
//!
//! The rules themselves are ported by hand into `src/Grapheme.mw`,
//! `src/Word.mw` and `src/Sentence.mw`.

#[allow(
    dead_code,
    clippy::all,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
mod upstream {
    include!(concat!(env!("OUT_DIR"), "/upstream.rs"));
}

#[allow(dead_code, clippy::all)]
mod testdata {
    include!(concat!(env!("OUT_DIR"), "/testdata.rs"));
}

use std::fmt::Write as _;
use std::path::PathBuf;
use unicode_segmentation::UnicodeSegmentation;
use upstream::{emoji, grapheme, sentence, word};

const SCALARS: u32 = 0x11_0000;

/// The crate version pinned in `Cargo.toml`.
const UPSTREAM_VERSION: &str = "1.13.3";

/// The fingerprint of the crate's `grapheme.rs`, `word.rs` and `sentence.rs`,
/// whose rules `src/Grapheme.mw`, `src/Word.mw` and `src/Sentence.mw` port.
const RULES: u64 = 0x01a5_efac_75d5_6126;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "../..".into()));

    let print = fingerprint(include_str!(concat!(env!("OUT_DIR"), "/rules.rs.txt")));
    if print != RULES {
        eprintln!(
            "error: the unicode-segmentation rules are not the ones src/*.mw port.\n\
             Compare src/grapheme.rs, src/word.rs and src/sentence.rs in {} with the\n\
             previous version, carry any change into the Meadow sources, then set\n\
             RULES in scripts/generate/src/main.rs to {print:#x}",
            env!("UPSTREAM_DIR")
        );
        std::process::exit(1);
    }

    let tables = tables();
    let tables_path = root.join("src/Tables.mw");
    std::fs::write(&tables_path, &tables).unwrap();
    eprintln!("wrote {} ({} bytes)", tables_path.display(), tables.len());

    let cases = cases();
    let cases_path = root.join("src/Cases.mw");
    std::fs::write(&cases_path, &cases).unwrap();
    eprintln!("wrote {} ({} bytes)", cases_path.display(), cases.len());
}

/// FNV-1a: stable across builds, which `DefaultHasher` does not promise.
fn fingerprint(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// --- encoding -----------------------------------------------------------------------

/// A number as `digits` base-64 digits, most significant first, each digit the
/// character `'0' + d`: `'0'` to `'o'`, one contiguous run of ASCII.
fn digits(out: &mut String, value: u64, digits: u32) {
    assert!(
        value < 1 << (6 * digits),
        "{value} does not fit in {digits} digits"
    );
    for k in (0..digits).rev() {
        out.push(char::from(b'0' + ((value >> (6 * k)) & 63) as u8));
    }
}

/// `text` as one Meadow string literal, broken with `\`-newline every `width`
/// characters. Only printable ASCII is written raw; a space that would start a
/// line is `\x20`, since a continuation drops leading whitespace.
fn long_literal(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / width * 4 + 2);
    out.push('"');
    for (i, c) in text.chars().enumerate() {
        let line_start = i > 0 && i % width == 0;
        if line_start {
            out.push_str("\\\n    ");
        }
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            ' ' if line_start => out.push_str("\\x20"),
            ' '..='~' => out.push(c),
            _ => {
                let _ = write!(out, "\\u{{{:X}}}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

const HEADER: &str = "\
-- Copyright 2012-2025 The Rust Project Developers, and the Meadow port's
-- authors. Dual-licensed under Apache-2.0 or MIT: see COPYRIGHT.";

// --- tables -------------------------------------------------------------------------

/// Maximal runs of code points with the same value of `f`, leaving out those
/// whose value is `skip`: `(lo, hi, value)`.
fn value_runs<T: PartialEq + Copy>(f: impl Fn(char) -> T, skip: T) -> Vec<(u32, u32, T)> {
    let mut out: Vec<(u32, u32, T)> = Vec::new();
    for cp in 0..SCALARS {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let v = f(c);
        if v == skip {
            continue;
        }
        match out.last_mut() {
            // The surrogates are never asked about, so a run may jump them.
            Some((_, hi, lv)) if *lv == v && (*hi + 1 == cp || (*hi == 0xD7FF && cp == 0xE000)) => {
                *hi = cp
            }
            _ => out.push((cp, cp, v)),
        }
    }
    out
}

fn value_table(runs: &[(u32, u32, u8)]) -> String {
    let mut s = String::new();
    for &(lo, hi, v) in runs {
        digits(&mut s, lo.into(), 4);
        digits(&mut s, hi.into(), 4);
        digits(&mut s, v.into(), 1);
    }
    s
}

fn range_table(runs: &[(u32, u32, bool)]) -> String {
    let mut s = String::new();
    for &(lo, hi, _) in runs {
        digits(&mut s, lo.into(), 4);
        digits(&mut s, hi.into(), 4);
    }
    s
}

/// The grapheme category the crate's cursor uses: the table, except for
/// ASCII, which it decides by hand (and the same way).
fn grapheme_cat(c: char) -> u8 {
    if c <= '\u{7e}' {
        let cat = if c >= '\u{20}' {
            grapheme::GC_Any
        } else if c == '\n' {
            grapheme::GC_LF
        } else if c == '\r' {
            grapheme::GC_CR
        } else {
            grapheme::GC_Control
        };
        cat as u8
    } else {
        grapheme::grapheme_category(c).2 as u8
    }
}

fn word_cat(c: char) -> u8 {
    word::word_category(c).2 as u8
}

fn sentence_cat(c: char) -> u8 {
    sentence::sentence_category(c).2 as u8
}

fn is_pictographic(c: char) -> bool {
    emoji::emoji_category(c).2 == emoji::EmojiCat::EC_Extended_Pictographic
}

/// Enum names in declaration order, which is what `as u8` numbers them by.
fn names<T: std::fmt::Debug>(all: &[T]) -> String {
    all.iter()
        .enumerate()
        .map(|(i, v)| format!("--   {i:>2} {v:?}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A property written as a range table: its name, what it is, and the test.
type PropertySet = (&'static str, &'static str, fn(char) -> bool);

fn tables() -> String {
    use grapheme::GraphemeCat as G;
    use sentence::SentenceCat as S;
    use word::WordCat as W;
    let (maj, min, pat) = upstream::UNICODE_VERSION;

    // Checked, since Meadow numbers the categories by hand to match.
    let g_names = names(&[
        G::GC_Any,
        G::GC_CR,
        G::GC_Control,
        G::GC_Extend,
        G::GC_Extended_Pictographic,
        G::GC_InCB_Consonant,
        G::GC_L,
        G::GC_LF,
        G::GC_LV,
        G::GC_LVT,
        G::GC_Prepend,
        G::GC_Regional_Indicator,
        G::GC_SpacingMark,
        G::GC_T,
        G::GC_V,
        G::GC_ZWJ,
    ]);
    assert_eq!(G::GC_ZWJ as u8, 15);
    let w_names = names(&[
        W::WC_ALetter,
        W::WC_Any,
        W::WC_CR,
        W::WC_Double_Quote,
        W::WC_Extend,
        W::WC_ExtendNumLet,
        W::WC_Format,
        W::WC_Hebrew_Letter,
        W::WC_Katakana,
        W::WC_LF,
        W::WC_MidLetter,
        W::WC_MidNum,
        W::WC_MidNumLet,
        W::WC_Newline,
        W::WC_Numeric,
        W::WC_Regional_Indicator,
        W::WC_Single_Quote,
        W::WC_WSegSpace,
        W::WC_ZWJ,
    ]);
    assert_eq!(W::WC_ZWJ as u8, 18);
    let s_names = names(&[
        S::SC_ATerm,
        S::SC_Any,
        S::SC_CR,
        S::SC_Close,
        S::SC_Extend,
        S::SC_Format,
        S::SC_LF,
        S::SC_Lower,
        S::SC_Numeric,
        S::SC_OLetter,
        S::SC_SContinue,
        S::SC_STerm,
        S::SC_Sep,
        S::SC_Sp,
        S::SC_Upper,
    ]);
    assert_eq!(S::SC_Upper as u8, 14);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- GENERATED by scripts/generate.sh from unicode-segmentation {UPSTREAM_VERSION}.
-- Do not edit: run the script again instead.
--
-- Every table is a string of fixed-width records, each field written in the
-- base-64 digits '0' ('0' + 0) to 'o' ('0' + 63), most significant first, and
-- is read in place by `Lookup.mw`.
--
{HEADER}

-- The Unicode version the tables describe.
@pub(pkg) def unicodeVersion = ({maj}, {min}, {pat})
"
    );

    let mut def = |name: &str, doc: String, body: &str| {
        let _ = writeln!(
            out,
            "{doc}\n@pub(pkg) def {name} =\n  {}\n",
            long_literal(body, 96)
        );
    };
    let categories = |what: &str, listing: &str, default: &str| {
        format!(
            "-- Each code point's {what} category, as `lo hi` (4 digits each) and the\n\
             -- category (1). Code points not listed are {default}. The categories:\n--\n{listing}"
        )
    };
    def(
        "graphemeCats",
        categories("grapheme cluster break", &g_names, "Any"),
        &value_table(&value_runs(grapheme_cat, G::GC_Any as u8)),
    );
    def(
        "wordCats",
        categories("word break", &w_names, "Any"),
        &value_table(&value_runs(word_cat, W::WC_Any as u8)),
    );
    def(
        "sentenceCats",
        categories("sentence break", &s_names, "Any"),
        &value_table(&value_runs(sentence_cat, S::SC_Any as u8)),
    );
    let sets: [PropertySet; 4] = [
        (
            "extendedPictographic",
            "are Extended_Pictographic",
            is_pictographic,
        ),
        (
            "incbLinker",
            "are Indic_Conjunct_Break=Linker",
            upstream::is_incb_linker,
        ),
        (
            "incbExtend",
            "are Indic_Conjunct_Break=Extend",
            upstream::derived_property::InCB_Extend,
        ),
        (
            "alphanumeric",
            "are Alphabetic or General_Category=Number: what makes a segment a\n-- word or a sentence",
            upstream::util::is_alphanumeric,
        ),
    ];
    for (name, what, f) in sets {
        def(
            name,
            format!("-- Code points that {what}: `lo hi`, 4 digits each."),
            &range_table(&value_runs(f, false)),
        );
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

// --- cases --------------------------------------------------------------------------

/// A small deterministic generator, so that the cases are the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A few characters of every category the rules distinguish, and of the
/// properties beside them, so that random sequences of them reach every rule.
fn interesting() -> Vec<char> {
    let mut picks: Vec<char> = Vec::new();
    let mut take = |f: &dyn Fn(char) -> u8| {
        let mut seen: std::collections::BTreeMap<u8, Vec<char>> = Default::default();
        for cp in 0..SCALARS {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let list = seen.entry(f(c)).or_default();
            // The first two of each, and one from much further on.
            if list.len() < 2 || (list.len() < 3 && cp > 0x3000) {
                list.push(c);
            }
        }
        picks.extend(seen.into_values().flatten());
    };
    take(&grapheme_cat);
    take(&word_cat);
    take(&sentence_cat);
    take(&|c| u8::from(upstream::is_incb_linker(c)));
    take(&|c| u8::from(upstream::derived_property::InCB_Extend(c)));
    picks.extend(" .,;:'\"!?()[]0123456789aAzZ\t\n\r".chars());
    // Real consonants and a virama, for GB9c, and a family of emoji.
    picks.extend("\u{915}\u{937}\u{94d}\u{200d}\u{1f468}\u{1f469}\u{2764}\u{fe0f}".chars());
    picks.sort_unstable();
    picks.dedup();
    picks
}

/// A segmentation as the byte offsets where each segment ends.
fn ends<'a>(s: &str, parts: impl Iterator<Item = &'a str>) -> Vec<usize> {
    let base = s.as_ptr() as usize;
    parts
        .map(|p| p.as_ptr() as usize - base + p.len())
        .collect()
}

/// Filtered segments as `(start, end)` byte offsets.
fn spans<'a>(s: &str, parts: impl Iterator<Item = &'a str>) -> Vec<(usize, usize)> {
    let base = s.as_ptr() as usize;
    parts
        .map(|p| {
            let start = p.as_ptr() as usize - base;
            (start, start + p.len())
        })
        .collect()
}

fn offsets(out: &mut String, list: &[usize]) {
    digits(out, list.len() as u64, 3);
    for &n in list {
        digits(out, n as u64, 3);
    }
}

fn pairs(out: &mut String, list: &[(usize, usize)]) {
    digits(out, list.len() as u64, 3);
    for &(a, b) in list {
        digits(out, a as u64, 3);
        digits(out, b as u64, 3);
    }
}

fn cases() -> String {
    let mut strings: Vec<String> = Vec::new();
    strings.extend(testdata::TEST_SAME.iter().map(|(s, _)| s.to_string()));
    strings.extend(testdata::TEST_DIFF.iter().map(|(s, _, _)| s.to_string()));
    strings.extend(testdata::TEST_WORD.iter().map(|(s, _)| s.to_string()));
    strings.extend(testdata::TEST_SENTENCE.iter().map(|(s, _)| s.to_string()));
    let official = strings.len();

    // Prose, where the rules for abbreviations, quotes and numbers meet.
    strings.extend(
        [
            "Mr. Fox jumped. [...] The dog was too lazy.",
            "The quick (\"brown\") fox can't jump 32.3 feet, right?",
            "He said \"Stop!\" She didn't. 3.14 is pi; e.g. it's not 3,14.",
            "etc.)' Hello? \u{2019}Tis true.\r\nNew line\u{2029}New paragraph",
            "कि क्ष हिन्दी 👨\u{200d}👩\u{200d}👧 🇺🇸🇬🇧 a\u{308}",
            "日本語のテキスト。カタカナ、ひらがな。",
            "שָׁלוֹם \"עולם\" ה'ג",
            "can't won’t e.g. U.S.A. 1,000.5 $3 _foo_bar a_1",
        ]
        .map(String::from),
    );

    let pool = interesting();
    let mut rng = Rng(0x5eed_5e9e_c0ff_ee42);
    for _ in 0..4000 {
        let len = 1 + rng.below(10);
        strings.push((0..len).map(|_| pool[rng.below(pool.len())]).collect());
    }
    strings.sort();
    strings.dedup();

    let mut body = String::new();
    for s in &strings {
        digits(&mut body, s.len() as u64, 3);
        body.push_str(s);
        offsets(&mut body, &ends(s, s.graphemes(true)));
        offsets(&mut body, &ends(s, s.graphemes(false)));
        offsets(&mut body, &ends(s, s.split_word_bounds()));
        pairs(&mut body, &spans(s, s.unicode_words()));
        offsets(&mut body, &ends(s, s.split_sentence_bounds()));
        pairs(&mut body, &spans(s, s.unicode_sentences()));
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- GENERATED by scripts/generate.sh from unicode-segmentation {UPSTREAM_VERSION}.
-- Do not edit: run the script again instead.
--
-- Strings with the segmentations the crate gives them, for `Tests.mw`: {} in
-- all, {official} of them from the official Unicode test files.
--
{HEADER}

-- Each: the length in bytes (3 digits) and that many bytes, then six lists,
-- each a count (3) and its items. Where each segment ends (3 per item) for
-- extended graphemes, legacy graphemes, word bounds; `start end` (3 + 3) for
-- words; ends for sentence bounds; `start end` for sentences.
@cfg(test)
@pub(pkg) def cases =
  {}",
        strings.len(),
        long_literal(&body, 96)
    );
    out
}
