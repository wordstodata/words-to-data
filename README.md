# Words To Data - Convert Legal Documents Into Diffable Data Structures

[![CI](https://github.com/Scronkfinkle/words-to-data/actions/workflows/ci.yml/badge.svg)](https://github.com/Scronkfinkle/words-to-data/actions/workflows/ci.yml)

## Overview

`words_to_data` parses US Code titles and Public Laws (bills) from USLM XML format, providing structured access to legislative text, the ability to track what changed between two readings of one document, and tools for annotating how bills amend existing law.

Written in Rust.

## Features

- **Dataset-centric workflow** - Manage legal documents, bills, and annotations in a single structure
- **Work-scoped storage** - A document is keyed by what it is and when it was published, so documents that share no release cycle can sit in one dataset
- **Parse USC and Public Law documents** - Extract hierarchical structure from USLM XML files
- **Rich text content** - Capture heading, chapeau, proviso, content, and continuation fields
- **Bill amendment extraction** - Identify USC references and amending actions from bills
- **Hierarchical diffing** - Compute word-level differences between two expressions of one work
- **Congress data integration** - Fetch bill metadata and text from Congress.gov API

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
words-to-data = "0.3.0"
```

## Getting Data
- Title data: https://uscode.house.gov/download/download.shtml
- Bill data: https://congress.gov

## Quick Start

### Dataset Workflow

The `Dataset` is the primary abstraction for working with legal documents over time. It holds expressions, bills, and annotations together.

A **work** is a document as a concept, with no date: `uscode/title_26`. An **expression** is that work as it read on one date, written `uscode/title_26@2025-07-18`. A dataset is keyed by expression, so documents that share no release cycle — court opinions, state codes — sit side by side without pretending to.

```rust
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::uslm::bill_parser::parse_bill_amendments;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Add documents. Each file becomes one expression per work it holds.
    dataset.add_uslm_xml("path/to/old.xml", "2025-07-18", Some("Before".into()))?;
    dataset.add_uslm_xml("path/to/new.xml", "2025-07-30", Some("After".into()))?;

    // Add bill
    let bill = parse_bill_amendments("119-21", "path/to/bill.xml")?;
    dataset.add_bill(bill)?;

    // Diff two expressions of one work
    let title_26 = WorkId::new("uscode/title_26");
    let before = ExpressionId::new(title_26.clone(), "2025-07-18");
    let after = ExpressionId::new(title_26, "2025-07-30");
    let diff = dataset.compute_diff(&before, &after)?;

    // Navigate to specific section
    if let Some(s174a) = diff.find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a") {
        for change in &s174a.changes {
            println!("{:?}: {} → {}", change.field_name, change.old_value, change.new_value);
        }
    }

    // Save dataset
    dataset.save("my_dataset.json", Format::Compact)?;
    Ok(())
}
```

An `ExpressionId` parses from the same form it prints, so it can come straight off a command line or out of a citation:

```rust
use words_to_data::dataset::ExpressionId;

let id: ExpressionId = "uscode/title_26@2025-07-18".parse()?;
assert_eq!(id.to_string(), "uscode/title_26@2025-07-18");
```

### Download from Congress API

Bills can be automatically fetched with additional metadata from the congress.gov API

```rust
use words_to_data::congress::CongressClient;
use words_to_data::dataset::{Dataset, DatasetMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with API key (get from https://api.congress.gov/sign-up/)
    let client = CongressClient::new(std::env::var("CONGRESS_API_KEY")?);

    // Download bill data (XML + sponsors + members)
    let download = client.download_bill("119-hr-1")?;

    // Create dataset and load bill
    let metadata = DatasetMetadata {
        name: "HR 1 Analysis".to_string(),
        description: "Tracking HR 1 amendments".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Load bill into dataset (parses XML, stores sponsors/members)
    let bill_id = dataset.load_bill_download(&download)?;
    println!("Loaded bill: {}", bill_id);

    // Access bill data
    let bill = dataset.get_bill(&bill_id).unwrap();
    println!("Amendments: {}", bill.amendments.len());

    Ok(())
}
```

## Core Concepts

### Dataset

The `Dataset` is the primary abstraction for working with versioned legal documents:

- **DatasetMetadata**: Name, description, author, license, version
- **WorkId**: A document as a concept, with no date — `uscode/title_26`
- **ExpressionId**: That work as it read on one date — `uscode/title_26@2025-07-18`
- **Expression**: One expression's tree, plus an optional label
- **Bills**: Parsed bill data with extracted amendments
- **Annotations**: Links diff paths to bill amendments with verification status

Use `Dataset` to load documents, compute diffs, and track which amendment caused each change. Ask `works()` what documents it holds and `expressions(&work)` when each was published; `scope()` reports both together, so an empty result can be answered with "out of scope" rather than "not found".

### USLM Elements

Documents are represented as trees of `USLMElement` structures. Each element contains:

- **ElementData**: Metadata, text content, and identification
- **Children**: Nested child elements forming the document hierarchy

The library uses two types of paths:

1. **Structural Path**: Full hierarchy including all elements
   Example: `uscode/title_26/subtitle_A/chapter_1/section_174`

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

### Amending Actions

Bills can perform these operations on existing code:

`Amend`, `Add`, `Delete`, `Insert`, `Redesignate`, `Repeal`, `Move`, `Strike`, `StrikeAndInsert`

## API Documentation

Generate and view the full API documentation:

```bash
cargo doc --open
```

### Development

```bash
# Run tests
cargo test
```
