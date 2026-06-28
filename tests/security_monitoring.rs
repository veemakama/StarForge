use starforge::utils::security::{
    anomaly::AnomalyDetector, evaluate_event, default_rules, threat_intel::ThreatFeed,
    IncidentStore,
};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn security_event_rules_detect_admin_changes() {
    let rules = default_rules();
    let events = evaluate_event(
        &rules,
        "CABC123",
        100,
        "evt-1",
        &["admin".into()],
        &json!({"action": "set_admin", "new_admin": "GAAA"}),
    );
    assert!(!events.is_empty());
    assert_eq!(events[0].rule_id, "admin-change");
}

#[test]
fn anomaly_detector_flags_rate_spike() {
    let mut detector = AnomalyDetector::new("CABC123");
    for _ in 0..20 {
        detector.record_event(None);
    }
    let finding = detector.record_event(None);
    assert!(finding.is_some());
}

#[test]
fn threat_intel_matches_known_patterns() {
    let feed = ThreatFeed::default_feed();
    let matches = feed.match_event("possible drain attack detected");
    assert!(!matches.is_empty());
}

#[test]
fn incident_store_create_and_list() {
    let home = TempDir::new().unwrap();
    std::env::set_var("HOME", home.path());

    let incident = IncidentStore::create(
        "CABC123",
        "high",
        "Test incident",
        "Automated test incident",
    )
    .unwrap();
    let all = IncidentStore::load_all().unwrap();
    assert!(all.iter().any(|i| i.id == incident.id));
}
