# Words To Data

A Rust library for parsing United States legislative documents in USLM (United States Legislative Markup) XML format and computing diffs between document versions.

## Overview

`words_to_data` parses US Code titles and Public Laws (bills) from USLM XML format, providing structured access to legislative text and the ability to track changes between document versions. All data structures are JSON-serializable for easy integration with other systems.

## Features

- **Parse USC and Public Law documents** - Extract hierarchical structure from USLM XML files
- **Bill amendment extraction** - Identify USC references and amending actions from bills
- **Hierarchical diffing** - Compute word-level differences between document versions
- **Parallel processing** - Parse multiple documents concurrently using Rayon
- **Dual path system** - Track both structural paths and official USLM identifiers
- **Rich text content** - Capture heading, chapeau, proviso, content, and continuation fields
- **JSON serialization** - All structures implement Serde traits

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
words_to_data = "0.1.0"
```

Or clone and build locally:

```bash
git clone https://github.com/yourusername/words_to_data
cd words_to_data
cargo build --release
```

## Getting Data
Title data can be retrieved from https://uscode.house.gov/download/download.shtml  

Bill data is available on https://congress.gov
## Quick Start

### Parse a US Code Document

```rust
use words_to_data::uslm::parser::parse;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")?;

    println!("Parsed: {}", document.data.verbose_name);
    println!("USLM ID: {:?}", document.data.uslm_id);
    println!("Children: {}", document.children.len());

    Ok(())
}
```

### Compute a Diff Between Versions

```rust
use std::fs;

use words_to_data::{diff::TreeDiff, uslm::parser::parse};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")?;
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30")?;

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);
    words_to_data::utils::write_json_file(&diff, "diff.json")?;
    Ok(())
}
```

### Extract Amendments from a Bill

```rust
use words_to_data::uslm::bill_parser::parse_bill_amendments;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")?;

    println!("Bill {}: {} amendments found", data.bill_id, data.amendments.len());

    for amendment in &data.amendments {
        println!("\nAmendment at: {}", amendment.source_path);
        println!("  USC sections modified: {}", amendment.target_paths.len());
        println!("  Actions: {:?}", amendment.action_types);
    }

    Ok(())
}
```

### Parse Multiple Documents in Parallel

```rust
use words_to_data::utils::parse_uslm_directory;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let documents = parse_uslm_directory("tests/test_data/usc/2025-07-18", "2025-07-18")?;

    println!("Parsed {} documents in parallel", documents.len());

    for doc in documents.iter().take(5) {
        println!("  - {} ({})", doc.data.verbose_name, doc.data.path);
    }

    Ok(())
}
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

Generate and view the full API documentation:

```bash
cargo doc --open
```

The documentation includes detailed information about all public types, functions, and examples.
