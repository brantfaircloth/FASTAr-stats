# fastar-stats

A small, fast command-line tool that reports per-sequence composition stats for
a FASTA file: length, N-content, and soft-masked content.

## Coding

I won't mince words - I vibe coded this because it's a tool I needed, and I wanted
it to be **fast**.

## What it does

Given a FASTA file, `fastar-stats` prints one row per record (contig/scaffold)
with:

| column | meaning |
|---|---|
| `name` | sequence ID — the first whitespace-delimited token on the `>` header line |
| `num_bases` | total sequence length (all characters on the sequence lines, including N) |
| `num_n` | count of `N`/`n` bases (assembly gaps) |
| `num_masked` | count of lowercase bases (soft-masked repeats) |
| `pct_n` | `num_n / num_bases * 100`, 2 decimal places |
| `pct_masked` | `num_masked / num_bases * 100`, 2 decimal places |

Output is tab-separated, written to stdout, with a header row.

```
$ fastar-stats tests/fixtures/sample.fasta
name    num_bases       num_n   num_masked      pct_n   pct_masked
rec01_mixed_wrap60     250     3       10      1.20    4.00
rec02_longgap_unwrapped 400    50      0       12.50   0.00
rec03_heavymask_wrap70  180    20      160     11.11   88.89
...
```

## Comparison to `faSize -detailed`/`-veryDetailed`

The closest widely-used equivalent is [UCSC kent's `faSize`](https://github.com/ucscGenomeBrowser/kent/blob/master/src/utils/faSize/faSize.c). It has two relevant flags, and neither produces the
same columns as `fastar-stats`:

| tool / flag | columns per record |
|---|---|
| `fastar-stats` | `name  num_bases  num_n  num_masked  pct_n  pct_masked` |
| `faSize -detailed` | `name  size` — no composition breakdown at all |
| `faSize -veryDetailed` | `name  size  nCount  realCount  upperCount  lowerCount` |

`-veryDetailed` is the one with comparable counts, but there's a real
semantic difference, not just a naming one: **`faSize` treats "is N" and "is
masked" as mutually exclusive, `fastar-stats` treats them as independent
axes.** In `faSize`'s loop, a base is classified as N first; only bases that
aren't N are then checked for upper/lower case, so a lowercase `n` (a
soft-masked assembly gap) is counted only in `nCount` — never in
`lowerCount`. `fastar-stats` counts case and N-ness separately, so a
lowercase `n` is counted in *both* `num_n` and `num_masked`.

Concretely, for the 16-base record `acgtNNNNnnnnACGT`:

| tool | N | masked/lower |
|---|---|---|
| `fastar-stats` | `num_n=8` | `num_masked=8` |
| `faSize -veryDetailed` | `nCount=8` | `lowerCount=4` |

Both agree on the N count; `faSize`'s lower-case count excludes the 4
lowercase `n`s that `fastar-stats` counts as masked, since it never gets a
chance to classify their case. Which convention is "right" depends on what
you're using the mask count for — whether a soft-masked gap should count as
masked sequence or not.

Other differences: `fastar-stats` reports percentages per record;
neither `faSize` flag does (only its plain, non-detailed mode prints an
aggregate `%masked` across the whole input, not per record). `faSize` also
accepts multiple files and `.2bit` input in one invocation, which
`fastar-stats` does not.

### Speed

On a synthetic 196MB, 200-record FASTA (mixed N-gaps, soft-masked runs, and
varying line-wrap widths — same style as `tests/fixtures/sample.fasta`, just
bigger), mean of 4 runs after a warm-up run, on a 10-core Apple Silicon host:

| tool | time | throughput |
|---|---|---|
| `fastar-stats` | 65ms | ~3.0 GB/s |
| `faSize -detailed` | 432ms | ~0.46 GB/s |
| `faSize -veryDetailed` | 432ms | ~0.46 GB/s |

`fastar-stats` is about **6.6x faster** here. Both flags of `faSize` take the
same time, since they're printing different columns from the same computed
totals. Output was spot-checked to agree between the two tools on length and
N-count for the same input.

This isn't a knock on `faSize` — its per-record loop does more (case-folding
into a canonical form via `faToDnaPC`, building a linked list of results for
the summary stats at the end) and it supports far more input formats and
flags than `fastar-stats` does. `fastar-stats` is deliberately narrow: one
format in, one streaming pass, one output shape — that's most of where the
gap comes from.

## Install

### Prebuilt binary

Every tagged release publishes prebuilt binaries for Linux (x86_64, aarch64),
macOS (x86_64, aarch64), and Windows (x86_64) — grab the archive for your
platform from the [Releases page](../../releases), extract it, and put the
`fastar-stats` binary somewhere on your `PATH`.

### From source

Requires a Rust toolchain (stable, 2024 edition support — install via
[rustup](https://rustup.rs) if you don't have one).

```
git clone <this-repo>
cd fastar-stats
cargo build --release
```

The binary is at `target/release/fastar-stats`. Copy it somewhere on your
`PATH`, or run it in place:

```
./target/release/fastar-stats path/to/file.fasta
```

## Usage

```
fastar-stats <fasta-file>
```

Takes exactly one argument: the path to a FASTA file. There is no support for
reading from stdin, multiple files, or gzip-compressed input — this tool does
one thing.

Exit codes: `0` on success, `2` on a usage error (wrong number of arguments),
`1` on an I/O error (e.g. file not found), with the error printed to stderr.

## How it works

The whole program is a single forward pass over the file:

- The file is read through a `BufReader` with a 1MB buffer, line by line
  (`read_until(b'\n', ...)` into a reused byte buffer — no per-line UTF-8
  validation or allocation).
- A `>` at the start of a line closes out the previous record (writing its row
  immediately) and starts a new one, named from the first whitespace-delimited
  token after the `>`.
- Every other line is a sequence line. Each byte is classified in a tight,
  branchless loop: `is_ascii_lowercase()` for masking, and a case-insensitive
  `N` check (`byte | 0x20 == b'n'`) for gaps — both counted alongside a running
  length.
- Output is written through a buffered stdout writer, one row per record, as
  soon as each record's totals are final.

No sequence data is ever held in memory — only the handful of running counters
for whatever record is currently being scanned — so memory use stays flat
(effectively the size of the I/O buffers) regardless of file size. There are
no runtime dependencies: the release binary links only against libc.

This design was chosen over memory-mapping the file: for a single sequential
counting pass, mmap's main advantage (avoiding a copy) is masked by the cost
of the per-byte classification itself, while it would add an unsafe
dependency, per-page-fault overhead on very large files without explicit
tuning, and the risk of a `SIGBUS` crash instead of a clean error if the file
changes underneath it. Plain buffered reads are just as fast here and much
simpler.

On a synthetic 196MB, 200-record FASTA, `fastar-stats` runs in well under
100ms — see [Speed](#speed) for numbers and a comparison against `faSize`.

## Testing

`tests/fixtures/sample.fasta` is a hand-designed FASTA file covering the
cases that matter for this kind of parser: multiple line-wrap widths
(including no wrapping at all), N-gaps and soft-masked runs of varying
lengths, a masked run and an N-gap straddling a line-wrap boundary, an
all-N record, a single-base record, and headers with description text
after the ID.

`tests/fixtures/sample_answer_key.json` is the corresponding truth set —
exact expected `length` / `gc_count` / `at_count` / `n_count` /
`masked_count` per record (plus a whole-file `_file_summary`), computed
directly from the generated sequences so it can't drift from the fixture.

`tests/verify_against_truth_set.rs` runs the compiled binary against the
fixture and asserts every output column against the truth set.

```
cargo test --release
```

## Releasing

Pushing a tag matching `v*.*.*` (e.g. `v0.1.0`) triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which
builds binaries for every supported platform and attaches them to a new
GitHub Release for that tag:

```
git tag v0.1.0
git push origin v0.1.0
```

## License

MIT — see [LICENSE](LICENSE).
