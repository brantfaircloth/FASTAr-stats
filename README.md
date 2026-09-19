# fastar-stats

A small, fast command-line tool that reports per-sequence composition stats for
a FASTA file: length, N-content, and soft-masked content.

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

## Install

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

On a synthetic 196MB, 200-record FASTA, `fastar-stats` runs in ~75ms
(~2.6 GB/s).

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

## License

MIT — see [LICENSE](LICENSE).
