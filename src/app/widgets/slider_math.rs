//! Slider value logic, kept apart from drawing so it can be tested.

/// Decimal places shown for a range: whole numbers for wide ranges.
pub fn decimals(lo: f64, hi: f64) -> usize {
    let span = hi - lo;
    if span >= 100.0 {
        0
    } else if span >= 10.0 {
        1
    } else {
        2
    }
}

/// Position of `v` along the range, 0..=1 (0 for an empty range).
pub fn frac(v: f64, lo: f64, hi: f64) -> f32 {
    if hi <= lo { 0.0 } else { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 }
}

pub fn from_frac(t: f32, lo: f64, hi: f64) -> f64 {
    lo + t.clamp(0.0, 1.0) as f64 * (hi - lo)
}

/// The nearest `lo + k·step`, kept in range.
pub fn snap(v: f64, lo: f64, hi: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return v.clamp(lo, hi);
    }
    (lo + ((v - lo) / step).round() * step).clamp(lo, hi)
}

/// One arrow-key step: 1 % of the range, 10 % with Shift.
pub fn nudge(v: f64, lo: f64, hi: f64, dir: i8, big: bool) -> f64 {
    let by = (hi - lo) * if big { 0.1 } else { 0.01 };
    (v + by * dir as f64).clamp(lo, hi)
}

/// A typed value: the suffix is optional, a comma counts as a decimal
/// point, and anything that isn't a finite number is ignored.
pub fn parse_typed(text: &str, suffix: &str, lo: f64, hi: f64) -> Option<f64> {
    let t = text.trim();
    let t = t.strip_suffix(suffix.trim()).filter(|_| !suffix.trim().is_empty()).unwrap_or(t);
    let n: f64 = t.trim().replace(',', ".").parse().ok()?;
    n.is_finite().then(|| n.clamp(lo, hi))
}

pub fn format_value(v: f64, decimals: usize, suffix: &str) -> String {
    format!("{v:.decimals$}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn decimals_follow_the_span() {
        assert_eq!(decimals(5.0, 1500.0), 0);
        assert_eq!(decimals(0.0, 40.0), 1);
        assert_eq!(decimals(0.0, 1.0), 2);
    }

    #[test]
    fn frac_clamps_and_survives_empty_ranges() {
        assert_eq!(frac(2000.0, 5.0, 1500.0), 1.0);
        assert_eq!(frac(-3.0, 0.0, 1.0), 0.0);
        assert_eq!(frac(1.0, 3.0, 3.0), 0.0);
        assert!(close(from_frac(0.25, 0.0, 40.0), 10.0));
    }

    #[test]
    fn snap_goes_to_the_nearest_step() {
        assert!(close(snap(0.37, 0.0, 1.0, 0.25), 0.25));
        assert!(close(snap(0.38, 0.0, 1.0, 0.25), 0.5));
        assert!(close(snap(9.0, 0.0, 1.0, 0.25), 1.0));
    }

    #[test]
    fn nudge_moves_by_a_percent_or_ten() {
        assert!(close(nudge(0.5, 0.0, 1.0, 1, false), 0.51));
        assert!(close(nudge(0.5, 0.0, 1.0, -1, true), 0.4));
        assert!(close(nudge(0.95, 0.0, 1.0, 1, true), 1.0));
    }

    #[test]
    fn typed_values_strip_suffix_accept_commas_and_clamp() {
        assert_eq!(parse_typed("16 px", " px", 0.0, 60.0), Some(16.0));
        assert_eq!(parse_typed("16px", " px", 0.0, 60.0), Some(16.0));
        assert_eq!(parse_typed(" 0,5 ", "", 0.0, 1.0), Some(0.5));
        assert_eq!(parse_typed("99", "%", 0.0, 40.0), Some(40.0));
    }

    #[test]
    fn garbage_typed_values_are_ignored() {
        for bad in ["abc", "", "NaN", "inf", "1e400"] {
            assert_eq!(parse_typed(bad, "", 0.0, 1.0), None, "{bad}");
        }
    }

    #[test]
    fn values_format_with_their_suffix() {
        assert_eq!(format_value(16.24, 1, "%"), "16.2%");
        assert_eq!(format_value(90.0, 0, "°"), "90°");
        assert_eq!(format_value(0.5, 2, ""), "0.50");
    }
}
