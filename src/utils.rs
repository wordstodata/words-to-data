use crate::uslm::parser::ParseError;
use crate::uslm::{USLMElement, parser::parse};
use rayon::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use time::Date;

type Result<T> = std::result::Result<T, ParseError>;

/// Load an XML file from disk into a string
///
/// # Arguments
///
/// * `path` - Path to the XML file to load
///
/// # Returns
///
/// The file contents as a `String`, or an I/O error if the file cannot be read.
pub fn load_xml_file(path: &str) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

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
/// - The month is out of range (must be 1-12)
/// - The day is invalid for the given month/year
///
/// # Examples
///
/// ```
/// use words_to_data::utils::date_str_to_date;
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
    let year_num = i32::from_str(date_split[0]).expect("year should be valid i32");
    let month_num = i32::from_str(date_split[1]).expect("month should be valid i32");
    let day_num = u8::from_str(date_split[2]).expect("day should be valid u8");
    let month_enum = month_from_number(month_num).expect("month num shoudl be between 1-12");
    let date = Date::from_calendar_date(year_num, month_enum, day_num);
    match date {
        Ok(d) => Ok(d),
        Err(_e) => Err(ParseError::InvalidDate),
    }
}

/// Parse a USLM XML file into a USLMElement tree
///
/// # Arguments
///
/// * `xml_path` - Path to the USLM XML file
/// * `date` - Publication date string in "YYYY-MM-DD" format
///
/// # Returns
///
/// The parsed document as a `USLMElement` tree, or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```
/// use words_to_data::utils::parse_uslm_xml;
///
/// let element = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
/// assert_eq!(element.data.uslm_id.unwrap(), "/us/usc/t7");
/// ```
pub fn parse_uslm_xml(xml_path: &str, date: &str) -> Result<USLMElement> {
    parse(xml_path, date)
}

/// Write any serializable data to a JSON file with pretty formatting
///
/// # Arguments
///
/// * `data` - The data to serialize (must implement `Serialize`)
/// * `json_path` - Path where the JSON file will be written
///
/// # Returns
///
/// `Ok(())` if successful, or a `ParseError` if serialization or file writing fails.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::utils::{parse_uslm_xml, write_json_file};
///
/// let element = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
/// write_json_file(&element, "output.json").unwrap();
/// ```
pub fn write_json_file<T: Serialize>(data: &T, json_path: &str) -> Result<()> {
    let json_string = serde_json::to_string_pretty(data)?;
    let mut output = fs::File::create(json_path)?;
    write!(output, "{}", json_string)?;
    Ok(())
}

/// Read and deserialize a JSON file into any type
///
/// # Arguments
///
/// * `json_path` - Path to the JSON file to read
///
/// # Returns
///
/// The deserialized data of type `T`, or a `ParseError` if reading or
/// deserialization fails.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::utils::read_json_file;
/// use words_to_data::uslm::USLMElement;
///
/// let element: USLMElement = read_json_file("output.json").unwrap();
/// ```
pub fn read_json_file<T: DeserializeOwned>(json_path: &str) -> Result<T> {
    let json_string = fs::read_to_string(json_path)?;
    let data = serde_json::from_str(&json_string)?;
    Ok(data)
}

/// Parse a USLM XML file and write it directly to JSON
///
/// Convenience function that combines parsing and JSON serialization in one step.
///
/// # Arguments
///
/// * `xml_path` - Path to the USLM XML file
/// * `date` - Publication date string in "YYYY-MM-DD" format
/// * `json_path` - Path where the JSON output will be written
///
/// # Returns
///
/// `Ok(())` if successful, or a `ParseError` if parsing or writing fails.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::utils::parse_uslm_to_json;
///
/// parse_uslm_to_json(
///     "tests/test_data/usc/2025-07-18/usc07.xml",
///     "2025-07-18",
///     "usc07.json"
/// ).unwrap();
/// ```
pub fn parse_uslm_to_json(xml_path: &str, date: &str, json_path: &str) -> Result<()> {
    let element = parse_uslm_xml(xml_path, date)?;
    write_json_file(&element, json_path)?;
    Ok(())
}

/// Parse all USLM XML files in a directory in parallel
///
/// Reads all .xml files from the input directory and parses them concurrently
/// using Rayon for parallel processing. This is significantly faster than
/// parsing files sequentially when processing multiple documents.
///
/// # Arguments
///
/// * `input_dir` - Path to directory containing XML files
/// * `date` - Publication date string in "YYYY-MM-DD" format to associate with all files
///
/// # Returns
///
/// A vector of successfully parsed `USLMElement` trees, or a `ParseError` if
/// any files fail to parse.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::utils::parse_uslm_directory;
///
/// let elements = parse_uslm_directory("usc_data/2025-07-18", "2025-07-18").unwrap();
/// println!("Parsed {} documents", elements.len());
/// ```
pub fn parse_uslm_directory(input_dir: &str, date: &str) -> Result<Vec<USLMElement>> {
    let dir_path = Path::new(input_dir);

    // Collect all XML file paths
    let xml_files: Vec<PathBuf> = fs::read_dir(dir_path)
        .map_err(ParseError::Io)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "xml" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Parse files in parallel
    let results: Vec<Result<USLMElement>> = xml_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_str().ok_or_else(|| {
                ParseError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid path encoding",
                ))
            })?;
            parse_uslm_xml(path_str, date)
        })
        .collect();

    // Separate successes from failures
    let mut elements = Vec::new();
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(element) => elements.push(element),
            Err(e) => errors.push(e),
        }
    }

    // If there were any errors, report them
    if !errors.is_empty() {
        let error_msg = format!("Failed to parse {} files", errors.len());
        return Err(ParseError::UnableToParseElement(error_msg));
    }

    Ok(elements)
}

/// Parse all USLM XML files in a directory and write them as JSON files in parallel
///
/// Reads all .xml files from the input directory, parses them concurrently
/// using Rayon, and writes each to a corresponding .json file in the output
/// directory. Output files have the same name as the input files but with
/// a .json extension.
///
/// The output directory will be created if it doesn't exist.
///
/// # Arguments
///
/// * `input_dir` - Path to directory containing XML files
/// * `date` - Publication date string in "YYYY-MM-DD" format to associate with all files
/// * `output_dir` - Path to directory where JSON files will be written
///
/// # Returns
///
/// `Ok(())` if all files were successfully parsed and written, or a `ParseError`
/// if any operations fail.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::utils::parse_uslm_directory_to_json;
///
/// parse_uslm_directory_to_json(
///     "usc_data/2025-07-18",
///     "2025-07-18",
///     "output/json"
/// ).unwrap();
/// ```
pub fn parse_uslm_directory_to_json(input_dir: &str, date: &str, output_dir: &str) -> Result<()> {
    let dir_path = Path::new(input_dir);
    let out_path = Path::new(output_dir);

    // Create output directory if it doesn't exist
    fs::create_dir_all(out_path)?;

    // Collect all XML file paths
    let xml_files: Vec<PathBuf> = fs::read_dir(dir_path)
        .map_err(ParseError::Io)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "xml" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Parse and write files in parallel
    let results: Vec<Result<()>> = xml_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_str().ok_or_else(|| {
                ParseError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid path encoding",
                ))
            })?;

            let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                ParseError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid filename",
                ))
            })?;

            let output_path = out_path.join(format!("{}.json", file_stem));
            let output_str = output_path.to_str().ok_or_else(|| {
                ParseError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid output path encoding",
                ))
            })?;

            parse_uslm_to_json(path_str, date, output_str)
        })
        .collect();

    // Check for any errors
    let errors: Vec<ParseError> = results.into_iter().filter_map(|r| r.err()).collect();

    if !errors.is_empty() {
        let error_msg = format!("Failed to process {} files", errors.len());
        return Err(ParseError::UnableToParseElement(error_msg));
    }

    Ok(())
}
