//! Links projected from real stored annotations.
//!
//! The fixture is `tests/test_data/processed/annotations.json`, the output of a
//! real matching run, not invented data.

use words_to_data::annotation::{AnnotationStatus, ChangeAnnotation};
use words_to_data::link::{Corroboration, Link, LinkKind, Target, VerificationState};

const ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";

fn real_annotations() -> Vec<ChangeAnnotation> {
    let json = std::fs::read_to_string(ANNOTATIONS).expect("the fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse as annotations")
}

#[test]
fn should_project_a_stored_annotation_into_one_link_per_path() {
    let annotations = real_annotations();
    let annotation = annotations.first().expect("the fixture should hold some");

    let links = Link::from_annotation(annotation);

    assert_eq!(
        links.len(),
        annotation.paths.len(),
        "each path is its own statement, because each can be confirmed alone"
    );

    let link = links.first().expect("at least one link");
    assert_eq!(link.kind, LinkKind::new(LinkKind::AMENDED_BY));
    assert_eq!(link.kind.namespace(), "legislature");
    assert_eq!(
        link.subject,
        Target::Provision(annotation.paths[0].clone()),
        "the subject is the provision that changed"
    );
}

/// An amendment is a legislature concept, so a link reaches it as an external
/// reference in that namespace. A reader that knows nothing of bills can still
/// report that the change was caused by something, and name it.
#[test]
fn should_reach_the_amendment_as_an_external_reference() {
    let annotations = real_annotations();
    let annotation = annotations.first().expect("the fixture should hold some");

    let links = Link::from_annotation(annotation);
    let link = links.first().expect("at least one link");

    match &link.object {
        Target::External { reference, display } => {
            assert!(
                reference.starts_with("legislature.amendment:"),
                "the reference should name its namespace, got {reference}"
            );
            assert!(
                reference.ends_with(&annotation.source_bill.amendment_id),
                "the reference should carry the amendment id"
            );
            assert_eq!(display, &annotation.source_bill.causative_text);
        }
        other => panic!("an amendment should be an external reference, got {other:?}"),
    }
}

/// The fixture was produced by a model and never reviewed by a person, so every
/// statement in it is machine-suggested. The model's score is kept, but as
/// diagnostic data, never as the trust level.
#[test]
fn should_mark_unreviewed_model_output_as_machine_suggested() {
    let annotations = real_annotations();
    let from_model: Vec<&ChangeAnnotation> = annotations
        .iter()
        .filter(|a| a.metadata.annotator.starts_with("model:"))
        .collect();

    assert!(
        !from_model.is_empty(),
        "the fixture should hold model-made annotations"
    );

    for annotation in from_model {
        for link in Link::from_annotation(annotation) {
            assert_eq!(
                link.provenance.verification,
                VerificationState::MachineSuggested
            );
            assert_eq!(link.provenance.source, annotation.metadata.annotator);
            assert_eq!(link.provenance.raw_score, annotation.metadata.confidence);
            assert!(
                link.provenance.evidence.is_none(),
                "raw model replies were never persisted (#58), so there is no evidence to carry"
            );
        }
    }
}

/// The stored `Rejected` status means the claim was checked and found wrong.
/// That is settled, unlike `Disputed`, which means someone objects. The real
/// fixture holds only `Pending`, so this takes a real annotation and varies the
/// one field under test.
#[test]
fn should_mark_a_rejected_annotation_as_refuted_rather_than_disputed() {
    let mut annotation = real_annotations()
        .into_iter()
        .next()
        .expect("the fixture should hold some");

    annotation.metadata.status = AnnotationStatus::Rejected;
    for link in Link::from_annotation(&annotation) {
        assert_eq!(link.provenance.verification, VerificationState::Refuted);
    }

    annotation.metadata.status = AnnotationStatus::Disputed;
    for link in Link::from_annotation(&annotation) {
        assert_eq!(
            link.provenance.verification,
            VerificationState::Disputed,
            "an objection is not a finding of falsehood"
        );
    }
}

/// Corroboration is reproducible, so it may be relied on. It does not raise the
/// verification state: a machine's proposal that scores well is still a
/// machine's proposal.
#[test]
fn should_carry_corroboration_without_raising_the_verification_state() {
    let annotations = real_annotations();
    let annotation = annotations.first().expect("the fixture should hold some");

    let link = Link::from_annotation(annotation)
        .into_iter()
        .next()
        .expect("at least one link")
        .with_corroboration(Corroboration {
            method: "precision_weighted_f1".to_string(),
            score: 0.82,
            detail: vec![("precision".to_string(), 0.9)],
        });

    assert_eq!(
        link.provenance.verification,
        VerificationState::MachineSuggested,
        "evidence is not confirmation"
    );
    let corroboration = link
        .provenance
        .corroboration
        .expect("the measurement should be carried");
    assert_eq!(corroboration.method, "precision_weighted_f1");
}
