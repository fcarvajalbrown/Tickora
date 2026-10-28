pub fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted_percentile(&sorted, q)
}

pub fn sorted_percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let position = q / 100.0 * (sorted.len() - 1) as f64;
    let low = position.floor() as usize;
    let high = (low + 1).min(sorted.len() - 1);
    let fraction = position - low as f64;
    sorted[low] + (sorted[high] - sorted[low]) * fraction
}

pub fn median(values: &[f64]) -> f64 {
    percentile(values, 50.0)
}

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn power_mean_db(levels: &[f64]) -> f64 {
    let powers: Vec<f64> = levels
        .iter()
        .map(|level| 10f64.powf(level / 10.0))
        .collect();
    10.0 * mean(&powers).log10()
}

pub fn round_to(value: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    (value * scale).round_ties_even() / scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_interpolate_linearly_like_numpy() {
        let values = [3.0, 1.0, 4.0, 1.0, 5.0];
        assert_eq!(median(&values), 3.0);
        assert_eq!(percentile(&values, 10.0), 1.0);
        assert!((percentile(&values, 90.0) - 4.6).abs() < 1e-12);
        assert_eq!(median(&[1.0, 2.0]), 1.5);
    }

    #[test]
    fn rounding_goes_to_even_on_ties() {
        assert_eq!(round_to(2.5, 0), 2.0);
        assert_eq!(round_to(3.5, 0), 4.0);
        assert_eq!(round_to(-1.25, 1), -1.2);
    }
}
