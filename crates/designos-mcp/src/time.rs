//! Small UTC timestamp helper for MCP-created comments.

/// Returns the current UTC time in RFC 3339 with millisecond precision.
pub(crate) fn now_rfc3339() -> String {
    let millis = designos_doc::now_ms();
    let seconds = (millis / 1_000) as i64;
    let days = seconds.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let day_seconds = seconds.rem_euclid(86_400);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", day_seconds / 3_600, day_seconds % 3_600 / 60, day_seconds % 60, millis % 1_000)
}

// Howard Hinnant's proleptic-Gregorian civil date conversion.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 { month_part + 3 } else { month_part - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, now_rfc3339};

    #[test]
    fn epoch_and_known_dates_are_stable() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn uses_the_injected_document_clock() {
        designos_doc::set_thread_mock_now_ms(Some(1_790_000_000_321));
        assert_eq!(now_rfc3339(), "2026-09-21T14:13:20.321Z");
        designos_doc::set_thread_mock_now_ms(None);
    }
}
