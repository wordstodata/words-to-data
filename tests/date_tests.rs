use words_to_data::date::date_str_to_date;

#[test]
fn test_date_str_to_date_valid() {
    let result = date_str_to_date("2025-07-18");
    assert!(
        result.is_ok(),
        "Failed to parse valid date: {:?}",
        result.err()
    );

    let date = result.unwrap();
    assert_eq!(date.year(), 2025);
    assert_eq!(date.month() as u8, 7);
    assert_eq!(date.day(), 18);
}

#[test]
fn test_date_str_to_date_leap_year() {
    // 2024 is a leap year
    let result = date_str_to_date("2024-02-29");
    assert!(result.is_ok(), "Should parse Feb 29 in leap year");
}

#[test]
fn test_date_str_to_date_invalid_format() {
    let result = date_str_to_date("2025-07");
    assert!(result.is_err(), "Should fail with missing day");
}
