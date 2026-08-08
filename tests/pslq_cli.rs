use serde_json::Value;
use std::process::Command;

fn run_pslq(denominators: &str, precision: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_pi_arctan"))
        .args([
            "pslq",
            "--denominators",
            denominators,
            "--precision",
            precision,
            "--max-coeff",
            "1000000",
        ])
        .output()
        .expect("run pi_arctan");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON output")
}

#[test]
fn pslq_recovers_machin_formula() {
    let result = run_pslq("5,239", "50");
    assert_eq!(result["relation"], serde_json::json!([1, -4, 1]));
    assert_eq!(result["exact"], true);
}

#[test]
fn pslq_recovers_four_term_formula() {
    let result = run_pslq("49,57,239,110443", "100");
    assert_eq!(result["relation"], serde_json::json!([1, -12, -32, 5, -12]));
    assert_eq!(result["formula"].as_array().unwrap().len(), 4);
    assert_eq!(result["exact"], true);
}

#[test]
fn pslq_accepts_denominator_ranges() {
    let result = run_pslq("5,239-239", "50");
    assert_eq!(result["denominators"], serde_json::json!([5, 239]));
    assert_eq!(result["exact"], true);
}

#[test]
fn pslq_deflates_target_free_relations() {
    let result = run_pslq("2-10", "100");
    assert_eq!(result["exact"], true);
    assert_eq!(result["formulas"].as_array().unwrap().len(), 2);
    assert!(result["deflations"].as_array().unwrap().len() >= 3);
    assert_eq!(result["deflations"][0]["removed_denominator"], 7);
    assert_eq!(
        result["formula"],
        serde_json::json!([
            { "coefficient": 1, "denominator": 2 },
            { "coefficient": 1, "denominator": 5 },
            { "coefficient": 1, "denominator": 8 }
        ])
    );
    assert_eq!(
        result["formulas"][1]["formula"],
        serde_json::json!([
            { "coefficient": 1, "denominator": 2 },
            { "coefficient": 1, "denominator": 3 }
        ])
    );
}
