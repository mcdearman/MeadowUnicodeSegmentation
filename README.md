# unicodeSegmentation

Split text into grapheme clusters, words and sentences, for
[Meadow](https://github.com/mcdearman/meadow).

This package is a port of Rust's
[`unicode-segmentation`](https://github.com/unicode-rs/unicode-segmentation)
1.13.3, covering Unicode 17.0.0. It follows the rules of
[UAX #29](https://www.unicode.org/reports/tr29/). Use it wherever "one
character" should mean what a reader sees rather than one code point: moving a
cursor, truncating a label, reversing a string, or counting words.

## Install

```sh
meadow add mcdearman/UnicodeSegmentation
```

## Use

```meadow
use UnicodeSegmentation (graphemes, unicodeWords, unicodeSentences)

def main =
  ( graphemes True "a\u{310}e\u{301}o\u{308}\u{332}",
    -- ["a̐", "é", "ö̲"]: three clusters, seven code points
    unicodeWords "The quick (\"brown\") fox can't jump 32.3 feet, right?",
    -- ["The", "quick", "brown", "fox", "can't", "jump", "32.3", "feet", "right"]
    unicodeSentences "Mr. Fox jumped. [...] The dog was too lazy."
    -- ["Mr. ", "Fox jumped. ", "The dog was too lazy."]
  )
```

The string is always the last argument, and offsets are in bytes, as in
`Std.String`. Every function returns a `Vector`.

| function | |
|---|---|
| `graphemes extended s` | the grapheme clusters of `s` |
| `graphemeIndices extended s` | the same, each paired with the byte offset where it starts |
| `isGraphemeBoundary extended offset s` | whether byte `offset` falls between two clusters |
| `nextGraphemeBoundary extended offset s` | the next boundary after `offset`, or `None` at the end |
| `prevGraphemeBoundary extended offset s` | the previous boundary before `offset`, or `None` at the start |
| `splitWordBounds s` | `s` split at every word boundary, keeping spaces and punctuation |
| `splitWordBoundIndices s` | the same, with offsets |
| `unicodeWords s` | only the pieces that contain a letter or number |
| `unicodeWordIndices s` | the same, with offsets |
| `splitSentenceBounds s` | `s` split at every sentence boundary |
| `splitSentenceBoundIndices s` | the same, with offsets |
| `unicodeSentences s` | only the pieces that contain a letter or number |
| `unicodeVersion` | `(17, 0, 0)` |

Pass `extended = True` unless you need the old behaviour. Extended clusters are
what UAX #29 recommends: spacing marks and Indic conjuncts stay together
(`"क्षि"` is one cluster). Legacy clusters split them apart.

### Differences from the crate

- **Whole strings only.** The crate's `GraphemeCursor` works on text that
  arrives in chunks; this port doesn't. The three boundary functions cover what
  a cursor is usually needed for, but each call reads the string from the
  start.
- **No reverse iteration.** The results are vectors, so reverse the vector
  instead.

## How it's made

- **`src/Tables.mw`** is generated from the crate's own tables. It holds the
  grapheme, word and sentence break category of every code point, plus the
  other properties the rules use.
- **`src/Grapheme.mw`, `src/Word.mw` and `src/Sentence.mw`** translate the
  crate's rules by hand. That includes its word state machine and its faster
  ASCII word splitter, which `unicodeWords` uses for all-ASCII strings.
- **`src/Cases.mw`** is generated test data: 6,552 strings, 3,222 of them from
  Unicode's official test files and the rest built at random from characters
  of every category. For each string, the test checks this port against the
  crate's output for all six segmentations: extended and legacy graphemes, word
  bounds, words, sentence bounds and sentences.

To regenerate, or to move to a newer version of the crate, update the version
pin in `scripts/generate/Cargo.toml` and run:

```sh
scripts/generate.sh
```

This needs a Rust toolchain. The generator fingerprints the crate's rule
sources and refuses to run if they changed, because then the Meadow translation
has to be updated by hand first.

## Licence

Like the crate, this package is dual-licensed under [Apache-2.0](LICENSE-APACHE)
or [MIT](LICENSE-MIT), at your option. See [COPYRIGHT](COPYRIGHT).
