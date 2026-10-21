//! Times as feeds spell them, in Unix milliseconds.

use crab_lake::time;

/// 2026-10-11T06:55:00Z.
const MORNING: u64 = 1_791_701_700_000;
/// 2026-10-09T23:27:00Z.
const NIGHT: u64 = 1_791_588_420_000;

#[test]
fn a_date_is_its_midnight_utc() {
    assert_eq!(time::date(1970, 1, 1), Some(0));
    assert_eq!(time::date(2026, 10, 11), Some(1_791_676_800_000));
    assert_eq!(time::date(2024, 2, 29), Some(1_709_164_800_000));
}

#[test]
fn a_date_before_1970_or_out_of_range_is_none() {
    assert_eq!(time::date(1969, 12, 31), None);
    assert_eq!(time::date(2026, 13, 1), None);
    assert_eq!(time::date(2026, 0, 1), None);
    assert_eq!(time::date(2026, 1, 32), None);
}

#[test]
fn rfc3339_reads_zones_fractions_and_dates() {
    assert_eq!(time::rfc3339("2026-10-11T06:55:00+00:00"), Some(MORNING));
    assert_eq!(time::rfc3339("2026-10-11T06:55:00Z"), Some(MORNING));
    assert_eq!(time::rfc3339("2026-10-11T06:55:00.123Z"), Some(MORNING));
    assert_eq!(time::rfc3339("2026-10-11T06:55:00"), Some(MORNING));
    assert_eq!(time::rfc3339("2026-10-11T08:55:00+02:00"), Some(MORNING));
    assert_eq!(time::rfc3339("2026-10-11T01:55:00-0500"), Some(MORNING));
    assert_eq!(time::rfc3339(" 2026-10-11 "), Some(1_791_676_800_000));
    assert_eq!(time::rfc3339("2026-10-11T06:55"), None);
    assert_eq!(time::rfc3339("yesterday"), None);
}

#[test]
fn rfc822_reads_weekdays_years_and_zones() {
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 23:27:00 +0000"), Some(NIGHT));
    assert_eq!(time::rfc822("09 Oct 2026 23:27:00 GMT"), Some(NIGHT));
    assert_eq!(time::rfc822("Fri, 9 Oct 26 23:27 UT"), Some(NIGHT));
    assert_eq!(time::rfc822("Fri, 09 October 2026 23:27:00 Z"), Some(NIGHT));
    assert_eq!(
        time::rfc822("Sat, 10 Oct 2026 01:27:00 +02:00"),
        Some(NIGHT)
    );
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 19:27:00 EDT"), Some(NIGHT));
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 15:27:00 PST"), Some(NIGHT));
    assert_eq!(
        time::rfc822("Fri, 31 Dec 99 23:59:00 GMT"),
        Some(946_684_740_000)
    );
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 23:27:00"), None);
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 23:27:00 CET"), None);
}

#[test]
fn a_month_is_named_by_its_first_three_letters() {
    assert_eq!(time::month("Jan"), Some(1));
    assert_eq!(time::month("december"), Some(12));
    assert_eq!(time::month("SEP"), Some(9));
    assert_eq!(time::month("Ju"), None);
    assert_eq!(time::month("Foo"), None);
}

#[test]
fn parse_reads_either_spelling() {
    assert_eq!(time::parse("2026-10-11T06:55:00Z"), Some(MORNING));
    assert_eq!(time::parse("Fri, 09 Oct 2026 23:27:00 +0000"), Some(NIGHT));
    assert_eq!(time::parse("yesterday"), None);
}

#[test]
fn a_day_past_its_month_or_a_year_past_9999_is_none() {
    assert_eq!(time::date(2026, 2, 29), None);
    assert_eq!(time::date(2026, 2, 31), None);
    assert_eq!(time::date(2026, 4, 31), None);
    assert_eq!(time::date(2000, 2, 29), Some(951_782_400_000));
    assert_eq!(time::date(2100, 2, 29), None);
    assert_eq!(time::date(10_000, 1, 1), None);
    assert_eq!(time::date(i64::MAX, 1, 1), None);
    assert_eq!(time::rfc3339("2026-02-30T00:00:00Z"), None);
    assert_eq!(time::rfc822("31 Feb 2026 00:00 GMT"), None);
}

#[test]
fn a_clock_or_offset_out_of_range_is_none() {
    assert_eq!(time::rfc3339("2026-10-11T24:00:00Z"), None);
    assert_eq!(time::rfc3339("2026-10-11T06:60:00Z"), None);
    assert_eq!(time::rfc3339("2026-10-11T06:55:61Z"), None);
    assert_eq!(
        time::rfc3339("2026-10-11T06:55:60Z"),
        Some(MORNING + 60_000)
    );
    assert_eq!(time::rfc3339("2026-10-11T06:55:00+24:00"), None);
    assert_eq!(time::rfc822("Fri, 09 Oct 2026 23:27:00 +0099"), None);
    assert_eq!(
        time::rfc822("Fri, 09 Oct 2026 23:27:00 +00999999999999999999"),
        None
    );
    assert_eq!(
        time::rfc822("Fri, 09 Oct 2026 9999999999999999:27:00 GMT"),
        None
    );
    assert_eq!(
        time::rfc822("Fri, 09 Oct 922337203685477580 23:27:00 GMT"),
        None
    );
}
