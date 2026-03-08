use words_to_data::utils::date_str_to_date;

#[test]
fn test_valid_date_parsing() {
    // Standard format
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
fn test_different_months() {
    // January
    let result = date_str_to_date("2025-01-15");
    assert!(result.is_ok());
    let date = result.unwrap();
    assert_eq!(date.month() as u8, 1);
    assert_eq!(date.day(), 15);

    // December
    let result = date_str_to_date("2025-12-31");
    assert!(result.is_ok());
    let date = result.unwrap();
    assert_eq!(date.month() as u8, 12);
    assert_eq!(date.day(), 31);
}

#[test]
fn test_leap_year_feb_29() {
    // 2024 is a leap year
    let result = date_str_to_date("2024-02-29");
    assert!(result.is_ok(), "2024-02-29 should be valid (leap year)");

    let date = result.unwrap();
    assert_eq!(date.year(), 2024);
    assert_eq!(date.month() as u8, 2);
    assert_eq!(date.day(), 29);
}

#[test]
fn test_invalid_date_missing_components() {
    // Only year and month
    let result = date_str_to_date("2025-07");
    assert!(result.is_err(), "Should fail with missing day component");

    // Only year
    let result = date_str_to_date("2025");
    assert!(result.is_err(), "Should fail with only year");

    // Empty string
    let result = date_str_to_date("");
    assert!(result.is_err(), "Should fail with empty string");
}

#[test]
fn test_invalid_date_too_many_components() {
    let result = date_str_to_date("2025-07-18-00");
    assert!(result.is_err(), "Should fail with too many components");
}

#[test]
#[should_panic(expected = "month num shoudl be between 1-12")]
fn test_invalid_month_number() {
    // Month 13 doesn't exist
    let _result = date_str_to_date("2025-13-01");
}

#[test]
#[should_panic(expected = "month num shoudl be between 1-12")]
fn test_month_out_of_range() {
    // Month 0 is invalid
    let _result = date_str_to_date("2025-00-15");
}

#[test]
fn test_invalid_day_for_month() {
    // February 30 doesn't exist
    let result = date_str_to_date("2025-02-30");
    assert!(result.is_err(), "February 30 should be invalid");

    // Non-leap year Feb 29
    let result = date_str_to_date("2025-02-29");
    assert!(
        result.is_err(),
        "2025 is not a leap year, Feb 29 should be invalid"
    );
}

#[test]
#[should_panic]
fn test_non_numeric_values() {
    let _result = date_str_to_date("abcd-ef-gh");
}

#[test]
fn test_month_boundaries() {
    // Test January (1)
    let result = date_str_to_date("2025-01-01");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().month() as u8, 1);

    // Test December (12)
    let result = date_str_to_date("2025-12-01");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().month() as u8, 12);
}

#[test]
fn test_zero_padded_values() {
    // Leading zeros should work
    let result = date_str_to_date("2025-01-05");
    assert!(result.is_ok());

    let date = result.unwrap();
    assert_eq!(date.month() as u8, 1);
    assert_eq!(date.day(), 5);
}
