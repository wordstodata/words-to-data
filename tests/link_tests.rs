//! Links projected from real stored annotations.
//!
//! The fixture is `tests/test_data/processed/annotations.json`, the output of a
//! real matching run, not invented data.

use words_to_data::annotation::{AnnotationStatus, ChangeAnnotation};
use words_to_data::dataset::{ExpressionId, WorkId};
use words_to_data::diff::AmendmentSimilarity;
use words_to_data::link::{Corroboration, Link, LinkKind, Target, VerificationState};

const ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";

/// The pair the fixture's annotations sit between.
///
/// A link's subject is a change, and a change is not addressable without the
/// dates it changed between, so the projection now needs the pair.
fn from() -> ExpressionId {
    ExpressionId::new(WorkId::new("uscode/title_26"), "2025-07-18")
}

fn to() -> ExpressionId {
    ExpressionId::new(WorkId::new("uscode/title_26"), "2025-07-30")
}

fn real_annotations() -> Vec<ChangeAnnotation> {
    let json = std::fs::read_to_string(ANNOTATIONS).expect("the fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse as annotations")
}

#[test]
fn should_project_a_stored_annotation_into_one_link_per_path() {
    let annotations = real_annotations();
    let annotation = annotations.first().expect("the fixture should hold some");

    let links = Link::from_annotation(annotation, &from(), &to());

    assert_eq!(
        links.len(),
        annotation.paths.len(),
        "each path is its own statement, because each can be confirmed alone"
    );

    let link = links.first().expect("at least one link");
    assert_eq!(link.kind, LinkKind::new(LinkKind::AMENDED_BY));
    assert_eq!(link.kind.namespace(), "legislature");
    // The subject is the *change*, not the provision. A bare provision cannot
    // say when it was amended, so the expression pair had nowhere to live and
    // was silently dropped (#70, `docs/adr/0004`).
    assert_eq!(
        link.subject,
        Target::Change {
            work: from().work.clone(),
            path: annotation.paths[0].clone(),
            from_date: from().at.clone(),
            to_date: to().at.clone(),
        },
        "the subject is the change, which is the provision plus its two dates"
    );
}

/// An amendment is a legislature concept, so a link reaches it as an external
/// reference in that namespace. A reader that knows nothing of bills can still
/// report that the change was caused by something, and name it.
#[test]
fn should_reach_the_amendment_as_an_external_reference() {
    let annotations = real_annotations();
    let annotation = annotations.first().expect("the fixture should hold some");

    let links = Link::from_annotation(annotation, &from(), &to());
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
        for link in Link::from_annotation(annotation, &from(), &to()) {
            assert_eq!(
                link.provenance.verification,
                VerificationState::MachineSuggested
            );
            assert_eq!(link.provenance.source, annotation.metadata.annotator);
            assert_eq!(link.provenance.raw_score, annotation.metadata.confidence);
            assert_eq!(
                link.provenance
                    .evidence
                    .as_ref()
                    .and_then(|e| e.reasoning.clone()),
                annotation.metadata.reasoning,
                "the model's stated reasoning is one part of the evidence"
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
    for link in Link::from_annotation(&annotation, &from(), &to()) {
        assert_eq!(link.provenance.verification, VerificationState::Refuted);
    }

    annotation.metadata.status = AnnotationStatus::Disputed;
    for link in Link::from_annotation(&annotation, &from(), &to()) {
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

    let link = Link::from_annotation(annotation, &from(), &to())
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

/// Corroboration must be reproducible: that is the whole reason it may be
/// relied on when a model's self-reported score may not. This checks the claim
/// against 383 real scores from a production run, by recomputing precision from
/// the word counts the record carries.
#[test]
fn should_carry_a_reproducible_measurement_from_real_similarity_scores() {
    let json = std::fs::read_to_string("tests/test_data/processed/similarity_scores.json")
        .expect("the fixture should be readable");
    let scores: Vec<AmendmentSimilarity> =
        serde_json::from_str(&json).expect("the fixture should parse as similarities");

    assert!(!scores.is_empty(), "the fixture should hold real scores");

    for similarity in &scores {
        let corroboration = Corroboration::from(similarity);

        assert_eq!(corroboration.method, "precision_weighted_f1");
        assert_eq!(corroboration.score, similarity.score);

        let detail: std::collections::HashMap<&str, f32> = corroboration
            .detail
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();

        // Recompute precision from the counts the record carries. If this
        // holds, a receiver can check the figure instead of trusting it.
        if similarity.tree_diff_words > 0 {
            let recomputed = similarity.matched_words as f32 / similarity.tree_diff_words as f32;
            let reported = detail["precision"];
            assert!(
                (recomputed - reported).abs() < 1e-5,
                "precision should recompute from the counts: {recomputed} against {reported} \
                 for {}",
                similarity.tree_diff_path
            );
        }

        assert_eq!(detail["matched_words"], similarity.matched_words as f32);
        assert_eq!(detail["tree_diff_words"], similarity.tree_diff_words as f32);
    }
}

/// The fixture's annotations carry the model's reasoning, so the links built
/// from them carry it as evidence. A machine claim that cannot say why is
/// weaker than one that can, and this is the part of "why" that survived.
#[test]
fn should_carry_the_models_reasoning_as_evidence() {
    let annotations = real_annotations();
    let with_reasoning: Vec<&ChangeAnnotation> = annotations
        .iter()
        .filter(|a| a.metadata.reasoning.is_some())
        .collect();

    assert!(
        !with_reasoning.is_empty(),
        "the fixture should hold annotations the model explained"
    );

    for annotation in with_reasoning {
        for link in Link::from_annotation(annotation, &from(), &to()) {
            let reasoning = link
                .provenance
                .evidence
                .and_then(|e| e.reasoning)
                .expect("a link should carry the reasoning behind it");
            assert!(!reasoning.trim().is_empty());
        }
    }
}
