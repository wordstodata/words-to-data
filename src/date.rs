//! Date parsing utilities for USLM documents
//!
//! This module provides functions for parsing date strings into `time::Date` values.

use std::str::FromStr;
use time::Date;

use crate::uslm::parser::ParseError;

type Result<T> = std::result::Result<T, ParseError>;

/// Convert a month number to the corresponding Month enum
///
/// # Arguments
///
/// * `n` - Month number (1-12, where 1 is January and 12 is December)
///
/// # Returns
///
/// The corresponding `time::Month` value, or `ParseError::InvalidDate` if
/// the month number is out of range.
fn month_from_number(n: i32) -> Result<time::Month> {
    // TODO: There's probably something built into time that does this, but I couldn't find it and it was easy to write
    match n {
        1 => Ok(time::Month::January),
        2 => Ok(time::Month::February),
        3 => Ok(time::Month::March),
        4 => Ok(time::Month::April),
        5 => Ok(time::Month::May),
        6 => Ok(time::Month::June),
        7 => Ok(time::Month::July),
        8 => Ok(time::Month::August),
        9 => Ok(time::Month::September),
        10 => Ok(time::Month::October),
        11 => Ok(time::Month::November),
        12 => Ok(time::Month::December),
        _ => Err(ParseError::InvalidDate),
    }
}

/// Parse a date string in YYYY-MM-DD format to a Date
///
/// # Arguments
///
/// * `date_str` - Date string in the format "YYYY-MM-DD" (e.g., "2025-07-18")
///
/// # Returns
///
/// A `time::Date` if parsing succeeds, or `ParseError::InvalidDate` if:
/// - The format is invalid (not three dash-separated components)
/// - The year, month, or day cannot be parsed as integers
/// - The month is out of range (must be 1-12)
/// - The day is invalid for the given month/year
///
/// # Examples
///
/// ```
/// use words_to_data::date::date_str_to_date;
///
/// let date = date_str_to_date("2025-07-18").unwrap();
/// assert_eq!(date.year(), 2025);
/// assert_eq!(date.month() as u8, 7);
/// assert_eq!(date.day(), 18);
/// ```
pub fn date_str_to_date(date_str: &str) -> Result<Date> {
    let date_split: Vec<&str> = date_str.split("-").collect();
    if date_split.len() != 3 {
        return Err(ParseError::InvalidDate);
    }
    let year_num = i32::from_str(date_split[0]).map_err(|_| ParseError::InvalidDate)?;
    let month_num = i32::from_str(date_split[1]).map_err(|_| ParseError::InvalidDate)?;
    let day_num = u8::from_str(date_split[2]).map_err(|_| ParseError::InvalidDate)?;
    let month_enum = month_from_number(month_num)?;
    Date::from_calendar_date(year_num, month_enum, day_num).map_err(|_| ParseError::InvalidDate)
}
