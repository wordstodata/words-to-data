//! Links projected from real stored annotations.
//!
//! The fixture is `tests/test_data/processed/annotations.json`, the output of a
//! real matching run, not invented data.

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::link::{Link, LinkKind, Target, VerificationState};

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
