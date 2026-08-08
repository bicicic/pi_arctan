use crate::core::{
    Term, exact_pi_over_four, fixed_arctan_reciprocal, normalize_relation, relation_to_formula,
};
use crate::pslq::{self, PslqConfig};
use num_bigint::BigInt;
use num_traits::One;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DeflationStep {
    pub relation: Vec<i64>,
    pub removed_denominator: u64,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct FormulaMatch {
    pub stage: usize,
    pub relation: Vec<i64>,
    pub formula: Vec<Term>,
    pub iterations: usize,
    pub residual_bits: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchProgress {
    pub phase: &'static str,
    pub stage: usize,
    pub iteration: usize,
    pub max_iterations: usize,
    pub residual_bits: usize,
    pub active_dimensions: usize,
    pub removed_denominator: Option<u64>,
    pub formulas_found: usize,
    pub formula: Option<Vec<Term>>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub denominators: Vec<u64>,
    pub active_denominators: Vec<u64>,
    pub relation: Option<Vec<i64>>,
    pub formula: Option<Vec<Term>>,
    pub exact: bool,
    pub precision_digits: usize,
    pub iterations: usize,
    pub residual_bits: usize,
    pub deflations: Vec<DeflationStep>,
    pub formulas: Vec<FormulaMatch>,
    pub note: String,
}

#[allow(clippy::too_many_arguments)]
fn result(
    denominators: Vec<u64>,
    active_denominators: Vec<u64>,
    precision_digits: usize,
    iterations: usize,
    residual_bits: usize,
    deflations: Vec<DeflationStep>,
    formulas: Vec<FormulaMatch>,
    note: String,
) -> SearchResult {
    SearchResult {
        relation: formulas.first().map(|found| found.relation.clone()),
        formula: formulas.first().map(|found| found.formula.clone()),
        exact: !formulas.is_empty(),
        denominators,
        active_denominators,
        precision_digits,
        iterations,
        residual_bits,
        deflations,
        formulas,
        note,
    }
}

pub fn search<F>(
    denominators: Vec<u64>,
    precision_digits: usize,
    max_coefficient: i64,
    max_iterations: usize,
    mut progress: F,
) -> Result<SearchResult, String>
where
    F: FnMut(SearchProgress),
{
    if precision_digits < 20 || precision_digits > 2_000 {
        return Err("precision must be between 20 and 2000 digits".to_string());
    }
    if max_coefficient <= 0 || max_iterations == 0 {
        return Err("coefficient and iteration limits must be positive".to_string());
    }

    let mut active_denominators = denominators.clone();
    let target_bits = ((precision_digits as f64) * std::f64::consts::LOG2_10).ceil() as usize;
    let precision_bits = target_bits + 64;
    let scale = BigInt::one() << precision_bits;
    let pi_over_four =
        fixed_arctan_reciprocal(5, &scale) * 4 - fixed_arctan_reciprocal(239, &scale);
    let mut values = Vec::with_capacity(active_denominators.len() + 1);
    values.push(pi_over_four);
    values.extend(
        active_denominators
            .iter()
            .map(|denominator| fixed_arctan_reciprocal(*denominator, &scale)),
    );
    let config = PslqConfig {
        precision_bits,
        target_bits,
        max_coefficient,
        max_iterations,
    };
    let mut deflations = Vec::new();
    let mut formulas = Vec::new();
    let mut total_iterations = 0;
    let mut stage = 0;

    loop {
        stage += 1;
        progress(SearchProgress {
            phase: "pslq",
            stage,
            iteration: 0,
            max_iterations,
            residual_bits: 0,
            active_dimensions: values.len(),
            removed_denominator: None,
            formulas_found: formulas.len(),
            formula: None,
            message: format!("段階{stage}: {}次元のPSLQを開始", values.len()),
        });

        let mut stage_iterations = 0;
        let found =
            pslq::find_relation_with_progress(&values, &config, |iteration, residual_bits| {
                stage_iterations = iteration;
                progress(SearchProgress {
                    phase: "pslq",
                    stage,
                    iteration,
                    max_iterations,
                    residual_bits,
                    active_dimensions: values.len(),
                    removed_denominator: None,
                    formulas_found: formulas.len(),
                    formula: None,
                    message: format!("段階{stage}: PSLQ反復 {iteration}/{max_iterations}"),
                });
            });
        total_iterations += stage_iterations;

        let Some(found) = found else {
            let note = if formulas.is_empty() {
                "指定した範囲では整数関係が見つかりませんでした".to_string()
            } else {
                format!(
                    "{}件の公式を発見し、残りの基底では探索を終了しました",
                    formulas.len()
                )
            };
            progress(SearchProgress {
                phase: "complete",
                stage,
                iteration: stage_iterations,
                max_iterations,
                residual_bits: 0,
                active_dimensions: values.len(),
                removed_denominator: None,
                formulas_found: formulas.len(),
                formula: None,
                message: note.clone(),
            });
            return Ok(result(
                denominators,
                active_denominators,
                precision_digits,
                total_iterations,
                0,
                deflations,
                formulas,
                note,
            ));
        };

        let relation = normalize_relation(found.coefficients);
        if relation[0] == 0 {
            let remove_index = relation[1..]
                .iter()
                .rposition(|coefficient| *coefficient != 0)
                .ok_or_else(|| "PSLQ returned an invalid zero relation".to_string())?;
            let removed_denominator = active_denominators.remove(remove_index);
            values.remove(remove_index + 1);
            deflations.push(DeflationStep {
                relation,
                removed_denominator,
                reason: "target_free_relation",
            });
            progress(SearchProgress {
                phase: "deflation",
                stage,
                iteration: found.iterations,
                max_iterations,
                residual_bits: found.residual_bits,
                active_dimensions: values.len(),
                removed_denominator: Some(removed_denominator),
                formulas_found: formulas.len(),
                formula: None,
                message: format!(
                    "πを含まない関係を除去: 分母 {removed_denominator} を基底から外します"
                ),
            });
            if active_denominators.is_empty() {
                let note = format!(
                    "デフレーションによって基底が尽きるまでに{}件の公式を発見しました",
                    formulas.len()
                );
                return Ok(result(
                    denominators,
                    active_denominators,
                    precision_digits,
                    total_iterations,
                    found.residual_bits,
                    deflations,
                    formulas,
                    note,
                ));
            }
            continue;
        }

        let formula = relation_to_formula(&relation, &active_denominators);
        if formula
            .as_deref()
            .is_none_or(|formula| !exact_pi_over_four(formula))
        {
            let note = if formula.is_none() {
                "π/4について解いた関係の係数が整数になりませんでした"
            } else {
                "数値的な関係がガウス整数による厳密検証に失敗しました"
            };
            return Ok(result(
                denominators,
                active_denominators,
                precision_digits,
                total_iterations,
                found.residual_bits,
                deflations,
                formulas,
                note.to_string(),
            ));
        }

        let formula = formula.expect("validated above");
        formulas.push(FormulaMatch {
            stage,
            relation: relation.clone(),
            formula: formula.clone(),
            iterations: found.iterations,
            residual_bits: found.residual_bits,
        });
        progress(SearchProgress {
            phase: "formula",
            stage,
            iteration: found.iterations,
            max_iterations,
            residual_bits: found.residual_bits,
            active_dimensions: values.len(),
            removed_denominator: None,
            formulas_found: formulas.len(),
            formula: Some(formula),
            message: format!("{}件目の厳密な公式を発見しました", formulas.len()),
        });

        let remove_index = relation[1..]
            .iter()
            .rposition(|coefficient| *coefficient != 0)
            .ok_or_else(|| "PSLQ formula relation has no arctangent term".to_string())?;
        let removed_denominator = active_denominators.remove(remove_index);
        values.remove(remove_index + 1);
        deflations.push(DeflationStep {
            relation,
            removed_denominator,
            reason: "found_formula",
        });
        progress(SearchProgress {
            phase: "deflation",
            stage,
            iteration: found.iterations,
            max_iterations,
            residual_bits: found.residual_bits,
            active_dimensions: values.len(),
            removed_denominator: Some(removed_denominator),
            formulas_found: formulas.len(),
            formula: None,
            message: format!("次の公式を探すため、分母 {removed_denominator} を基底から外します"),
        });
        if active_denominators.is_empty() {
            let note = format!("基底が尽きるまでに{}件の公式を発見しました", formulas.len());
            return Ok(result(
                denominators,
                active_denominators,
                precision_digits,
                total_iterations,
                found.residual_bits,
                deflations,
                formulas,
                note,
            ));
        }
    }
}
