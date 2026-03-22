use crate::uslm::parser::ParseError;
use crate::uslm::{USLMElement, parser::parse};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

// Re-export from io module for backward compatibility
pub use crate::io::{load_xml_file, read_json_file, write_json_file};

// Re-export from date module for backward compatibility
pub use crate::date::date_str_to_date;

type Result<T> = std::result::Result<T, ParseError>;

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
