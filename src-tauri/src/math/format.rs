//! Human-friendly number formatting: integers stay integers, float noise is hidden,
//! very large or small values use scientific notation.

pub fn format_number(v: f64) -> String {
    if v.is_nan() {
        return "undefined".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞" } else { "-∞" }.to_string();
    }
    let v = if v == 0.0 { 0.0 } else { v }; // normalise -0
    let abs = v.abs();

    if abs != 0.0 && !(1e-6..1e15).contains(&abs) {
        let s = format!("{:.8e}", v);
        let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
        return format!("{}e{}", trim_fraction(mantissa), exponent);
    }
    if v.fract() == 0.0 {
        return format!("{}", v as i64);
    }
    let int_digits = if abs >= 1.0 {
        abs.log10().floor() as usize + 1
    } else {
        0
    };
    let decimals = 12usize.saturating_sub(int_digits).max(1);
    trim_fraction(&format!("{:.*}", decimals, v))
}

fn trim_fraction(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::format_number as f;

    #[test]
    fn integers_and_zero() {
        assert_eq!(f(4.0), "4");
        assert_eq!(f(-0.0), "0");
        assert_eq!(f(123456789012345.0), "123456789012345");
    }

    #[test]
    fn hides_float_noise() {
        assert_eq!(f(0.1 + 0.2), "0.3");
        assert_eq!(f(3.3 * 3.0), "9.9");
        assert_eq!(f(2.9999999999999996), "3");
        assert_eq!(f(1.0 / 3.0), "0.333333333333");
        assert_eq!(f(1234567.891), "1234567.891");
    }

    #[test]
    fn scientific_for_extremes() {
        assert_eq!(f(1e20), "1e20");
        assert_eq!(f(1.5e-9), "1.5e-9");
        assert_eq!(f(0.000001), "0.000001");
        assert_eq!(f(f64::INFINITY), "∞");
    }
}
