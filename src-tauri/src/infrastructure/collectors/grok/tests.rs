use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use tempfile::TempDir;

use super::{GrokSessionIndex, GrokSessionSummary, UnifiedLogReader};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/collectors/grok")
        .join(relative)
}

#[test]
fn reads_single_session_inference_rows_from_fixture() {
    let (rows, summary) =
        UnifiedLogReader::read_from_path(&fixture_path("unified-log/single-session.jsonl"))
            .expect("read unified log");

    assert_eq!(summary.lines_read, 2);
    assert_eq!(summary.inference_rows_accepted, 2);
    assert_eq!(summary.lines_skipped, 0);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].session_id, "019f0000-0000-7000-8000-000000000001");
    assert_eq!(rows[0].loop_index, 1);
    assert_eq!(rows[0].prompt_tokens, 12_000);
    assert_eq!(rows[0].cached_prompt_tokens, 8_000);
    assert_eq!(rows[0].completion_tokens, 240);
    assert_eq!(
        rows[0].observed_at,
        Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0)
            .single()
            .expect("timestamp")
    );
}

#[test]
fn reads_multi_session_inference_rows_from_fixture() {
    let (rows, summary) =
        UnifiedLogReader::read_from_path(&fixture_path("unified-log/multi-session.jsonl"))
            .expect("read unified log");

    assert_eq!(summary.inference_rows_accepted, 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].session_id, "019f0000-0000-7000-8000-000000000001");
    assert_eq!(rows[1].session_id, "019f0000-0000-7000-8000-000000000002");
}

#[test]
fn skips_malformed_and_non_inference_lines() {
    let (rows, summary) =
        UnifiedLogReader::read_from_path(&fixture_path("unified-log/malformed-lines.jsonl"))
            .expect("read unified log");

    assert_eq!(summary.lines_read, 3);
    assert_eq!(summary.inference_rows_accepted, 1);
    assert_eq!(summary.lines_skipped, 2);
    assert_eq!(rows.len(), 1);
}

#[test]
fn skips_rows_with_missing_session_id() {
    let (rows, summary) =
        UnifiedLogReader::read_from_path(&fixture_path("unified-log/missing-sid.jsonl"))
            .expect("read unified log");

    assert_eq!(summary.inference_rows_accepted, 0);
    assert!(rows.is_empty());
}

#[test]
fn skips_rows_with_invalid_token_counts() {
    let (rows, summary) =
        UnifiedLogReader::read_from_path(&fixture_path("unified-log/invalid-tokens.jsonl"))
            .expect("read unified log");

    assert_eq!(summary.inference_rows_accepted, 0);
    assert!(rows.is_empty());
}

#[test]
fn scans_session_summaries_from_fixture_layout() {
    let temp = TempDir::new().expect("temp dir");
    let grok_home = temp.path();
    let session_dir = grok_home
        .join("sessions")
        .join("%2Ftmp%2Fgrok-fixture-project")
        .join("019f0000-0000-7000-8000-000000000001");
    fs::create_dir_all(&session_dir).expect("session dir");
    fs::copy(
        fixture_path("sessions/summary-valid.json"),
        session_dir.join("summary.json"),
    )
    .expect("copy summary");

    let summaries = GrokSessionIndex::from_grok_home(grok_home)
        .scan()
        .expect("scan summaries");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0],
        GrokSessionSummary {
            session_id: "019f0000-0000-7000-8000-000000000001".to_owned(),
            cwd: "/tmp/grok-fixture-project".to_owned(),
            current_model_id: Some("grok-composer-2.5-fast".to_owned()),
            agent_name: Some("cursor".to_owned()),
            git_root_dir: Some("/tmp/grok-fixture-project/".to_owned()),
            head_branch: Some("main".to_owned()),
            created_at: Some(
                Utc.with_ymd_and_hms(2026, 7, 6, 10, 0, 0)
                    .single()
                    .expect("created_at"),
            ),
            updated_at: Some(
                Utc.with_ymd_and_hms(2026, 7, 6, 10, 5, 0)
                    .single()
                    .expect("updated_at"),
            ),
        }
    );
}

#[test]
fn skips_incompatible_session_summaries_during_scan() {
    let temp = TempDir::new().expect("temp dir");
    let session_dir = temp
        .path()
        .join("sessions")
        .join("encoded-cwd")
        .join("session-id");
    fs::create_dir_all(&session_dir).expect("session dir");
    fs::write(session_dir.join("summary.json"), r#"{"num_messages": 0}"#).expect("summary");

    let summaries = GrokSessionIndex::from_grok_home(temp.path())
        .scan()
        .expect("scan summaries");

    assert!(summaries.is_empty());
}
