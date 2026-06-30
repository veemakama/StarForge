use starforge::utils::multisig_builder::{
    calculate_progress, common_templates, generate_proposal_signature, proposal_from_template,
    render_progress_bar, validate_signatures, Proposal,
};

#[test]
fn templates_create_proposals_with_metadata() {
    let templates = common_templates();
    assert!(templates.iter().any(|template| template.name == "escrow"));

    let proposal = proposal_from_template("escrow", "testnet".to_string()).unwrap();
    assert_eq!(proposal.threshold, 2);
    assert_eq!(proposal.signers, vec!["buyer", "seller", "arbiter"]);
    assert_eq!(proposal.network, "testnet");
    assert_eq!(proposal.metadata.template.as_deref(), Some("escrow"));
    assert_eq!(
        proposal.metadata.transaction_type.as_deref(),
        Some("escrow_release")
    );
}

#[test]
fn progress_tracks_valid_signatures_and_pending_signers() {
    let mut proposal = Proposal::new(
        2,
        vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
        "testnet".to_string(),
    );

    let signature = generate_proposal_signature("alice", &proposal).unwrap();
    proposal
        .add_signature_checked("alice".to_string(), signature)
        .unwrap();

    let progress = calculate_progress(&proposal);
    assert_eq!(progress.signed, 1);
    assert_eq!(progress.required, 2);
    assert_eq!(progress.percent, 50);
    assert!(!progress.ready);
    assert_eq!(progress.pending_signers, vec!["bob", "carol"]);

    let bar = render_progress_bar(&progress, 10);
    assert_eq!(bar, "[#####.....] 50% (1/2)");
}

#[test]
fn signature_validation_rejects_invalid_and_duplicate_signatures() {
    let mut proposal = Proposal::new(
        2,
        vec!["alice".to_string(), "bob".to_string()],
        "testnet".to_string(),
    );

    let alice_signature = generate_proposal_signature("alice", &proposal).unwrap();
    proposal
        .add_signature_checked("alice".to_string(), alice_signature)
        .unwrap();
    assert!(proposal
        .add_signature_checked("alice".to_string(), "duplicate".to_string())
        .is_err());

    proposal.add_signature("bob".to_string(), "not-a-valid-signature".to_string());

    let validation = validate_signatures(&proposal);
    assert_eq!(validation.valid_signatures, 1);
    assert_eq!(validation.invalid_signers, vec!["bob"]);
    assert_eq!(validation.missing_signers, vec!["bob"]);
    assert!(!validation.ready);
}

#[test]
fn validation_marks_ready_when_threshold_is_met() {
    let mut proposal = Proposal::new(
        2,
        vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
        "testnet".to_string(),
    );

    let alice_signature = generate_proposal_signature("alice", &proposal).unwrap();
    proposal
        .add_signature_checked("alice".to_string(), alice_signature)
        .unwrap();
    let bob_signature = generate_proposal_signature("bob", &proposal).unwrap();
    proposal
        .add_signature_checked("bob".to_string(), bob_signature)
        .unwrap();

    let validation = validate_signatures(&proposal);
    assert_eq!(validation.valid_signatures, 2);
    assert!(validation.ready);
    assert_eq!(calculate_progress(&proposal).percent, 100);
}
