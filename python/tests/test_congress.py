"""Test Python bindings for Congress.gov API types."""
from pathlib import Path

import pytest
from words_to_data import (
    Member, Party, Chamber,
    Dataset, DatasetMetadata,
    CongressClient, BillDownload,
)



def test_dataset_member_integration():
    """Test adding members to Dataset."""
    meta = DatasetMetadata(
        name="Test",
        description="Test dataset",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0",
    )
    dataset = Dataset(meta)

    member = Member(
        bioguide_id="L000174",
        name="Patrick J. Leahy",
        first_name="Patrick",
        last_name="Leahy",
        party=Party.democrat(),
        state="VT",
        district=None,
        chamber=Chamber.senate(),
    )

    dataset.add_member(member)

    retrieved = dataset.get_member("L000174")
    assert retrieved is not None
    assert retrieved.last_name == "Leahy"


def test_dataset_load_bill_download():
    """Test loading BillDownload into Dataset."""
    # Never expire the cache: the fixtures are committed and must be read
    # regardless of their on-disk age.
    client = CongressClient.with_ttl("", "tests/test_data/congress_client_cache", None)
    download = client.download_bill("119-hr-1")

    meta = DatasetMetadata(
        name="Test",
        description="Test",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0",
    )
    dataset = Dataset(meta)

    bill_id = dataset.load_bill_download(download)

    assert bill_id == "119-hr-1"  # Uses bill_id from BillDownload
    assert dataset.get_bill(bill_id) is not None
    assert dataset.get_sponsor_info(bill_id) is not None
