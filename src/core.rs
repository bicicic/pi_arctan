use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Term {
    pub coefficient: i64,
    pub denominator: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct Gaussian {
    pub(crate) re: BigInt,
    pub(crate) im: BigInt,
}

impl Gaussian {
    pub(crate) fn new(re: BigInt, im: BigInt) -> Self {
        Self { re, im }
    }

    pub(crate) fn one() -> Self {
        Self::new(BigInt::one(), BigInt::zero())
    }

    pub(crate) fn multiply(&self, other: &Self) -> Self {
        Self {
            re: &self.re * &other.re - &self.im * &other.im,
            im: &self.re * &other.im + &self.im * &other.re,
        }
    }

    pub(crate) fn conjugate(&self) -> Self {
        Self::new(self.re.clone(), -&self.im)
    }

    pub(crate) fn pow(base: &Self, mut exponent: u64) -> Self {
        let mut result = Self::one();
        let mut factor = base.clone();
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result.multiply(&factor);
            }
            factor = factor.multiply(&factor);
            exponent >>= 1;
        }
        result
    }
}

pub fn parse_formula(input: &str) -> Result<Vec<Term>, String> {
    let mut terms = Vec::new();
    for part in input.split(',') {
        let (coefficient, denominator) = part
            .split_once(':')
            .ok_or_else(|| "expected coefficient:denominator".to_string())?;
        let coefficient: i64 = coefficient
            .trim()
            .parse()
            .map_err(|_| format!("invalid coefficient: {coefficient}"))?;
        let denominator: u64 = denominator
            .trim()
            .parse()
            .map_err(|_| format!("invalid denominator: {denominator}"))?;
        if denominator == 0 || coefficient == 0 {
            return Err("coefficient and denominator must be nonzero".to_string());
        }
        terms.push(Term {
            coefficient,
            denominator,
        });
    }
    if terms.is_empty() {
        return Err("formula is empty".to_string());
    }
    Ok(terms)
}

pub fn parse_denominators(specification: &str) -> Result<Vec<u64>, String> {
    let mut values = Vec::new();
    for raw in specification.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            return Err("empty denominator in list".to_string());
        }
        if let Some((start, end)) = token.split_once('-') {
            let start: u64 = start
                .trim()
                .parse()
                .map_err(|_| format!("invalid denominator range: {token}"))?;
            let end: u64 = end
                .trim()
                .parse()
                .map_err(|_| format!("invalid denominator range: {token}"))?;
            if start == 0 || start > end {
                return Err(format!("invalid denominator range: {token}"));
            }
            if end - start > 999 {
                return Err("a range can contain at most 1000 denominators".to_string());
            }
            values.extend(start..=end);
        } else {
            let value: u64 = token
                .parse()
                .map_err(|_| format!("invalid denominator: {token}"))?;
            if value == 0 {
                return Err("denominators must be positive".to_string());
            }
            values.push(value);
        }
    }
    values.sort_unstable();
    values.dedup();
    if values.is_empty() {
        return Err("at least one denominator is required".to_string());
    }
    Ok(values)
}

pub fn fixed_arctan_reciprocal(denominator: u64, scale: &BigInt) -> BigInt {
    let denominator = BigInt::from(denominator);
    let denominator_squared = &denominator * &denominator;
    let mut power = scale / &denominator;
    let mut sum = power.clone();
    let mut index = 1_u64;
    loop {
        power = (&power * -1) / &denominator_squared;
        let term = &power / (2 * index + 1);
        if term.is_zero() {
            break;
        }
        sum += term;
        index += 1;
    }
    sum
}

pub fn normalize_relation(mut relation: Vec<i64>) -> Vec<i64> {
    let divisor = relation
        .iter()
        .fold(0_i64, |gcd, value| gcd.gcd(value))
        .abs()
        .max(1);
    for value in &mut relation {
        *value /= divisor;
    }
    if relation
        .iter()
        .find(|value| **value != 0)
        .is_some_and(|value| *value < 0)
    {
        for value in &mut relation {
            *value = -*value;
        }
    }
    relation
}

pub fn relation_to_formula(relation: &[i64], denominators: &[u64]) -> Option<Vec<Term>> {
    let target = *relation.first()?;
    if target == 0 {
        return None;
    }
    relation[1..]
        .iter()
        .zip(denominators)
        .filter(|(coefficient, _)| **coefficient != 0)
        .map(|(coefficient, denominator)| {
            (coefficient % target == 0).then_some(Term {
                coefficient: -coefficient / target,
                denominator: *denominator,
            })
        })
        .collect()
}

pub fn exact_pi_over_four(terms: &[Term]) -> bool {
    let mut positive = Gaussian::one();
    let mut negative = Gaussian::one();
    for term in terms {
        let base = Gaussian::new(BigInt::from(term.denominator), BigInt::one());
        let power = Gaussian::pow(&base, term.coefficient.unsigned_abs());
        if term.coefficient > 0 {
            positive = positive.multiply(&power);
        } else {
            negative = negative.multiply(&power);
        }
    }
    let quotient = positive.multiply(&negative.conjugate());
    quotient.re == quotient.im
}
