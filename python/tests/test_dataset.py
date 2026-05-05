import os
import tempfile

from words_to_data import (
    Dataset,
    DatasetMetadata,
    VersionSnapshot,
    parse_uslm_xml,
    parse_bill_amendments,
)

# def test_dl():
#     from words_to_data import CongressClient
#     metadata = DatasetMetadata(
#         name="Test Dataset",
#         description="For testing",
#         author="Test Author",
#         source_urls=["https://example.com"],
#         license="MIT",
#         version="1.0.0",
#     )
#     dataset = Dataset(metadata)
#     client = CongressClient(api_key=os.environ["CONGRESS_API_KEY"], cache_dir="./cache")
#     download = client.download_bill("119-hr-1")
#     assert False


def test_add_version():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    elem = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
    snapshot = VersionSnapshot("2025-07-18", elem, "First version")
    dataset.add_version(snapshot)

    assert len(dataset.versions) == 1
    assert dataset.versions[0].date == "2025-07-18"
    assert dataset.versions[0].label == "First version"


def test_get_version():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    elem = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
    dataset.add_version(VersionSnapshot("2025-07-18", elem, "First"))

    found = dataset.get_version("2025-07-18")
    assert found is not None
    assert found.label == "First"

    not_found = dataset.get_version("2000-01-01")
    assert not_found is None


def test_compute_diff():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    elem1 = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
    elem2 = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30")

    dataset.add_version(VersionSnapshot("2025-07-18", elem1, None))
    dataset.add_version(VersionSnapshot("2025-07-30", elem2, None))

    diff = dataset.compute_diff("2025-07-18", "2025-07-30")
    assert diff.root_path == "uscode"


def test_save_and_load():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    elem = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
    dataset.add_version(VersionSnapshot("2025-07-18", elem, "First"))

    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
        path = f.name

    try:
        dataset.save(path)
        loaded = Dataset.load(path)

        assert loaded.metadata.name == "Test"
        assert len(loaded.versions) == 1
        assert loaded.versions[0].date == "2025-07-18"
    finally:
        os.unlink(path)


def test_add_and_query_bills():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    bill = parse_bill_amendments("119-21", "tests/test_data/bills/119-hr-1/bill_119_hr_1.xml")
    dataset.add_bill(bill)

    assert len(dataset.bills) == 1

    found = dataset.get_bill(bill.bill_id)
    assert found is not None
    assert found.bill_id == "119-21"

    not_found = dataset.get_bill("nonexistent")
    assert not_found is None


def test_add_uslm_folder_invalid_path_raises_os_error():
    """Regression: add_uslm_folder previously panicked (expect()) on bad paths."""
    import pytest

    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    with pytest.raises(OSError):
        dataset.add_uslm_folder("/nonexistent/path/that/does/not/exist", "2025-01-01")


def test_version_matches_cargo_toml():
    """Cargo.toml is the source of truth; pyproject.toml and __version__ must agree."""
    import tomllib
    import words_to_data
    from pathlib import Path

    repo_root = Path(__file__).parent.parent.parent
    with open(repo_root / "Cargo.toml", "rb") as f:
        cargo = tomllib.load(f)
    with open(repo_root / "pyproject.toml", "rb") as f:
        pyproject = tomllib.load(f)

    cargo_version = cargo["package"]["version"]
    assert pyproject["project"]["version"] == cargo_version, (
        f"pyproject.toml version {pyproject['project']['version']!r} "
        f"does not match Cargo.toml version {cargo_version!r}"
    )
    assert words_to_data.__version__ == cargo_version, (
        f"words_to_data.__version__ {words_to_data.__version__!r} "
        f"does not match Cargo.toml version {cargo_version!r}"
    )


def test_search_text():
    metadata = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0.0",
    )
    dataset = Dataset(metadata)

    elem = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
    dataset.add_version(VersionSnapshot("2025-07-18", elem, None))

    results = dataset.search_text("Agriculture")
    assert len(results) > 0
    assert results[0].date == "2025-07-18"
