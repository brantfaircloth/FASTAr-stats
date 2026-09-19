use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Deserialize)]
struct RecordTruth {
    length: u64,
    n_count: u64,
    masked_count: u64,
}

// `_file_summary` has a different shape than a per-record entry, so pull the
// raw JSON in as untyped values and only typecheck the per-record ones.
fn load_truth() -> HashMap<String, serde_json::Value> {
    let text = std::fs::read_to_string("tests/fixtures/sample_answer_key.json")
        .expect("truth set fixture must be readable");
    serde_json::from_str(&text).expect("truth set fixture must be valid JSON")
}

fn pct(count: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (count as f64 / total as f64) * 100.0
    }
}

#[test]
fn matches_truth_set_per_record() {
    let truth = load_truth();

    let output = Command::new(env!("CARGO_BIN_EXE_fastar-stats"))
        .arg("tests/fixtures/sample.fasta")
        .output()
        .expect("binary must run");
    assert!(output.status.success(), "binary exited non-zero");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let mut lines = stdout.lines();

    assert_eq!(
        lines.next(),
        Some("name\tnum_bases\tnum_n\tnum_masked\tpct_n\tpct_masked"),
        "header row must match the expected column layout"
    );

    let mut seen = 0usize;
    for line in lines {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 6, "row must have 6 columns: {line}");

        let name = cols[0];
        let expected = truth
            .get(name)
            .unwrap_or_else(|| panic!("truth set has no entry for record `{name}`"));
        let expected: RecordTruth =
            serde_json::from_value(expected.clone()).expect("per-record entry must parse");

        let got_bases: u64 = cols[1].parse().unwrap();
        let got_n: u64 = cols[2].parse().unwrap();
        let got_masked: u64 = cols[3].parse().unwrap();
        let got_pct_n: f64 = cols[4].parse().unwrap();
        let got_pct_masked: f64 = cols[5].parse().unwrap();

        assert_eq!(got_bases, expected.length, "num_bases mismatch for {name}");
        assert_eq!(got_n, expected.n_count, "num_n mismatch for {name}");
        assert_eq!(
            got_masked, expected.masked_count,
            "num_masked mismatch for {name}"
        );

        let want_pct_n = format!("{:.2}", pct(expected.n_count, expected.length));
        let want_pct_masked = format!("{:.2}", pct(expected.masked_count, expected.length));
        assert_eq!(cols[4], want_pct_n, "pct_n mismatch for {name}");
        assert_eq!(cols[5], want_pct_masked, "pct_masked mismatch for {name}");
        // sanity: the parsed values round-trip through the same formatting.
        assert_eq!(format!("{got_pct_n:.2}"), cols[4]);
        assert_eq!(format!("{got_pct_masked:.2}"), cols[5]);

        seen += 1;
    }

    let expected_record_count = truth.len() - 1; // minus `_file_summary`
    assert_eq!(
        seen, expected_record_count,
        "expected one output row per truth-set record"
    );
}
