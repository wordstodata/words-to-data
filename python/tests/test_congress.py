"""Test Python bindings for Congress.gov API types."""
import os
import tempfile
from pathlib import Path

import pytest
from words_to_data import (
    Member, Party, Chamber, MemberTerm,
    VotePosition, VoteResult, RollCall,
    SponsorInfo, CosponsorRecord,
    Dataset, DatasetMetadata,
    CongressClient, BillDownload,
)


def test_party_creation():
    """Test Party enum creation."""
    dem = Party.democrat()
    rep = Party.republican()
    ind = Party.independent()
    other = Party.other("Green")

    assert dem.is_democrat()
    assert rep.is_republican()
    assert ind.is_independent()
    assert other.name() == "Green"


def test_chamber_creation():
    """Test Chamber enum creation."""
    senate = Chamber.senate()
    house = Chamber.house()

    assert senate.is_senate()
    assert house.is_house()


def test_member_creation():
    """Test Member struct creation and access."""
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

    assert member.bioguide_id == "L000174"
    assert member.last_name == "Leahy"
    assert member.party.is_democrat()
    assert member.district is None
    assert member.chamber.is_senate()


def test_vote_position_creation():
    """Test VotePosition enum creation."""
    yea = VotePosition.yea()
    nay = VotePosition.nay()
    present = VotePosition.present()
    not_voting = VotePosition.not_voting()

    assert yea.is_yea()
    assert nay.is_nay()
    assert present.is_present()
    assert not_voting.is_not_voting()


def test_roll_call_creation():
    """Test RollCall struct creation."""
    votes = {"L000174": VotePosition.yea(), "B000575": VotePosition.nay()}

    roll = RollCall(
        congress=118,
        session=1,
        roll_number=100,
        chamber=Chamber.senate(),
        date="2023-05-01",
        bill_id="118-hr-1234",
        result=VoteResult.passed(),
        votes=votes,
    )

    assert roll.congress == 118
    assert roll.roll_number == 100
    assert roll.chamber.is_senate()


def test_sponsor_info_creation():
    """Test SponsorInfo and CosponsorRecord creation."""
    cosponsor = CosponsorRecord(
        bioguide_id="B000575",
        date="2023-01-15",
        withdrawn=False,
    )

    sponsor = SponsorInfo(
        bill_id="118-hr-1234",
        sponsor="L000174",
        cosponsors=[cosponsor],
    )

    assert sponsor.bill_id == "118-hr-1234"
    assert sponsor.sponsor == "L000174"
    assert len(sponsor.cosponsors) == 1
    assert not sponsor.cosponsors[0].withdrawn


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


def test_dataset_sponsor_info_integration():
    """Test adding sponsor info to Dataset."""
    meta = DatasetMetadata(
        name="Test",
        description="Test dataset",
        author="Test",
        source_urls=[],
        license="MIT",
        version="1.0",
    )
    dataset = Dataset(meta)

    sponsor = SponsorInfo(
        bill_id="118-hr-1234",
        sponsor="L000174",
        cosponsors=[],
    )

    dataset.add_sponsor_info(sponsor)

    retrieved = dataset.get_sponsor_info("118-hr-1234")
    assert retrieved is not None
    assert retrieved.sponsor == "L000174"


def test_congress_client_creation():
    """Test CongressClient can be created."""
    with tempfile.TemporaryDirectory() as tmpdir:
        client = CongressClient(api_key="test_key", cache_dir=tmpdir)
        assert client.api_key == "test_key"


def test_bill_download_creation():
    """Test BillDownload struct creation."""
    download = BillDownload(
        bill_id="119-pl-21",
        bill_xml="<xml>test</xml>",
        sponsors_json='{"bill":{}}',
        cosponsors_json='{"cosponsors":[]}',
        votes_json=None,
        member_jsons={},
    )

    assert download.bill_id == "119-pl-21"
    assert download.bill_xml == "<xml>test</xml>"
    assert download.votes_json is None


def test_dataset_load_bill_download():
    """Test loading BillDownload into Dataset."""
    # Get test data paths
    test_dir = Path(__file__).parent.parent.parent / "tests" / "test_data"
    bill_xml = (test_dir / "bills" / "pl-119-21.xml").read_text()
    member_json = (test_dir / "congress" / "members" / "L000174.json").read_text()

    download = BillDownload(
        bill_id="119-pl-21",
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

    assert bill_id == "119-21"  # Canonical ID from XML
    assert dataset.get_bill(bill_id) is not None
    assert dataset.get_member("L000174") is not None
    assert dataset.get_sponsor_info(bill_id) is not None
