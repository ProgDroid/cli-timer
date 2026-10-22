//! Parsing of timer durations: `hh:mm:ss` (as before) and short forms such as `90s` or `1h30m`.

use std::time::Duration;

/// Longest duration accepted anywhere in the crate (100 days).
pub const MAX_DURATION: Duration = Duration::from_hours(100 * 24);

const USAGE: &str = "use hh:mm:ss or forms like 90s, 20m, 1h30m";

#[derive(Debug, thiserror::Error)]
pub enum DurationError {
    #[error("invalid duration \"{0}\": {USAGE}")]
    Invalid(String),
    #[error("duration \"{0}\" is too long: the maximum is 100 days")]
    Overflow(String),
}

/// Parses a duration given as `hh:mm:ss` / `hh:mm` / `hh` (a bare number is hours), or as a short
/// form of `<n>h`, `<n>m` and `<n>s` parts, each at most once and in that order (`1h30m`, `90s`).
///
/// # Errors
/// Returns [`DurationError::Invalid`] when `arg` matches neither form, and
/// [`DurationError::Overflow`] when the value does not fit or exceeds [`MAX_DURATION`].
pub fn parse(arg: &str) -> Result<Duration, DurationError> {
    let seconds = if arg.contains(':') || is_digits(arg) {
        parse_colon(arg)
    } else {
        parse_short(arg)
    };

    match seconds {
        Ok(total) if Duration::from_secs(total) <= MAX_DURATION => Ok(Duration::from_secs(total)),
        Ok(_) | Err(Fault::Overflow) => Err(DurationError::Overflow(arg.to_owned())),
        Err(Fault::Malformed) => Err(DurationError::Invalid(arg.to_owned())),
    }
}

/// [`parse`] with the error as text, for use as a clap `value_parser`.
///
/// # Errors
/// Returns the [`DurationError`] message when [`parse`] fails.
pub fn parse_arg(arg: &str) -> Result<Duration, String> {
    parse(arg).map_err(|e| e.to_string())
}

/// Why a string could not be turned into seconds.
enum Fault {
    Malformed,
    Overflow,
}

/// `value * unit` added onto `total`, failing with [`Fault::Overflow`] instead of wrapping.
fn add_scaled(total: u64, digits: &str, unit: u64) -> Result<u64, Fault> {
    let value = digits.parse::<u64>().map_err(|_| Fault::Overflow)?;
    value
        .checked_mul(unit)
        .and_then(|scaled| total.checked_add(scaled))
        .ok_or(Fault::Overflow)
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Seconds of a `hh[:mm[:ss]]` string.
fn parse_colon(arg: &str) -> Result<u64, Fault> {
    const UNITS: [u64; 3] = [3600, 60, 1];

    let parts: Vec<&str> = arg.split(':').collect();
    if parts.len() > UNITS.len() || !parts.iter().all(|part| is_digits(part)) {
        return Err(Fault::Malformed);
    }

    parts
        .iter()
        .zip(UNITS)
        .try_fold(0_u64, |total, (part, unit)| add_scaled(total, part, unit))
}

/// Seconds of a short-form string.
fn parse_short(arg: &str) -> Result<u64, Fault> {
    const UNITS: [(char, u64); 3] = [('h', 3600), ('m', 60), ('s', 1)];

    // Check the whole shape first, so a malformed string is never reported as an overflow.
    let mut parts: Vec<(&str, u64)> = Vec::new();
    let mut rest = arg;
    let mut next_unit = 0;
    while !rest.is_empty() {
        let digits_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .ok_or(Fault::Malformed)?;
        let (digits, tail) = rest.split_at(digits_end);
        let unit_char = tail.chars().next().ok_or(Fault::Malformed)?;
        let offset = UNITS[next_unit..]
            .iter()
            .position(|&(name, _)| name == unit_char)
            .ok_or(Fault::Malformed)?;
        if digits.is_empty() {
            return Err(Fault::Malformed);
        }
        let (_, unit) = UNITS[next_unit + offset];
        parts.push((digits, unit));
        next_unit += offset + 1;
        rest = &tail[unit_char.len_utf8()..];
    }
    if parts.is_empty() {
        return Err(Fault::Malformed);
    }

    parts.iter().try_fold(0_u64, |total, &(digits, unit)| {
        add_scaled(total, digits, unit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colon_forms_keep_todays_meaning() {
        assert_eq!(parse("00:20:00").unwrap(), Duration::from_mins(20));
        assert_eq!(parse("1:30").unwrap(), Duration::from_mins(90)); // hh:mm
        assert_eq!(parse("5").unwrap(), Duration::from_hours(5)); // bare number = hours, as today
    }

    #[test]
    fn colon_forms_with_seconds_and_unpadded_parts() {
        assert_eq!(parse("1:02:03").unwrap(), Duration::from_secs(3723));
        assert_eq!(parse("0:0:7").unwrap(), Duration::from_secs(7));
        assert_eq!(parse("0:90").unwrap(), Duration::from_mins(90)); // minutes are not range-checked, as today
    }

    #[test]
    fn short_forms() {
        assert_eq!(parse("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse("20m").unwrap(), Duration::from_mins(20));
        assert_eq!(parse("2h").unwrap(), Duration::from_hours(2));
        assert_eq!(parse("1h30m").unwrap(), Duration::from_mins(90));
        assert_eq!(parse("1h0m5s").unwrap(), Duration::from_secs(3605));
        assert_eq!(parse("0s").unwrap(), Duration::ZERO);
    }

    #[test]
    fn short_forms_combine_each_unit_once() {
        assert_eq!(parse("2h5s").unwrap(), Duration::from_secs(7205));
        assert_eq!(parse("3m4s").unwrap(), Duration::from_secs(184));
        assert_eq!(parse("1h2m3s").unwrap(), Duration::from_secs(3723));
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "", "abc", "1.5h", "30m1h", "1h1h", "-5m", "5x", "1h 30m", "::",
        ] {
            assert!(parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn rejects_more_malformed_forms() {
        for bad in [
            "1:2:3:4", "1::2", ":30", "30:", "1:-2", "+5", "-5", "h", "m30", "1h30", "30 ", " 30",
            "1H", "1:30m", "１h", "5é",
        ] {
            assert!(
                matches!(parse(bad), Err(DurationError::Invalid(_))),
                "{bad} should be Invalid"
            );
        }
    }

    #[test]
    fn invalid_error_names_the_input_and_accepted_forms() {
        let message = parse("1.5h").unwrap_err().to_string();
        assert_eq!(
            message,
            "invalid duration \"1.5h\": use hh:mm:ss or forms like 90s, 20m, 1h30m"
        );
    }

    #[test]
    fn rejects_overflow_without_panicking() {
        assert!(matches!(
            parse("99999999999999999999h"),
            Err(DurationError::Overflow(_))
        ));
        assert!(matches!(
            parse("9999999999999999:00:00"),
            Err(DurationError::Overflow(_))
        ));
        assert!(matches!(
            parse("99999999999h"),
            Err(DurationError::Overflow(_))
        )); // fits u64, panics chrono
        assert!(matches!(parse("2401h"), Err(DurationError::Overflow(_)))); // just over 100 days
        assert!(parse("2400h").is_ok());
    }

    #[test]
    fn max_duration_boundary_is_inclusive_in_every_form() {
        assert_eq!(parse("2400h").unwrap(), MAX_DURATION);
        assert_eq!(parse("144000m").unwrap(), MAX_DURATION);
        assert_eq!(parse("8640000s").unwrap(), MAX_DURATION);
        assert_eq!(parse("2400:00:00").unwrap(), MAX_DURATION);
        assert_eq!(parse("2400").unwrap(), MAX_DURATION);
        for over in ["8640001s", "144001m", "2400h1s", "2400:00:01", "2401"] {
            assert!(
                matches!(parse(over), Err(DurationError::Overflow(_))),
                "{over} should overflow"
            );
        }
    }

    #[test]
    fn overflow_in_the_multiplication_is_caught() {
        // 1e16 hours fits u64 but its seconds do not.
        assert!(matches!(
            parse("9999999999999999h"),
            Err(DurationError::Overflow(_))
        ));
        assert!(matches!(
            parse("18446744073709551615m"),
            Err(DurationError::Overflow(_))
        ));
    }

    #[test]
    fn overflow_that_would_wrap_to_a_small_value_is_caught() {
        // 2^60 hours * 3600 = 225 * 2^64, which wraps to exactly 0 seconds.
        // 2^62 minutes * 60 = 15 * 2^64, likewise. 7200 + (2^64 - 7200) wraps to 0 in the sum.
        for wrapping in [
            "1152921504606846976h",
            "1152921504606846976:00:00",
            "4611686018427387904m",
            "2h18446744073709544416s",
            "2:00:18446744073709544416",
        ] {
            assert!(
                matches!(parse(wrapping), Err(DurationError::Overflow(_))),
                "{wrapping} should overflow"
            );
        }
    }

    #[test]
    fn overflow_error_names_the_input_and_the_limit() {
        let message = parse("2401h").unwrap_err().to_string();
        assert_eq!(
            message,
            "duration \"2401h\" is too long: the maximum is 100 days"
        );
    }

    #[test]
    fn max_duration_is_one_hundred_days() {
        assert_eq!(MAX_DURATION, Duration::from_hours(100 * 24));
    }

    #[test]
    fn parse_arg_passes_good_values_through() {
        assert_eq!(parse_arg("1h30m").unwrap(), Duration::from_mins(90));
        assert_eq!(parse_arg("00:00:10").unwrap(), Duration::from_secs(10));
    }

    #[test]
    fn parse_arg_returns_the_error_text() {
        assert_eq!(
            parse_arg("abc").unwrap_err(),
            "invalid duration \"abc\": use hh:mm:ss or forms like 90s, 20m, 1h30m"
        );
        assert!(parse_arg("2401h").unwrap_err().contains("too long"));
    }
}
