use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process::ExitCode;

struct RecordStats {
    name: String,
    length: u64,
    n_count: u64,
    masked_count: u64,
}

impl RecordStats {
    fn write_row<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let n_pct = pct(self.n_count, self.length);
        let masked_pct = pct(self.masked_count, self.length);
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{:.2}\t{:.2}",
            self.name, self.length, self.n_count, self.masked_count, n_pct, masked_pct
        )
    }
}

fn pct(count: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (count as f64 / total as f64) * 100.0
    }
}

fn header_name(line: &[u8]) -> String {
    // line starts with '>'; name is the first whitespace-delimited token after it.
    let rest = &line[1..];
    let end = rest
        .iter()
        .position(|b| b.is_ascii_whitespace())
        .unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

fn run() -> io::Result<()> {
    let mut args = env::args_os();
    let _prog = args.next();
    let path = match args.next() {
        Some(p) if args.next().is_none() => p,
        _ => {
            eprintln!("usage: fastar-stats <fasta-file>");
            std::process::exit(2);
        }
    };

    let file = File::open(&path)?;
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());

    writeln!(
        out,
        "name\tnum_bases\tnum_n\tnum_masked\tpct_n\tpct_masked"
    )?;

    let mut current: Option<RecordStats> = None;
    let mut line = Vec::with_capacity(256);

    loop {
        line.clear();
        let bytes_read = reader.read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }
        // Trim trailing \n and \r.
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.is_empty() {
            continue;
        }

        if line[0] == b'>' {
            if let Some(rec) = current.take() {
                rec.write_row(&mut out)?;
            }
            current = Some(RecordStats {
                name: header_name(&line),
                length: 0,
                n_count: 0,
                masked_count: 0,
            });
        } else if let Some(rec) = current.as_mut() {
            for &b in &line {
                rec.length += 1;
                rec.masked_count += b.is_ascii_lowercase() as u64;
                rec.n_count += ((b | 0x20) == b'n') as u64;
            }
        }
    }

    if let Some(rec) = current.take() {
        rec.write_row(&mut out)?;
    }

    out.flush()
}

fn main() -> ExitCode {
    if let Err(e) = run() {
        eprintln!("fastar-stats: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
