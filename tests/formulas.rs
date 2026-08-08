use pi_arctan::core::{exact_pi_over_four, parse_formula};

#[test]
fn known_machin_formula_is_exact() {
    let formula = parse_formula("4:5,-1:239").unwrap();
    assert!(exact_pi_over_four(&formula));
}

#[test]
fn invalid_formula_is_rejected() {
    let formula = parse_formula("1:5,1:239").unwrap();
    assert!(!exact_pi_over_four(&formula));
}
