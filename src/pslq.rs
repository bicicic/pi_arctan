//! Fixed-point PSLQ integer-relation detection.
//!
//! This follows the Ferguson-Bailey-Arno PSLQ reduction, using `BigInt`
//! values scaled by `2^precision_bits`. Integer arithmetic keeps the matrix
//! transformations deterministic and avoids binary floating-point limits.

use num_bigint::{BigInt, BigUint};
use num_traits::{One, Signed, ToPrimitive, Zero};

#[derive(Clone, Debug)]
pub struct PslqConfig {
    pub precision_bits: usize,
    pub target_bits: usize,
    pub max_coefficient: i64,
    pub max_iterations: usize,
}

#[derive(Clone, Debug)]
pub struct PslqRelation {
    pub coefficients: Vec<i64>,
    pub iterations: usize,
    pub residual_bits: usize,
}

fn sqrt_fixed(value: &BigInt, precision: usize) -> BigInt {
    debug_assert!(!value.is_negative());
    let shifted: BigUint = (value << precision).to_biguint().unwrap_or_default();
    BigInt::from(shifted.sqrt())
}

fn round_fixed(value: &BigInt, precision: usize) -> BigInt {
    if precision == 0 {
        return value.clone();
    }
    ((value + (BigInt::one() << (precision - 1))) >> precision) << precision
}

fn identity_matrix(n: usize, one: &BigInt) -> Vec<Vec<BigInt>> {
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if i == j { one.clone() } else { BigInt::zero() })
                .collect()
        })
        .collect()
}

fn residual_bits(error: &BigInt, precision: usize) -> usize {
    if error.is_zero() {
        return precision;
    }
    let bits = error.abs().to_biguint().map_or(0, |v| v.bits() as usize);
    precision.saturating_sub(bits)
}

/// Finds one integer relation among equally-scaled, nonzero fixed-point values.
#[allow(dead_code)]
pub fn find_relation(input: &[BigInt], config: &PslqConfig) -> Option<PslqRelation> {
    find_relation_with_progress(input, config, |_, _| {})
}

/// Finds one relation and periodically reports `(iteration, residual_bits)`.
pub fn find_relation_with_progress<F>(
    input: &[BigInt],
    config: &PslqConfig,
    mut progress: F,
) -> Option<PslqRelation>
where
    F: FnMut(usize, usize),
{
    let n = input.len();
    let precision = config.precision_bits;
    if n < 2 || precision < 53 || config.target_bits >= precision {
        return None;
    }
    if input.iter().any(BigInt::is_zero) {
        return None;
    }

    let one = BigInt::one() << precision;
    let tolerance = BigInt::one() << (precision - config.target_bits);
    let mut a = identity_matrix(n, &one);
    let mut b = identity_matrix(n, &one);
    let mut h = vec![vec![BigInt::zero(); n]; n];

    let mut s = vec![BigInt::zero(); n];
    for k in 0..n {
        let sum = input[k..]
            .iter()
            .map(|x| (x * x) >> precision)
            .fold(BigInt::zero(), |acc, x| acc + x);
        s[k] = sqrt_fixed(&sum, precision);
    }
    let norm = s[0].clone();
    if norm.is_zero() {
        return None;
    }
    let mut y = input.to_vec();
    for k in 0..n {
        y[k] = (&input[k] << precision) / &norm;
        s[k] = (&s[k] << precision) / &norm;
    }

    for i in 0..n {
        if i < n - 1 && !s[i].is_zero() {
            h[i][i] = (&s[i + 1] << precision) / &s[i];
        }
        for j in 0..i {
            let denominator = &s[j] * &s[j + 1];
            if !denominator.is_zero() {
                h[i][j] = ((-&y[i] * &y[j]) << precision) / denominator;
            }
        }
    }

    reduce_rows(1, n, n - 1, precision, &mut y, &mut h, &mut a, &mut b);

    // gamma = sqrt(4/3), represented in the same fixed-point format.
    let gamma_argument = (BigInt::from(4) << precision) / 3;
    let gamma = sqrt_fixed(&gamma_argument, precision);
    let max_coefficient = BigInt::from(config.max_coefficient.unsigned_abs());
    progress(0, 0);

    for iteration in 0..config.max_iterations {
        let mut m = 0;
        let mut largest = BigInt::from(-1);
        let mut gamma_power = gamma.clone();
        for i in 0..n - 1 {
            let score = (&gamma_power * h[i][i].abs()) >> (precision * i);
            if score > largest {
                largest = score;
                m = i;
            }
            gamma_power *= &gamma;
        }

        y.swap(m, m + 1);
        h.swap(m, m + 1);
        a.swap(m, m + 1);
        for row in &mut b {
            row.swap(m, m + 1);
        }

        if m <= n.saturating_sub(3) {
            let sum = ((&h[m][m] * &h[m][m] + &h[m][m + 1] * &h[m][m + 1]) >> precision).abs();
            let t0 = sqrt_fixed(&sum, precision);
            if t0.is_zero() {
                break;
            }
            let t1 = (&h[m][m] << precision) / &t0;
            let t2 = (&h[m][m + 1] << precision) / &t0;
            for row in h.iter_mut().skip(m) {
                let t3 = row[m].clone();
                let t4 = row[m + 1].clone();
                row[m] = (&t1 * &t3 + &t2 * &t4) >> precision;
                row[m + 1] = (-&t2 * t3 + &t1 * t4) >> precision;
            }
        }

        reduce_rows(m + 1, n, m + 2, precision, &mut y, &mut h, &mut a, &mut b);

        let mut best_error: Option<BigInt> = None;
        for i in 0..n {
            let error = y[i].abs();
            if best_error.as_ref().is_none_or(|best| error < *best) {
                best_error = Some(error.clone());
            }
            if error < tolerance {
                let coefficients: Option<Vec<i64>> = (0..n)
                    .map(|j| {
                        let rounded = round_fixed(&b[j][i], precision) >> precision;
                        rounded.to_i64()
                    })
                    .collect();
                if let Some(coefficients) = coefficients {
                    let largest = coefficients
                        .iter()
                        .map(|c| c.unsigned_abs())
                        .max()
                        .unwrap_or(0);
                    if BigInt::from(largest) <= max_coefficient && largest > 0 {
                        let bits = residual_bits(&error, precision);
                        progress(iteration + 1, bits);
                        return Some(PslqRelation {
                            coefficients,
                            iterations: iteration + 1,
                            residual_bits: bits,
                        });
                    }
                }
            }
        }
        if (iteration + 1) % 25 == 0 {
            let bits = best_error
                .as_ref()
                .map_or(0, |error| residual_bits(error, precision));
            progress(iteration + 1, bits);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn reduce_rows(
    start: usize,
    n: usize,
    column_limit: usize,
    precision: usize,
    y: &mut [BigInt],
    h: &mut [Vec<BigInt>],
    a: &mut [Vec<BigInt>],
    b: &mut [Vec<BigInt>],
) {
    for i in start..n {
        let upper = (i - 1).min(column_limit.saturating_sub(1));
        for j in (0..=upper).rev() {
            if h[j][j].is_zero() {
                continue;
            }
            let quotient = (&h[i][j] << precision) / &h[j][j];
            let t = round_fixed(&quotient, precision);
            y[j] += (&t * &y[i]) >> precision;
            for k in 0..=j {
                let delta = (&t * &h[j][k]) >> precision;
                h[i][k] -= delta;
            }
            for k in 0..n {
                let delta_a = (&t * &a[j][k]) >> precision;
                a[i][k] -= delta_a;
                let delta_b = (&t * &b[k][i]) >> precision;
                b[k][j] += delta_b;
            }
        }
    }
}
