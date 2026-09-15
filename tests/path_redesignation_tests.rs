//! `path` pairs provisions over the redesignation links, not over the path
//! string (#165).
//!
//! `119-hr-1` put a new subparagraph at `26 U.S.C. § 45X(c)(6)(R)` and moved
//! (R) through (Z) down to (S) through (AA). Asked about (R), the command used
//! to answer "in both", and to report the heading changing from " Neodymium"
//! to " Metallurgical coal". Those are two different subparagraphs. Neodymium
//! is at (S), and no bill touched its words.
//!
//! Every case here is read out of the committed corpus.

use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::inspect::{self, NotFollowed, PathMatch, PathReport, Presence};
use words_to_data::link::{LinkKind, VerificationState};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;
use words_to_data::uslm::parser::parse;

const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// `26 U.S.C. § 45X(c)(6)`, the paragraph the bill renumbered through.
const PARAGRAPH_45X_C_6: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X/subsection_c/paragraph_6";

/// One subparagraph of that paragraph, by its letter.
fn subparagraph(letter: &str) -> String {
    format!("{PARAGRAPH_45X_C_6}/subparagraph_{letter}")
}

/// Title 26 at both release points, with `119-hr-1`'s redesignations recorded
/// as links, and the pair of expressions they name.
fn title_26_with_redesignations() -> (Dataset<InMemoryStorage>, ExpressionId, ExpressionId) {
    let work = WorkId::new("uscode/title_26");
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(file, date).expect("title 26 should parse");
        let root = work_roots(parsed).pop().expect("the file holds one title");
        dataset
            .add_expression(Expression {
                id: ExpressionId::new(work.clone(), date),
                label: None,
                root,
            })
            .expect("the expression should store");
    }
    let from = ExpressionId::new(work.clone(), BEFORE);
    let to = ExpressionId::new(work, AFTER);

    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    dataset
        .record_redesignations(BILL_ID, &stated, &from, &to)
        .expect("the redesignations should record");
    (dataset, from, to)
}

/// The report for one subparagraph of § 45X(c)(6), across the two release
/// points.
fn report_for(letter: &str) -> PathReport {
    let (dataset, from, to) = title_26_with_redesignations();
    inspect::path_report(
        &dataset,
        &subparagraph(letter),
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("a path report should build")
}

#[test]
fn should_report_a_move_out_and_an_addition_when_a_bill_inserted_a_subparagraph() {
    let report = report_for("R");

    assert_eq!(
        report.provisions.len(),
        2,
        "two provisions touch (R): the one that left it and the one that took it, got {:?}",
        report.provisions
    );

    // The neodymium subparagraph left (R) for (S), and the bill changed no word
    // of it.
    assert_eq!(
        report.provisions[0].presence,
        Presence::MovedOut {
            to_path: subparagraph("S")
        }
    );
    assert_eq!(report.provisions[0].from_position, Some(0));
    assert!(
        report.provisions[0].changes.is_empty(),
        "the renumbering changed no words, got {:?}",
        report.provisions[0].changes
    );

    // The metallurgical coal subparagraph is new law at (R).
    assert_eq!(report.provisions[1].presence, Presence::Added);
    assert_eq!(report.provisions[1].to_position, Some(0));

    // And the false statement is gone.
    assert!(
        !report
            .provisions
            .iter()
            .any(|p| p.presence == Presence::InBoth),
        "nothing at (R) is one provision across the two dates, got {:?}",
        report.provisions
    );
    assert!(
        !report
            .provisions
            .iter()
            .flat_map(|p| &p.changes)
            .any(|c| c.old_value.contains("Neodymium") && c.new_value.contains("coal")),
        "neodymium never became metallurgical coal, got {:?}",
        report.provisions
    );
}

#[test]
fn should_report_a_move_out_and_a_move_in_when_the_letters_cascade() {
    // The letters cascade, so every letter after (R) carries a provision away
    // and receives another. (S) used to claim that nickel became neodymium and
    // (T) that niobium became nickel. Both statements are false: the bill moved
    // each subparagraph down one letter and changed no word of any of them.
    for (letter, left_for, came_from) in [("S", "T", "R"), ("T", "U", "S")] {
        let report = report_for(letter);

        assert_eq!(
            report.provisions.len(),
            2,
            "({letter}) holds one provision that left and one that arrived, got {:?}",
            report.provisions
        );
        assert_eq!(
            report.provisions[0].presence,
            Presence::MovedOut {
                to_path: subparagraph(left_for)
            }
        );
        assert!(
            report.provisions[0].changes.is_empty(),
            "({letter}) moved to ({left_for}) with its words untouched, got {:?}",
            report.provisions[0].changes
        );
        assert_eq!(
            report.provisions[1].presence,
            Presence::MovedIn {
                from_path: subparagraph(came_from)
            }
        );
        assert!(
            report.provisions[1].changes.is_empty(),
            "({came_from}) moved to ({letter}) with its words untouched, got {:?}",
            report.provisions[1].changes
        );
        assert!(
            !report
                .provisions
                .iter()
                .any(|p| p.presence == Presence::InBoth),
            "({letter}) names two different provisions across the two dates, got {:?}",
            report.provisions
        );
    }
}

#[test]
fn should_name_the_bill_the_dates_and_the_trust_when_a_provision_moved() {
    // A move is a claim, and a reader must be able to weigh it. The link is
    // machine suggested: a rule read a sentence in a bill, and no person has
    // confirmed the reading.
    let report = report_for("R");

    let via = &report.provisions[0].via;
    assert_eq!(via.len(), 1, "one link carried the move, got {via:?}");
    assert_eq!(via[0].bill_id.as_deref(), Some(BILL_ID));
    assert_eq!(via[0].from_date, BEFORE);
    assert_eq!(via[0].to_date, AFTER);
    assert_eq!(via[0].verification, VerificationState::MachineSuggested);
}

#[test]
fn should_count_the_links_and_walk_nothing_when_no_expression_pair_is_given() {
    // With no window there is nothing to resolve. But a reader who asks about a
    // path that redesignation links name, and gets a clean answer, has no way
    // to learn that the path names different provisions on different dates.
    let (dataset, _, _) = title_26_with_redesignations();

    let report = inspect::path_report(&dataset, &subparagraph("S"), None, PathMatch::Subtree)
        .expect("a path report should build");

    assert!(
        report.provisions.is_empty(),
        "nothing is paired without a window, got {:?}",
        report.provisions
    );
    // (R) became (S), and (S) became (T). Both links name this path.
    assert_eq!(report.unfollowed_redesignations.len(), 2);
    assert!(
        report
            .unfollowed_redesignations
            .iter()
            .all(|u| u.reason == NotFollowed::NoWindow),
        "every one is unfollowed for want of a window, got {:?}",
        report.unfollowed_redesignations
    );
}

#[test]
fn should_refuse_a_refuted_link_and_say_it_exists() {
    // A reviewer checked the corpus's own (R) to (S) link and found it wrong.
    // Reporting a statement known to be false is worse than the string pairing
    // this report replaces, so the link is not followed. Falling back in
    // silence would give the reader the old false answer with nothing to show
    // that a link was passed over.
    let (dataset, from, to) = title_26_reviewed_as(VerificationState::Refuted);

    let report = inspect::path_report(
        &dataset,
        &subparagraph("R"),
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("a path report should build");

    let refused = &report.unfollowed_redesignations;
    assert_eq!(refused.len(), 1, "the refuted link is named, got {refused:?}");
    assert_eq!(refused[0].reason, NotFollowed::Refuted);
    assert_eq!(refused[0].link.from_path, subparagraph("R"));
    assert_eq!(refused[0].link.to_path, subparagraph("S"));
    assert!(
        !report
            .provisions
            .iter()
            .any(|p| matches!(p.presence, Presence::MovedOut { .. })),
        "a refuted move is not reported as a move, got {:?}",
        report.provisions
    );
}

#[test]
fn should_follow_a_disputed_link_and_mark_it() {
    // `Disputed` means someone objects and it is unsettled, which is a weaker
    // claim than `Refuted`. The move is reported, and the state travels with it
    // so a reader can see the objection.
    let (dataset, from, to) = title_26_reviewed_as(VerificationState::Disputed);

    let report = inspect::path_report(
        &dataset,
        &subparagraph("R"),
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("a path report should build");

    assert_eq!(
        report.provisions[0].presence,
        Presence::MovedOut {
            to_path: subparagraph("S")
        }
    );
    assert_eq!(
        report.provisions[0].via[0].verification,
        VerificationState::Disputed,
        "the objection is printed beside the claim"
    );
    assert!(
        report.unfollowed_redesignations.is_empty(),
        "a disputed link is followed, got {:?}",
        report.unfollowed_redesignations
    );
}

/// The corpus, with a reviewer's verdict recorded on the `(R)` to `(S)` link.
///
/// The link itself is the corpus's own, with its real paths, dates and bill.
/// Only the verification state changes, which is exactly what a review records.
fn title_26_reviewed_as(
    verdict: VerificationState,
) -> (Dataset<InMemoryStorage>, ExpressionId, ExpressionId) {
    let (mut dataset, from, to) = title_26_with_redesignations();

    let mut reviewed = dataset
        .links_for_path(&subparagraph("R"))
        .expect("links should read")
        .into_iter()
        .find(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .expect("(R) carries a redesignation link");
    reviewed.provenance.verification = verdict;
    dataset.add_link(reviewed).expect("the verdict should store");

    (dataset, from, to)
}

/// `26 U.S.C. § 951A`, where `119-hr-1` renumbered whole subsections: (c)
/// became (b), (e) became (c) and (f) became (d).
const SECTION_951A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_III/subpart_F/section_951A";

#[test]
fn should_carry_a_child_along_when_a_bill_renumbered_its_parent() {
    // The bill said nothing about paragraph (1). It renumbered subsection (c)
    // to subsection (b), and the paragraph went with it. Pairing by path string
    // therefore compared the old (c)(1), about net CFC tested income, with the
    // new (c)(1), which is the old (e)(1) about pro rata shares.
    let (dataset, from, to) = title_26_with_redesignations();

    let report = inspect::path_report(
        &dataset,
        &format!("{SECTION_951A}/subsection_c/paragraph_1"),
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("a path report should build");

    assert_eq!(
        report.provisions[0].presence,
        Presence::MovedOut {
            to_path: format!("{SECTION_951A}/subsection_b/paragraph_1")
        },
        "the paragraph went where its subsection went, got {:?}",
        report.provisions
    );
    assert_eq!(
        report.provisions[1].presence,
        Presence::MovedIn {
            from_path: format!("{SECTION_951A}/subsection_e/paragraph_1")
        },
        "and the paragraph now here came from the subsection that became (c), got {:?}",
        report.provisions
    );
    assert!(
        !report
            .provisions
            .iter()
            .flat_map(|p| &p.changes)
            .any(|c| c.old_value.contains("net CFC tested income")
                && c.new_value.contains("pro rata shares")),
        "net CFC tested income never became pro rata shares, got {:?}",
        report.provisions
    );
}
