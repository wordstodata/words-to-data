//! File I/O operations for USLM documents
//!
//! This module provides functions for reading and writing files,
//! including XML loading and JSON serialization.

use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File};
use std::io::{Read, Write};

use crate::uslm::parser::ParseError;

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
///
/// # Examples
///
/// ```
/// use words_to_data::io::load_xml_file;
///
/// let content = load_xml_file("tests/test_data/usc/2025-07-18/usc07.xml").unwrap();
/// assert!(content.contains("uscDoc"));
/// ```
pub fn load_xml_file(path: &str) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
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
/// use words_to_data::io::write_json_file;
///
/// let data = vec!["hello", "world"];
/// write_json_file(&data, "output.json").unwrap();
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
/// use words_to_data::io::read_json_file;
///
/// let data: Vec<String> = read_json_file("data.json").unwrap();
/// ```
pub fn read_json_file<T: DeserializeOwned>(json_path: &str) -> Result<T> {
    let json_string = fs::read_to_string(json_path)?;
    let data = serde_json::from_str(&json_string)?;
    Ok(data)
}
