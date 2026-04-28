"""Type stubs for words_to_data"""

from typing import Any, Literal

class USLMElement:
    """A hierarchical element in a USLM document tree"""

    @property
    def data(self) -> dict[str, Any]:
        """Element metadata and content"""
        ...

    @property
    def children(self) -> list[USLMElement]:
        """Child elements in document order"""
        ...

    def find(self, path: str) -> USLMElement | None:
        """Find an element by its structural path.

        Args:
            path: The full structural path of the element

        Returns:
            The matching element, or None if not found
        """
        ...

    def to_json(self) -> str:
        """Serialize the element to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the element to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> USLMElement:
        """Deserialize a JSON string to a USLMElement."""
        ...

    def merge_children(self, other: USLMElement) -> None:
        """Merge the children of two nodes into one, retains the caller's ElementData"""

class TextChange:
    """A single word-level change within a text field"""

    @property
    def value(self) -> str:
        """The text value of this change"""
        ...

    @property
    def old_index(self) -> int | None:
        """Position in the original text (None for insertions)"""
        ...

    @property
    def new_index(self) -> int | None:
        """Position in the new text (None for deletions)"""
        ...

    @property
    def tag(self) -> Literal["insert", "delete", "equal"]:
        """The type of change"""
        ...

    def to_json(self) -> str:
        """Serialize the change to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the change to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> TextChange:
        """Deserialize a JSON string to a TextChange."""
        ...

class FieldChangeEvent:
    """A change detected in a single text content field"""

    @property
    def field_name(self) -> Literal["heading", "chapeau", "proviso", "content", "continuation"]:
        """Which text content field changed"""
        ...

    @property
    def from_date(self) -> str:
        """The publication date of the original version"""
        ...

    @property
    def to_date(self) -> str:
        """The publication date of the new version"""
        ...

    @property
    def old_value(self) -> str:
        """The complete original text of the field"""
        ...

    @property
    def new_value(self) -> str:
        """The complete new text of the field"""
        ...

    @property
    def changes(self) -> list[TextChange]:
        """Word-level changes showing insertions, deletions, and unchanged portions"""
        ...

    def to_json(self) -> str:
        """Serialize the field change event to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the field change event to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> FieldChangeEvent:
        """Deserialize a JSON string to a FieldChangeEvent."""
        ...

class TreeDiff:
    """A hierarchical diff between two versions of a USLM document tree"""

    @property
    def root_path(self) -> str:
        """The structural path of the element being compared"""
        ...

    @property
    def changes(self) -> list[FieldChangeEvent]:
        """Text content field changes for this element"""
        ...

    @property
    def from_element(self) -> dict[str, Any]:
        """Metadata from the original version of this element"""
        ...

    @property
    def to_element(self) -> dict[str, Any]:
        """Metadata from the new version of this element"""
        ...

    @property
    def added(self) -> list[dict[str, Any]]:
        """Child elements that were added in the new version"""
        ...

    @property
    def removed(self) -> list[dict[str, Any]]:
        """Child elements that were removed from the old version"""
        ...

    @property
    def child_diffs(self) -> list[TreeDiff]:
        """Recursive diffs for child elements present in both versions"""
        ...

    def find(self, path: str) -> TreeDiff | None:
        """Find a diff by its structural path.

        Args:
            path: The full structural path of the element

        Returns:
            The matching diff, or None if not found
        """
        ...

    def shallow(self) -> TreeDiff:
        """Return a shallow copy of this TreeDiff without children.

        Useful when correlating a specific diff node with other data
        without needing the full subtree.

        Returns:
            A new TreeDiff with the same data but empty child_diffs
        """
        ...

    def calculate_amendment_similarities(
        self, amendment_data: AmendmentData
    ) -> list[AmendmentSimilarity]:
        """Calculate similarity between this TreeDiff and amendment data from a bill.

        Args:
            amendment_data: The parsed amendment data from a bill

        Returns:
            List of AmendmentSimilarity objects for TreeDiff paths that match
        """
        ...

    def scan_for_mentions(
        self, amendment_data: AmendmentData
    ) -> dict[str, list[MentionMatch]]:
        """Scan all amendment texts for mentions of changed sections.

        Uses regexes generated from this TreeDiff to find section mentions
        in each amendment's amending_text.

        Args:
            amendment_data: The parsed amendment data from a bill

        Returns:
            Dictionary mapping amendment_id to list of MentionMatch objects
        """
        ...

    def to_json(self) -> str:
        """Serialize the diff to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the diff to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> TreeDiff:
        """Deserialize a JSON string to a TreeDiff."""
        ...

class AmendmentSimilarity:
    """Similarity between a TreeDiff and a bill amendment.

    Used to rank how likely a BillAmendment caused the changes at a TreeDiff location.
    """

    @property
    def tree_diff_path(self) -> str:
        """The structural path of the TreeDiff node"""
        ...

    @property
    def amendment_id(self) -> str:
        """The ID of the matched BillAmendment"""
        ...

    @property
    def score(self) -> float:
        """Primary ranking metric (F1 score of best-matching BillDiff)"""
        ...

    @property
    def precision(self) -> float:
        """How well the amendment explains the TreeDiff's changes (0.0-1.0)"""
        ...

    @property
    def recall(self) -> float:
        """How much of the amendment is represented in this TreeDiff (0.0-1.0)"""
        ...

    @property
    def matched_words(self) -> int:
        """Number of words that matched between TreeDiff and Amendment"""
        ...

    @property
    def tree_diff_words(self) -> int:
        """Total significant words in the TreeDiff's changes"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> AmendmentSimilarity:
        """Deserialize a JSON string to an AmendmentSimilarity."""
        ...

class MentionMatch:
    """A match found when scanning amendment text for section mentions.

    When scanning bill amendments against a TreeDiff's regexes, this captures
    each match, linking the structural path from the TreeDiff to the text that
    matched in the amendment.
    """

    @property
    def tree_diff_path(self) -> str:
        """The structural path from the TreeDiff that generated this match"""
        ...

    @property
    def matched_text(self) -> str:
        """The text that matched the regex pattern"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> MentionMatch:
        """Deserialize a JSON string to a MentionMatch."""
        ...

def parse_uslm_xml(path: str, date: str) -> USLMElement:
    """Parse a USLM XML file and return as a USLMElement.

    Args:
        path: Path to the USLM XML file
        date: Publication date in YYYY-MM-DD format

    Returns:
        Parsed document as a USLMElement tree
    """
    ...

def load_uslm_folder(path: str, date: str) -> USLMElement | None:
    """Load and merge all USLM XML files from a folder into a single element.

    Reads all .xml files from the folder, parses them in parallel, and merges
    all parsed elements' children into a single root element. Useful for loading
    a complete US Code title that may be split across multiple XML files.

    Args:
        path: Path to directory containing USLM XML files
        date: Publication date in YYYY-MM-DD format

    Returns:
        Merged USLMElement tree, or None if the folder is empty or unreadable
    """
    ...

def compute_diff(old_element: USLMElement, new_element: USLMElement) -> TreeDiff:
    """Compute word-level diff between two USLM documents.

    Args:
        old_element: The original (older) version of the element
        new_element: The new (newer) version of the element

    Returns:
        TreeDiff containing all detected changes

    Raises:
        ValueError: If the two elements don't have the same structural path
    """
    ...

class BillDiff:
    """Word-level changes from a bill amendment instruction.

    Each BillDiff represents one atomic change instruction, such as
    "strike 'specified' and insert 'foreign'".
    """

    def __init__(self, added: list[str], removed: list[str]) -> None:
        """Create a new BillDiff.

        Args:
            added: Words that were added by this instruction
            removed: Words that were removed by this instruction
        """
        ...

    @property
    def added(self) -> list[str]:
        """Words that were added by this instruction"""
        ...

    @property
    def removed(self) -> list[str]:
        """Words that were removed by this instruction"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> BillDiff:
        """Deserialize a JSON string to a BillDiff."""
        ...

class BillAmendment:
    """An amendment found in a bill that modifies the US Code"""

    @property
    def id(self) -> str:
        """Content-based ID: sha256("{bill_id}:{amending_text}") - 64 hex chars"""
        ...

    @property
    def action_types(self) -> list[Literal["amend", "add", "delete", "insert", "redesignate", "repeal", "move", "strike", "strikeandinsert"]]:
        """Types of amending actions performed by this amendment"""
        ...

    @property
    def amending_text(self) -> str:
        """The full readable text of the amending instruction"""
        ...

    @property
    def changes(self) -> list[BillDiff]:
        """Word-level changes extracted from this amendment (populated externally)"""
        ...

    def update_changes(self, changes: list[BillDiff]) -> BillAmendment:
        """Create a new BillAmendment with updated changes.

        Returns a new BillAmendment with the same id, action_types, and amending_text,
        but with the provided changes.

        Args:
            changes: The new list of BillDiff changes

        Returns:
            A new BillAmendment with the updated changes
        """
        ...

    def to_json(self) -> str:
        """Serialize the amendment to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the amendment to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> BillAmendment:
        """Deserialize a JSON string to a BillAmendment."""
        ...

class AmendmentData:
    """Data extracted from a bill document"""

    def __init__(self, bill_id: str, amendments: list[BillAmendment]) -> None:
        """Create a new AmendmentData.

        Args:
            bill_id: The bill identifier (e.g., '119-21' for the 119th Congress, 21st law)
            amendments: List of BillAmendment objects extracted from the bill
        """
        ...

    @property
    def bill_id(self) -> str:
        """The bill identifier (e.g., '119-21' for the 119th Congress, 21st law)"""
        ...

    @property
    def amendments(self) -> list[BillAmendment]:
        """All amendments extracted from the bill"""
        ...

    def to_json(self) -> str:
        """Serialize the amendment data to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize the amendment data to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> AmendmentData:
        """Deserialize a JSON string to an AmendmentData."""
        ...

def parse_bill_amendments(path: str) -> AmendmentData:
    """Parse a Public Law bill and extract amendments to the US Code.

    Args:
        path: Path to the Public Law XML file

    Returns:
        AmendmentData containing the bill ID and all extracted amendments

    Raises:
        ValueError: If the XML is invalid or not a Public Law document
        OSError: If the file cannot be read
    """
    ...

# ============================================================================
# LegalDiff types
# ============================================================================

class BillReference:
    """A reference to a bill that caused a change"""

    def __init__(self, bill_id: str, amendment_id: str, causative_text: str) -> None:
        """Create a new bill reference.

        Args:
            bill_id: The bill identifier (e.g., "119-21")
            amendment_id: The amendment ID (content-hash) linking back to BillAmendment
            causative_text: Text of the amending instruction from the bill
        """
        ...

    @property
    def bill_id(self) -> str:
        """The bill identifier (e.g., "119-21" for Pub. L. 119-21)"""
        ...

    @property
    def amendment_id(self) -> str:
        """The amendment ID (content-hash) linking back to BillAmendment"""
        ...

    @property
    def causative_text(self) -> str:
        """Text of the amending instruction from the bill"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> BillReference:
        """Deserialize a JSON string to a BillReference."""
        ...

class AnnotationMetadata:
    """Metadata about an annotation"""

    @property
    def status(self) -> Literal["pending", "verified", "disputed", "rejected"]:
        """Current verification status of this annotation"""
        ...

    @property
    def confidence(self) -> float | None:
        """Confidence score for AI-generated annotations (0.0 - 1.0), None for human annotations"""
        ...

    @property
    def annotator(self) -> str:
        """Identifier for who/what created this annotation (e.g., "human:username" or "model:gpt-4")"""
        ...

    @property
    def timestamp(self) -> str:
        """When this annotation was created (ISO 8601 format)"""
        ...

    @property
    def notes(self) -> str | None:
        """Freeform notes about the annotation"""
        ...

    @property
    def reasoning(self) -> str | None:
        """Explanation of how/why this annotation was determined"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> AnnotationMetadata:
        """Deserialize a JSON string to an AnnotationMetadata."""
        ...

class ChangeAnnotation:
    """An annotation linking a change to its legal cause"""

    def __init__(
        self,
        operation: Literal["amend", "add", "delete", "insert", "redesignate", "repeal", "move", "strike", "strikeandinsert"],
        bill_id: str,
        amendment_id: str,
        causative_text: str,
        annotator: str,
        paths: list[str],
        confidence: float | None = None,
        notes: str | None = None,
        reasoning: str | None = None,
    ) -> None:
        """Create a new change annotation.

        Args:
            operation: The type of legal operation that caused this change
            bill_id: The bill identifier (e.g., "119-21")
            amendment_id: The amendment ID (content-hash) linking back to BillAmendment
            causative_text: Text of the amending instruction from the bill
            annotator: Identifier for who/what created this annotation
            paths: Structural paths of related changes (for moves, redesignations)
            confidence: Confidence score for AI-generated annotations (0.0 - 1.0)
            notes: Freeform notes about the annotation
            reasoning: Explanation of how/why this annotation was determined
        """
        ...

    @property
    def operation(self) -> Literal["amend", "add", "delete", "insert", "redesignate", "repeal", "move", "strike", "strikeandinsert"]:
        """The type of legal operation that caused this change"""
        ...

    @property
    def source_bill(self) -> BillReference:
        """Reference to the bill that enacted the change"""
        ...

    @property
    def metadata(self) -> AnnotationMetadata:
        """Metadata about the annotation itself"""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> ChangeAnnotation:
        """Deserialize a JSON string to a ChangeAnnotation."""
        ...

class LegalDiff:
    """A legal diff combining word-level changes with semantic annotations"""

    def __init__(self, tree_diff: TreeDiff) -> None:
        """Create a new LegalDiff from an existing TreeDiff with no annotations.

        Args:
            tree_diff: The underlying word-level diff
        """
        ...

    @property
    def tree_diff(self) -> TreeDiff:
        """The underlying word-level diffs"""
        ...

    @property
    def annotations_dict(self) -> dict[str, list[dict[str, Any]]]:
        """All annotations as a dictionary (path -> list of annotation dicts)"""
        ...

    @property
    def amendments_dict(self) -> dict[str, dict[str, Any]]:
        """The amendments that were annotated, keyed by amendment_id"""
        ...

    def add_annotation(self, annotation: ChangeAnnotation) -> None:
        """Add an annotation for a specific structural path.

        Args:
            path: The structural path to annotate
            annotation: The annotation to add
        """
        ...

    def get_annotations(self, path: str) -> list[ChangeAnnotation]:
        """Get all annotations for a specific path.

        Args:
            path: The structural path to look up

        Returns:
            List of annotations for the path (empty if none exist)
        """
        ...

    def get_diff_node(self, path: str) -> TreeDiff | None:
        """Get the TreeDiff node for a specific path.

        Args:
            path: The structural path to look up

        Returns:
            The TreeDiff node, or None if not found
        """
        ...

    def annotated_paths(self) -> list[str]:
        """Get all paths that have annotations."""
        ...

    def unannotated_paths(self) -> list[str]:
        """Get all paths in the TreeDiff that lack annotations."""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a dictionary."""
        ...

    @staticmethod
    def from_json(json_str: str) -> LegalDiff:
        """Deserialize a JSON string to a LegalDiff."""
        ...

# ============================================================================
# Dataset types
# ============================================================================

class DatasetMetadata:
    """Metadata for a Dataset"""

    def __init__(
        self,
        name: str,
        description: str,
        author: str,
        source_urls: list[str],
        license: str,
        version: str,
    ) -> None:
        """Create dataset metadata.

        Args:
            name: Name of the dataset
            description: Description of the dataset
            author: Author or organization
            source_urls: URLs where source data was obtained
            license: License for the dataset
            version: Version string for the dataset
        """
        ...

    @property
    def name(self) -> str:
        """Name of the dataset"""
        ...

    @property
    def description(self) -> str:
        """Description of the dataset"""
        ...

    @property
    def author(self) -> str:
        """Author or organization"""
        ...

    @property
    def source_urls(self) -> list[str]:
        """URLs where source data was obtained"""
        ...

    @property
    def license(self) -> str:
        """License for the dataset"""
        ...

    @property
    def version(self) -> str:
        """Version string for the dataset"""
        ...


class VersionSnapshot:
    """A snapshot of a USLMElement at a specific point in time"""

    def __init__(
        self, date: str, element: USLMElement, label: str | None
    ) -> None:
        """Create a version snapshot.

        Args:
            date: Date in YYYY-MM-DD format
            element: The element tree at this version
            label: Optional human-readable label (e.g., "Pre-Tax Cuts Act")
        """
        ...

    @property
    def date(self) -> str:
        """Date in YYYY-MM-DD format"""
        ...

    @property
    def label(self) -> str | None:
        """Optional human-readable label"""
        ...

    @property
    def element(self) -> USLMElement:
        """The element tree at this version"""
        ...


class SearchResult:
    """A search result from Dataset.search_text"""

    @property
    def date(self) -> str:
        """Version date where match was found"""
        ...

    @property
    def path(self) -> str:
        """Structural path of matching element"""
        ...

    @property
    def field(self) -> str:
        """Field name containing match (heading, content, etc.)"""
        ...

    @property
    def snippet(self) -> str:
        """Text snippet containing the match"""
        ...


class Dataset:
    """A versioned collection of legal documents with bill annotations"""

    def __init__(self, metadata: DatasetMetadata) -> None:
        """Create a new empty dataset with the given metadata.

        Args:
            metadata: Metadata describing the dataset
        """
        ...

    @staticmethod
    def load(path: str) -> Dataset:
        """Load a dataset from a JSON file.

        Args:
            path: Path to the JSON file

        Returns:
            The loaded dataset

        Raises:
            OSError: If the file cannot be read
            ValueError: If the JSON is invalid
        """
        ...

    def save(self, path: str) -> None:
        """Save the dataset to a JSON file.

        Args:
            path: Path where the JSON file will be written

        Raises:
            OSError: If the file cannot be written
        """
        ...

    @property
    def metadata(self) -> DatasetMetadata:
        """Dataset metadata"""
        ...

    @property
    def versions(self) -> list[VersionSnapshot]:
        """Chronologically sorted version snapshots"""
        ...

    @property
    def bills(self) -> list[AmendmentData]:
        """Bills that caused changes in this dataset"""
        ...

    def get_annotations(
        self, from_date: str, to_date: str
    ) -> list[ChangeAnnotation] | None:
        """Get annotations for a specific version pair.

        Args:
            from_date: Date of the older version
            to_date: Date of the newer version

        Returns:
            List of annotations for the version pair, or None if not found
        """
        ...

    def add_annotation(
        self, from_date: str, to_date: str, annotation: ChangeAnnotation
    ) -> None:
        """Add an annotation for a specific version pair.

        Args:
            from_date: Date of the older version
            to_date: Date of the newer version
            annotation: The annotation to add
        """
        ...

    def annotated_paths(self, from_date: str, to_date: str) -> list[str]:
        """Get paths that have annotations for a version pair.

        Args:
            from_date: Date of the older version
            to_date: Date of the newer version

        Returns:
            List of annotated paths
        """
        ...

    def unannotated_paths(self, from_date: str, to_date: str) -> list[str]:
        """Get paths with changes that lack annotations for a version pair.

        Args:
            from_date: Date of the older version
            to_date: Date of the newer version

        Returns:
            List of unannotated paths

        Raises:
            ValueError: If either version is not found
        """
        ...

    def add_version(self, snapshot: VersionSnapshot) -> None:
        """Add a version snapshot, maintaining chronological order.

        Args:
            snapshot: The version snapshot to add
        """
        ...

    def get_version(self, date: str) -> VersionSnapshot | None:
        """Get a version snapshot by exact date.

        Args:
            date: Date in YYYY-MM-DD format

        Returns:
            The version snapshot, or None if not found
        """
        ...

    def get_version_by_label(self, label: str) -> VersionSnapshot | None:
        """Get a version snapshot by label.

        Args:
            label: The label to search for

        Returns:
            The version snapshot, or None if not found
        """
        ...

    def next_version(self, date: str) -> VersionSnapshot | None:
        """Get the version after the given date.

        Args:
            date: Date in YYYY-MM-DD format

        Returns:
            The next version snapshot, or None if at end
        """
        ...

    def prev_version(self, date: str) -> VersionSnapshot | None:
        """Get the version before the given date.

        Args:
            date: Date in YYYY-MM-DD format

        Returns:
            The previous version snapshot, or None if at start
        """
        ...

    def compute_diff(self, from_date: str, to_date: str) -> TreeDiff:
        """Compute diff between two versions by date.

        Args:
            from_date: Date of the older version
            to_date: Date of the newer version

        Returns:
            TreeDiff between the two versions

        Raises:
            ValueError: If either version is not found
        """
        ...

    def add_bill(self, bill: AmendmentData) -> None:
        """Add a bill to the dataset.

        Args:
            bill: The bill's amendment data
        """
        ...

    def get_bill(self, bill_id: str) -> AmendmentData | None:
        """Get a bill by its ID.

        Args:
            bill_id: The bill identifier (e.g., "119-21")

        Returns:
            The amendment data, or None if not found
        """
        ...

    def annotations_for_path(self, path: str) -> list[ChangeAnnotation]:
        """Get all annotations that include the given path.

        Args:
            path: The structural path to search for

        Returns:
            List of annotations for this path
        """
        ...

    def annotations_for_bill(self, bill_id: str) -> list[ChangeAnnotation]:
        """Get all annotations associated with the given bill ID.

        Args:
            bill_id: The bill identifier

        Returns:
            List of annotations from this bill
        """
        ...

    def search_text(self, query: str) -> list[SearchResult]:
        """Search for text across all versions.

        Args:
            query: Text to search for (case-insensitive)

        Returns:
            List of search results
        """
        ...

    def find_element(self, path: str) -> list[tuple[str, USLMElement]]:
        """Find an element by path across all versions.

        Args:
            path: The structural path to search for

        Returns:
            List of (date, element) tuples for each version containing the path
        """
        ...

    def add_uslm_xml(
        self, xml_path: str, date: str, label: str | None = None
    ) -> None:
        """Parse a USLM XML file and add it as a version snapshot.

        Args:
            xml_path: Path to the USLM XML file
            date: Publication date in YYYY-MM-DD format
            label: Optional human-readable label for this version

        Raises:
            ValueError: If XML parsing fails
            OSError: If file cannot be read
        """
        ...

    def add_uslm_folder(
        self, folder_path: str, date: str, label: str | None = None
    ) -> None:
        """Load all USLM XML files from a folder and add as a merged version snapshot.

        Args:
            folder_path: Path to directory containing USLM XML files
            date: Publication date in YYYY-MM-DD format
            label: Optional human-readable label for this version
        """
        ...

    def to_json(self) -> str:
        """Serialize the dataset to a JSON string."""
        ...


    def add_changes_to_amendment(self, amendment_id: str, bill_diff: BillDiff) -> None:
        """Add changes to an existing amendment in the dataset.

        Args:
            amendment_id: The content-hash ID of the amendment
            bill_diff: The diff to add to the amendment's changes
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> Dataset:
        """Deserialize a JSON string to a Dataset."""
        ...


__version__: str
__all__: list[str]
