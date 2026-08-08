use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use num_bigint::BigInt;
use pi_arctan::core::{
    Term, exact_pi_over_four, fixed_arctan_reciprocal, parse_denominators, parse_formula,
};
use pi_arctan::machin;
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "pi_arctan",
    about = "Search and benchmark Machin-type formulas for pi"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Verify and benchmark one formula, e.g. "4:5,-1:239".
    Verify {
        formula: String,
        #[arg(long, default_value_t = 10_000)]
        digits: usize,
    },
    /// Find integer relations among pi/4 and selected atan(1/n) values.
    Pslq {
        /// Comma-separated denominators and inclusive ranges, e.g. 5,239 or 2-10.
        #[arg(long)]
        denominators: String,
        #[arg(long, default_value_t = 100)]
        precision: usize,
        #[arg(long, default_value_t = 1_000_000)]
        max_coeff: i64,
        #[arg(long, default_value_t = 10_000)]
        max_iterations: usize,
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize)]
struct ResultRecord {
    formula: Vec<Term>,
    exact: bool,
    estimated_digits_per_term: f64,
    digits: usize,
    elapsed_ms: u128,
    prefix: String,
}

#[derive(Serialize)]
struct TimedPslqResult<'a> {
    #[serde(flatten)]
    result: &'a machin::SearchResult,
    elapsed_ms: u128,
}

fn benchmark(terms: &[Term], digits: usize) -> Result<(u128, String)> {
    if digits == 0 {
        bail!("digits must be positive");
    }
    let scale = BigInt::from(10_u8).pow((digits + 12) as u32);
    let started = Instant::now();
    let sum: BigInt = terms
        .par_iter()
        .map(|term| fixed_arctan_reciprocal(term.denominator, &scale) * term.coefficient)
        .sum();
    let pi: BigInt = sum * 4;
    let text = pi.to_string();
    let fractional_digits = digits + 12;
    let split = text.len().saturating_sub(fractional_digits);
    let prefix = if split > 0 {
        format!(
            "{}.{}",
            &text[..split],
            &text[split..text.len().min(split + 51)]
        )
    } else {
        text
    };
    Ok((started.elapsed().as_millis(), prefix))
}

fn estimate(terms: &[Term]) -> f64 {
    terms
        .iter()
        .map(|term| (term.denominator as f64).log10() * 2.0)
        .sum::<f64>()
        / terms.len() as f64
}

fn run_verify(formula: &str, digits: usize) -> Result<()> {
    let terms = parse_formula(formula).map_err(anyhow::Error::msg)?;
    let (elapsed_ms, prefix) = benchmark(&terms, digits)?;
    let record = ResultRecord {
        exact: exact_pi_over_four(&terms),
        estimated_digits_per_term: estimate(&terms),
        formula: terms,
        digits,
        elapsed_ms,
        prefix,
    };
    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}

fn run_pslq(
    denominator_specification: &str,
    precision_digits: usize,
    max_coefficient: i64,
    max_iterations: usize,
    output: Option<String>,
) -> Result<()> {
    let denominators = parse_denominators(denominator_specification).map_err(anyhow::Error::msg)?;
    let started = Instant::now();
    let result = machin::search(
        denominators,
        precision_digits,
        max_coefficient,
        max_iterations,
        |progress| {
            if progress.iteration > 0 && progress.iteration % 250 == 0 {
                eprintln!("{}", progress.message);
            }
        },
    )
    .map_err(anyhow::Error::msg)?;
    let timed = TimedPslqResult {
        result: &result,
        elapsed_ms: started.elapsed().as_millis(),
    };
    let json = serde_json::to_string_pretty(&timed)?;
    if let Some(path) = output {
        fs::write(path, json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Verify { formula, digits } => run_verify(&formula, digits),
        Command::Pslq {
            denominators,
            precision,
            max_coeff,
            max_iterations,
            output,
        } => run_pslq(&denominators, precision, max_coeff, max_iterations, output),
    }
}
