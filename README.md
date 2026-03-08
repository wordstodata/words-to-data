# Words To Data - Convert Legal Documents Into Diffable Data Structures 

## Overview

`words_to_data` parses US Code titles and Public Laws (bills) from USLM XML format, providing structured access to legislative text and the ability to track changes between document versions.

Available for both **Rust** and **Python** with high-performance Rust core and ergonomic Python bindings via PyO3.

## Features

- **Parse USC and Public Law documents** - Extract hierarchical structure from USLM XML files
- **Bill amendment extraction** - Identify USC references and amending actions from bills
- **Hierarchical diffing** - Compute word-level differences between document versions
- **Parallel processing** - Parse multiple documents concurrently using Rayon (Rust only)
- **Dual path system** - Track both structural paths and official USLM identifiers
- **Rich text content** - Capture heading, chapeau, proviso, content, and continuation fields
- **Python bindings** - Full API access from Python with PyO3

## Installation

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
words-to-data = "0.1.2"
```

### Python

```bash
pip install words-to-data
```

**Note:** Pre-built wheels are available for Linux x86_64. Other platforms will build from source (requires Rust toolchain).

## Getting Data
- Title data: https://uscode.house.gov/download/download.shtml
- Bill data: https://congress.gov

## Quick Start

### Parse a US Code Document

**Rust:**
```rust
use words_to_data::uslm::parser::parse;

fn main() -> Result<(), Box< dyn std::error::Error>> {
    // Load a USCode Title
    let title_26 = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")?;

    // Navigate to §174(a)
    let s174a = title_26.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a").expect("§174 (a) not found");

    // Print the chapeau value
    println!(
        "§ 174(a) chapeau: {}",
        s174a.data.chapeau.clone().unwrap_or("<Empty>".to_string())
    );

    // Serialize
    words_to_data::utils::write_json_file(&title_26, "title_26.json")?;
    Ok(())
}
```

**Python:**
```python
from words_to_data import parse_uslm_xml

title_26 = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
s174a = title_26.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
print(f"§ 174(a) chapeau: {s174a.data['chapeau']}")
```

### Compute a Diff Between Versions

**Rust:**
```rust
use words_to_data::{diff::TreeDiff, uslm::parser::parse};

fn main() -> Result<(), Box< dyn std::error::Error>> {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")?;
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")?;

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);

    let s174a_diff = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a").expect("Section 174A has no changes, nor does its children!");

    for change in s174a_diff.changes.iter() {
        println!("{:#?} Changed:", change.field_name);
        println!("  Old: {}", change.old_value);
        println!("  New: {}", change.new_value);
        println!("  Number of word-level changes: {}", change.changes.len());
    }
    words_to_data::utils::write_json_file(&diff, "diff.json")?;
    Ok(())
}
```

**Python:**
```python
from words_to_data import parse_uslm_xml, compute_diff

doc_old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
doc_new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")

diff = compute_diff(doc_old, doc_new)

s174a_diff = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")

for change in s174a_diff.changes:
    print(f"{change.field_name} Changed:")
    print(f"  Old: {change.old_value}")
    print(f"  New: {change.new_value}")
    print(f"  Number of word-level changes: {len(change.changes)}")
```

### Extract Amendments from a Bill

**Rust:**
```rust
use words_to_data::uslm::bill_parser::parse_bill_amendments;

fn main() -> Result<(), Box< dyn std::error::Error>> {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")?;

    println!(
        "Bill {}: {} amendments found",
        data.bill_id,
        data.amendments.len()
    );

    for amendment in &data.amendments {
        println!("\nAmendment at: {}", amendment.source_path);
        println!("  USC sections modified: {}", amendment.target_paths.len());
        println!("  Actions: {:?}", amendment.action_types);
    }

    Ok(())
}
```

**Python:**
```python
from words_to_data import parse_bill_amendments

data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")

print(f"Bill {data.bill_id}: {len(data.amendments)} amendments found")

for amendment in data.amendments:
    print(f"\nAmendment at: {amendment.source_path}")
    print(f"  USC sections modified: {len(amendment.target_paths)}")
    print(f"  Actions: {amendment.action_types}")

    for ref in amendment.target_paths:
        print(f"    - {ref.display_text} ({ref.path})")
```

## Core Concepts

### USLM Elements

Documents are represented as trees of `USLMElement` structures. Each element contains:

- **ElementData**: Metadata, text content, and identification
- **Children**: Nested child elements forming the document hierarchy

The library uses two types of paths:

1. **Structural Path**: Full hierarchy including all elements
   Example: `uscodedocument_26/title_26/subtitle_A/chapter_1/section_174`

2. **USLM ID**: Official USLM identifier (excludes structural-only elements)
   Example: `/us/usc/t26/s174/a/1`

### Text Content Fields

Each element can contain up to five distinct text fields:

- **Heading**: Section or subsection title
- **Chapeau**: Opening text before enumerated items
- **Proviso**: Conditional or qualifying clauses
- **Content**: Main body text
- **Continuation**: Text appearing after child elements

### Diffs

The `TreeDiff` structure mirrors the element hierarchy and tracks:

- **Field changes**: Word-level differences in text content fields
- **Added elements**: New child elements in the newer version
- **Removed elements**: Elements that existed in the older version
- **Child diffs**: Recursive diffs for matching child elements

Diffs are computed using word-level granularity via the `similar` crate.

## API Documentation

### Rust

Generate and view the full API documentation:

```bash
cargo doc --open
```

### Python

Python type stubs are included for IDE autocomplete. Access help in Python:

```python
from words_to_data import parse_uslm_xml, compute_diff, USLMElement, TreeDiff

help(parse_uslm_xml)
help(USLMElement)
```

## Contributing

Contributions welcome! This project uses:
- **Rust** for the core library
- **PyO3** for Python bindings
- **GitHub Actions** for CI/CD

## License

MIT
