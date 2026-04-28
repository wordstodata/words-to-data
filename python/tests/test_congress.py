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
    # Get test data paths
    test_dir = Path(__file__).parent.parent.parent / "tests" / "test_data"
    bill_xml = (test_dir / "bills" / "119-hr-1.xml").read_text()
    member_json = (test_dir / "congress" / "members" / "L000174.json").read_text()

    download = BillDownload(
        bill_id="119-hr-1",
        bill_xml=bill_xml,
        sponsors_json='{"bill":{"sponsors":[{"bioguideId":"L000174"}]}}',
        cosponsors_json='{"cosponsors":[]}',
        votes_json=None,
        member_jsons={"L000174": member_json},
    )

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
    assert dataset.get_member("L000174") is not None
    assert dataset.get_sponsor_info(bill_id) is not None
