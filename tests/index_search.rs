use agents_sesame::index::TantivyIndex;
use agents_sesame::query::{DateFilter, DateOp, Filter};
use agents_sesame::session::Session;
use chrono::NaiveDateTime;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────────

fn make_session(
    id: &str,
    agent: &str,
    title: &str,
    directory: &str,
    content: &str,
    mtime: f64,
    message_count: usize,
    yolo: bool,
) -> Session {
    let ts = mtime as i64;
    Session {
        id: id.to_string(),
        agent: agent.to_string(),
        title: title.to_string(),
        directory: directory.to_string(),
        content: content.to_string(),
        mtime,
        message_count,
        yolo,
        timestamp: chrono::DateTime::from_timestamp(ts, 0).unwrap().naive_utc(),
    }
}

fn new_index() -> (TempDir, TantivyIndex) {
    let tmp = TempDir::new().unwrap();
    let idx = TantivyIndex::new_with_path(tmp.path().to_path_buf());
    (tmp, idx)
}

/// Sort sessions by id for deterministic comparisons.
fn sorted_by_id(mut sessions: Vec<Session>) -> Vec<Session> {
    sessions.sort_by(|a, b| a.id.cmp(&b.id));
    sessions
}

// ── A. Index CRUD ───────────────────────────────────────────────────────

#[test]
fn test_add_and_get_all() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Fix bug in parser",
            "/home/user/project-a",
            "» Fix the parser\n  Done.",
            1700000000.0,
            2,
            false,
        ),
        make_session(
            "s2",
            "codex",
            "Add logging",
            "/home/user/project-b",
            "» Add logging\n  Added.",
            1700000100.0,
            3,
            false,
        ),
        make_session(
            "s3",
            "claude",
            "Refactor tests",
            "/home/user/project-a",
            "» Refactor\n  Refactored.",
            1700000200.0,
            4,
            true,
        ),
    ];
    idx.add_sessions(&sessions);

    let result = sorted_by_id(idx.get_all_sessions());
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, "s1");
    assert_eq!(result[0].agent, "claude");
    assert_eq!(result[0].title, "Fix bug in parser");
    assert_eq!(result[0].directory, "/home/user/project-a");
    assert_eq!(result[0].message_count, 2);
    assert!(!result[0].yolo);

    assert_eq!(result[1].id, "s2");
    assert_eq!(result[1].agent, "codex");

    assert_eq!(result[2].id, "s3");
    assert!(result[2].yolo);
    assert_eq!(result[2].message_count, 4);
}

#[test]
fn test_update_session() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "s1",
        "claude",
        "Original title",
        "/tmp",
        "content",
        1700000000.0,
        2,
        false,
    );
    idx.add_sessions(&[s]);

    let updated = make_session(
        "s1",
        "claude",
        "Updated title",
        "/tmp",
        "new content",
        1700000999.0,
        5,
        true,
    );
    idx.update_sessions(&[updated]);

    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "Updated title");
    assert_eq!(result[0].message_count, 5);
    assert!(result[0].yolo);
}

#[test]
fn test_delete_session() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Session 1",
            "/tmp",
            "c1",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "codex",
            "Session 2",
            "/tmp",
            "c2",
            1700000100.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);
    assert_eq!(idx.get_all_sessions().len(), 2);

    idx.delete_sessions(&["s1".to_string()]);

    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "s2");
}

#[test]
fn test_batch_update() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "To delete",
            "/tmp",
            "c1",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "codex",
            "To keep",
            "/tmp",
            "c2",
            1700000100.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let new_session = make_session(
        "s3",
        "gemini",
        "Brand new",
        "/tmp",
        "c3",
        1700000200.0,
        1,
        false,
    );
    let updated_s2 = make_session(
        "s2",
        "codex",
        "Updated keep",
        "/tmp",
        "c2 updated",
        1700000300.0,
        2,
        false,
    );

    idx.batch_update(&["s1".to_string()], &[new_session, updated_s2]);

    let result = sorted_by_id(idx.get_all_sessions());
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "s2");
    assert_eq!(result[0].title, "Updated keep");
    assert_eq!(result[1].id, "s3");
    assert_eq!(result[1].agent, "gemini");
}

#[test]
fn test_clear() {
    let (_tmp, idx) = new_index();

    let s = make_session("s1", "claude", "Title", "/tmp", "c", 1700000000.0, 1, false);
    idx.add_sessions(&[s]);
    assert_eq!(idx.get_all_sessions().len(), 1);

    idx.clear();
    // Need to reload after clear to see the change
    assert_eq!(idx.get_all_sessions().len(), 0);
}

// ── B. Fast fields correctness ──────────────────────────────────────────

#[test]
fn test_fast_fields_no_content() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "s1",
        "claude",
        "Title",
        "/tmp",
        "This is the real content",
        1700000000.0,
        5,
        false,
    );
    idx.add_sessions(&[s]);

    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    // Fast fields path returns empty content
    assert!(
        result[0].content.is_empty(),
        "get_all_sessions() should return empty content, got: '{}'",
        result[0].content
    );
    // But all metadata is present
    assert_eq!(result[0].id, "s1");
    assert_eq!(result[0].title, "Title");
    assert_eq!(result[0].agent, "claude");
    assert_eq!(result[0].directory, "/tmp");
    assert_eq!(result[0].message_count, 5);
}

#[test]
fn test_get_session_content() {
    let (_tmp, idx) = new_index();

    let content = "» Hello, can you help me?\n  Of course! What do you need?";
    let s = make_session(
        "s1",
        "claude",
        "Help request",
        "/tmp",
        content,
        1700000000.0,
        2,
        false,
    );
    idx.add_sessions(&[s]);

    let loaded = idx.get_session_content("s1");
    assert_eq!(loaded.as_deref(), Some(content));
}

#[test]
fn test_get_session_content_not_found() {
    let (_tmp, idx) = new_index();

    assert!(idx.get_session_content("nonexistent").is_none());
}

#[test]
fn test_get_known_sessions() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session("s1", "claude", "T1", "/tmp", "c1", 1700000000.0, 1, false),
        make_session("s2", "codex", "T2", "/tmp", "c2", 1700000100.0, 2, false),
        make_session("s3", "claude", "T3", "/home", "c3", 1700000200.0, 3, true),
    ];
    idx.add_sessions(&sessions);

    let known = idx.get_known_sessions();
    assert_eq!(known.len(), 3);

    let (mtime1, agent1) = known.get("s1").unwrap();
    assert_eq!(*mtime1, 1700000000.0);
    assert_eq!(agent1, "claude");

    let (mtime2, agent2) = known.get("s2").unwrap();
    assert_eq!(*mtime2, 1700000100.0);
    assert_eq!(agent2, "codex");

    let (mtime3, agent3) = known.get("s3").unwrap();
    assert_eq!(*mtime3, 1700000200.0);
    assert_eq!(agent3, "claude");
}

// ── C. Search correctness ───────────────────────────────────────────────

#[test]
fn test_search_text_match() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Fix niri window rules",
            "/tmp",
            "» Fix niri config\n  Done.",
            1700000000.0,
            2,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "Add logging to server",
            "/tmp",
            "» Add logging\n  Added.",
            1700000100.0,
            2,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let results = idx.search("niri", &[], None, None, None, 10, 6);
    assert!(
        !results.is_empty(),
        "Search for 'niri' should find at least one result"
    );
    assert!(
        results.iter().any(|(id, _)| id == "s1"),
        "Should find s1 (contains 'niri')"
    );
    // s2 should NOT match "niri"
    assert!(
        !results.iter().any(|(id, _)| id == "s2"),
        "Should not find s2 (no 'niri')"
    );
}

#[test]
fn test_search_fuzzy() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "s1",
        "claude",
        "Configure niri compositor",
        "/tmp",
        "niri setup",
        1700000000.0,
        1,
        false,
    );
    idx.add_sessions(&[s]);

    // "nri" is 3 chars (< 6) and not a substring of "niri", so no match
    let results = idx.search("nri", &[], None, None, None, 10, 6);
    assert!(
        results.is_empty(),
        "Short non-substring 'nri' should not fuzzy-match 'niri'"
    );

    // But "nir" IS a substring of "niri", so substring regex matches
    let results = idx.search("nir", &[], None, None, None, 10, 6);
    assert!(!results.is_empty(), "Substring 'nir' should match 'niri'");
    assert_eq!(results[0].0, "s1");

    // Fuzzy only kicks in for terms >= 6 chars: "compostor" → "compositor" (distance 1)
    let results = idx.search("compostor", &[], None, None, None, 10, 6);
    assert!(
        !results.is_empty(),
        "Fuzzy search for 'compostor' should find 'compositor'"
    );
    assert_eq!(results[0].0, "s1");
}

#[test]
fn test_search_agent_filter() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Claude session",
            "/tmp",
            "claude content",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "codex",
            "Codex session",
            "/tmp",
            "codex content",
            1700000100.0,
            1,
            false,
        ),
        make_session(
            "s3",
            "claude",
            "Another claude",
            "/tmp",
            "more claude",
            1700000200.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let agent_filter = Filter {
        include: vec!["claude".to_string()],
        exclude: vec![],
    };
    let results = idx.search("", &[], Some(&agent_filter), None, None, 10, 6);
    assert_eq!(results.len(), 2, "Should find exactly 2 claude sessions");
    let ids: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"s1"));
    assert!(ids.contains(&"s3"));
    assert!(!ids.contains(&"s2"));
}

#[test]
fn test_search_directory_filter() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Session A",
            "/home/user/rust-resume",
            "c1",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "Session B",
            "/home/user/other-project",
            "c2",
            1700000100.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let dir_filter = Filter {
        include: vec!["rust-resume".to_string()],
        exclude: vec![],
    };
    let results = idx.search("", &[], None, Some(&dir_filter), None, 10, 6);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "s1");
}

#[test]
fn test_search_date_filter() {
    let (_tmp, idx) = new_index();

    // s1: old (2023-11-14), s2: recent (2025-01-01)
    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Old session",
            "/tmp",
            "c1",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "New session",
            "/tmp",
            "c2",
            1735689600.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    // Filter: newer than 2024-06-01 (timestamp 1717200000)
    let cutoff = NaiveDateTime::parse_from_str("2024-06-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let date_filter = DateFilter {
        op: DateOp::LessThan, // "newer than" = LessThan days ago
        cutoff,
        negated: false,
    };
    let results = idx.search("", &[], None, None, Some(&date_filter), 10, 6);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "s2");
}

#[test]
fn test_search_combined_filters() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Fix niri window rules",
            "/tmp",
            "niri config",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "codex",
            "Fix niri in codex",
            "/tmp",
            "niri codex",
            1700000100.0,
            1,
            false,
        ),
        make_session(
            "s3",
            "claude",
            "Add logging",
            "/tmp",
            "logging stuff",
            1700000200.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let agent_filter = Filter {
        include: vec!["claude".to_string()],
        exclude: vec![],
    };
    let results = idx.search("niri", &[], Some(&agent_filter), None, None, 10, 6);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "s1");
}

#[test]
fn test_search_empty_query() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "First",
            "/tmp",
            "c1",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "Second",
            "/tmp",
            "c2",
            1700000100.0,
            1,
            false,
        ),
        make_session(
            "s3",
            "claude",
            "Third",
            "/tmp",
            "c3",
            1700000200.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let results = idx.search("", &[], None, None, None, 10, 6);
    assert_eq!(results.len(), 3);
    // Empty query sorts by timestamp descending
    assert_eq!(results[0].0, "s3"); // newest first
    assert_eq!(results[1].0, "s2");
    assert_eq!(results[2].0, "s1");
}

#[test]
fn test_search_no_results() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "s1",
        "claude",
        "Hello world",
        "/tmp",
        "greeting",
        1700000000.0,
        1,
        false,
    );
    idx.add_sessions(&[s]);

    let results = idx.search("zzzznonexistentterm", &[], None, None, None, 10, 6);
    assert!(results.is_empty());
}

#[test]
fn test_search_limit() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "Session one",
            "/tmp",
            "content one",
            1700000000.0,
            1,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "Session two",
            "/tmp",
            "content two",
            1700000100.0,
            1,
            false,
        ),
        make_session(
            "s3",
            "claude",
            "Session three",
            "/tmp",
            "content three",
            1700000200.0,
            1,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let results = idx.search("", &[], None, None, None, 2, 6);
    assert_eq!(results.len(), 2);
}

// ── D. Edge cases ───────────────────────────────────────────────────────

#[test]
fn test_empty_index() {
    let (_tmp, idx) = new_index();

    assert!(idx.get_all_sessions().is_empty());
    assert!(idx.get_known_sessions().is_empty());
    assert!(
        idx.search("anything", &[], None, None, None, 10, 6)
            .is_empty()
    );
    assert!(idx.get_session_content("nonexistent").is_none());
}

#[test]
fn test_special_chars_in_title() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "s1",
        "claude",
        r#"Fix "parser" (v2) [urgent] & deploy"#,
        "/home/user/project",
        "content with special chars: <>&\"'",
        1700000000.0,
        1,
        false,
    );
    idx.add_sessions(&[s]);

    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, r#"Fix "parser" (v2) [urgent] & deploy"#);

    let content = idx.get_session_content("s1").unwrap();
    assert_eq!(content, "content with special chars: <>&\"'");
}

#[test]
fn test_cjk_search() {
    let (_tmp, idx) = new_index();

    let sessions = vec![
        make_session(
            "s1",
            "claude",
            "修复 niri 窗口规则",
            "/tmp",
            "» 修复窗口\n  完成",
            1700000000.0,
            2,
            false,
        ),
        make_session(
            "s2",
            "claude",
            "Add English feature",
            "/tmp",
            "» English\n  Done",
            1700000100.0,
            2,
            false,
        ),
    ];
    idx.add_sessions(&sessions);

    let results = idx.search("窗口", &[], None, None, None, 10, 6);
    assert!(!results.is_empty(), "Should find CJK text '窗口'");
    assert!(results.iter().any(|(id, _)| id == "s1"));
}

#[test]
fn test_duplicate_id_update() {
    let (_tmp, idx) = new_index();

    let s1 = make_session(
        "dup-id",
        "claude",
        "Version 1",
        "/tmp",
        "old content",
        1700000000.0,
        1,
        false,
    );
    idx.add_sessions(&[s1]);

    let s2 = make_session(
        "dup-id",
        "claude",
        "Version 2",
        "/tmp",
        "new content",
        1700000999.0,
        5,
        true,
    );
    idx.update_sessions(&[s2]);

    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "Version 2");
    assert_eq!(result[0].message_count, 5);
    assert!(result[0].yolo);

    let content = idx.get_session_content("dup-id").unwrap();
    assert_eq!(content, "new content");
}

// ── E. Session field roundtrip ──────────────────────────────────────────

#[test]
fn test_all_fields_roundtrip() {
    let (_tmp, idx) = new_index();

    let s = make_session(
        "abc-123-def",
        "opencode",
        "Complex session with many messages",
        "/home/user/deep/nested/project",
        "» First message\n  Response 1\n» Second message\n  Response 2",
        1700000500.5,
        42,
        true,
    );
    idx.add_sessions(&[s]);

    // Metadata via fast fields
    let result = idx.get_all_sessions();
    assert_eq!(result.len(), 1);
    let r = &result[0];
    assert_eq!(r.id, "abc-123-def");
    assert_eq!(r.agent, "opencode");
    assert_eq!(r.title, "Complex session with many messages");
    assert_eq!(r.directory, "/home/user/deep/nested/project");
    assert_eq!(r.message_count, 42);
    assert!(r.yolo);
    // mtime preserved (f64 comparison)
    assert!((r.mtime - 1700000500.5).abs() < 0.01);
    // Content is empty from fast fields
    assert!(r.content.is_empty());

    // Content via lazy load
    let content = idx.get_session_content("abc-123-def").unwrap();
    assert_eq!(
        content,
        "» First message\n  Response 1\n» Second message\n  Response 2"
    );

    // Known sessions map
    let known = idx.get_known_sessions();
    let (mtime, agent) = known.get("abc-123-def").unwrap();
    assert!((mtime - 1700000500.5).abs() < 0.01);
    assert_eq!(agent, "opencode");
}
