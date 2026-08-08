use crate::core::{Gaussian, Term};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};
use serde::Serialize;
use std::collections::HashMap;

const MAX_PARTIAL_STATES: u64 = 10_000_000;

#[derive(Clone, Debug, Serialize)]
pub struct ExactFormula {
    pub formula: Vec<Term>,
}

#[derive(Debug, Serialize)]
pub struct ExactSearchResult {
    pub denominators: Vec<u64>,
    pub formulas: Vec<ExactFormula>,
    pub exact: bool,
    pub processed_states: u64,
    pub total_states: u64,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct ExactProgress {
    pub phase: &'static str,
    pub processed: u64,
    pub total: u64,
    pub formulas_found: usize,
    pub formula: Option<Vec<Term>>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DirectionKey {
    re: BigInt,
    im: BigInt,
}

impl DirectionKey {
    fn from_gaussian(value: &Gaussian) -> Self {
        let gcd = value.re.abs().gcd(&value.im.abs());
        if gcd.is_zero() {
            return Self {
                re: value.re.clone(),
                im: value.im.clone(),
            };
        }
        Self {
            re: &value.re / &gcd,
            im: &value.im / gcd,
        }
    }

    fn target_after(&self) -> Self {
        let desired = Gaussian {
            re: &self.re + &self.im,
            im: &self.re - &self.im,
        };
        Self::from_gaussian(&desired)
    }
}

#[derive(Clone, Debug)]
struct Partial {
    direction: DirectionKey,
    terms: Vec<Term>,
    angle: f64,
}

fn choose(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (0..k).fold(1_u128, |value, i| value * (n - i) as u128 / (i + 1) as u128)
}

fn state_count(n: usize, max_terms: usize, coefficient_limit: u32) -> u64 {
    let choices = u128::from(coefficient_limit) * 2;
    let count = (0..=max_terms.min(n)).fold(0_u128, |total, terms| {
        total.saturating_add(choose(n, terms).saturating_mul(choices.saturating_pow(terms as u32)))
    });
    count.min(u128::from(u64::MAX)) as u64
}

fn contributions(denominators: &[u64], coefficient_limit: u32) -> Vec<Vec<(i64, Gaussian)>> {
    denominators
        .iter()
        .map(|denominator| {
            let positive = Gaussian::new(BigInt::from(*denominator), BigInt::one());
            let negative = Gaussian::new(BigInt::from(*denominator), -BigInt::one());
            (1..=coefficient_limit)
                .flat_map(|coefficient| {
                    let coefficient = i64::from(coefficient);
                    [
                        (coefficient, Gaussian::pow(&positive, coefficient as u64)),
                        (-coefficient, Gaussian::pow(&negative, coefficient as u64)),
                    ]
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn enumerate<F>(
    denominators: &[u64],
    choices: &[Vec<(i64, Gaussian)>],
    start: usize,
    max_terms: usize,
    product: &Gaussian,
    terms: &mut Vec<Term>,
    angle: f64,
    visit: &mut F,
) where
    F: FnMut(Partial),
{
    visit(Partial {
        direction: DirectionKey::from_gaussian(product),
        terms: terms.clone(),
        angle,
    });
    if terms.len() == max_terms {
        return;
    }

    for index in start..denominators.len() {
        for (coefficient, contribution) in &choices[index] {
            terms.push(Term {
                coefficient: *coefficient,
                denominator: denominators[index],
            });
            enumerate(
                denominators,
                choices,
                index + 1,
                max_terms,
                &product.multiply(contribution),
                terms,
                angle + (*coefficient as f64) * (1.0 / denominators[index] as f64).atan(),
                visit,
            );
            terms.pop();
        }
    }
}

pub fn search<F>(
    denominators: Vec<u64>,
    max_terms: usize,
    coefficient_limit: u32,
    mut progress: F,
) -> Result<ExactSearchResult, String>
where
    F: FnMut(ExactProgress),
{
    if denominators.is_empty() {
        return Err("分母を1つ以上指定してください".to_string());
    }
    if max_terms == 0 || coefficient_limit == 0 {
        return Err("最大項数と係数上限は正の整数で指定してください".to_string());
    }
    let split = denominators.len() / 2;
    let (left_denominators, right_denominators) = denominators.split_at(split);
    let left_count = state_count(left_denominators.len(), max_terms, coefficient_limit);
    let right_count = state_count(right_denominators.len(), max_terms, coefficient_limit);
    if left_count > MAX_PARTIAL_STATES || right_count > MAX_PARTIAL_STATES {
        return Err(format!(
            "部分積が多すぎます（左{left_count}、右{right_count}）。範囲・最大項数・係数上限を小さくしてください"
        ));
    }
    let total = left_count.saturating_add(right_count);
    let left_choices = contributions(left_denominators, coefficient_limit);
    let right_choices = contributions(right_denominators, coefficient_limit);
    let mut processed = 0_u64;
    let mut right_map: HashMap<DirectionKey, Vec<Partial>> = HashMap::new();

    progress(ExactProgress {
        phase: "generate",
        processed,
        total,
        formulas_found: 0,
        formula: None,
        message: "右側の部分積を生成しています".to_string(),
    });
    enumerate(
        right_denominators,
        &right_choices,
        0,
        max_terms,
        &Gaussian::one(),
        &mut Vec::new(),
        0.0,
        &mut |partial| {
            processed += 1;
            right_map
                .entry(partial.direction.clone())
                .or_default()
                .push(partial);
            if processed % 1_000 == 0 {
                progress(ExactProgress {
                    phase: "generate",
                    processed,
                    total,
                    formulas_found: 0,
                    formula: None,
                    message: "右側の部分積を生成しています".to_string(),
                });
            }
        },
    );

    let mut formulas = Vec::new();
    let target = std::f64::consts::FRAC_PI_4;
    enumerate(
        left_denominators,
        &left_choices,
        0,
        max_terms,
        &Gaussian::one(),
        &mut Vec::new(),
        0.0,
        &mut |left| {
            processed += 1;
            if let Some(rights) = right_map.get(&left.direction.target_after()) {
                for right in rights {
                    let support = left.terms.len() + right.terms.len();
                    if support == 0 || support > max_terms {
                        continue;
                    }
                    if (left.angle + right.angle - target).abs() > 1e-12 {
                        continue;
                    }
                    let mut formula = left.terms.clone();
                    formula.extend(right.terms.clone());
                    formulas.push(ExactFormula {
                        formula: formula.clone(),
                    });
                    progress(ExactProgress {
                        phase: "formula",
                        processed,
                        total,
                        formulas_found: formulas.len(),
                        formula: Some(formula),
                        message: format!("{}件目の公式を発見しました", formulas.len()),
                    });
                }
            }
            if processed % 1_000 == 0 {
                progress(ExactProgress {
                    phase: "match",
                    processed,
                    total,
                    formulas_found: formulas.len(),
                    formula: None,
                    message: "左右の部分積を照合しています".to_string(),
                });
            }
        },
    );
    processed = total;
    let note = format!(
        "指定条件の全探索が完了し、{}件の公式を発見しました",
        formulas.len()
    );
    progress(ExactProgress {
        phase: "complete",
        processed,
        total,
        formulas_found: formulas.len(),
        formula: None,
        message: note.clone(),
    });
    Ok(ExactSearchResult {
        denominators,
        exact: !formulas.is_empty(),
        formulas,
        processed_states: processed,
        total_states: total,
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustive_search_finds_two_term_identity() {
        let result = search(vec![2, 3], 2, 1, |_| {}).unwrap();
        assert_eq!(result.formulas.len(), 1);
        assert_eq!(result.formulas[0].formula.len(), 2);
        assert_eq!(result.formulas[0].formula[0].coefficient, 1);
        assert_eq!(result.formulas[0].formula[0].denominator, 2);
        assert_eq!(result.formulas[0].formula[1].coefficient, 1);
        assert_eq!(result.formulas[0].formula[1].denominator, 3);
        assert_eq!(result.processed_states, result.total_states);
    }
}
