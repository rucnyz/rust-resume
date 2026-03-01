use std::collections::HashMap;

use agents_sesame::search::SessionSearch;
use agents_sesame::tui::app::App;
use agents_sesame::tui::keybindings::KeyBindings;
use agents_sesame::tui::theme::Theme;
use criterion::{Criterion, criterion_group, criterion_main};

fn setup_app() -> App {
    let keybindings = KeyBindings::load(&HashMap::new());
    let theme = Theme::from_config(&None);
    let mut app = App::new(false, keybindings, theme);

    // Load real sessions from the user's index
    let mut search = SessionSearch::new();
    let sessions = search.get_all_sessions(false, None);
    app.sessions = sessions;
    app.search_engine = search;
    app.update_agent_counts();
    app
}

fn bench_apply_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_filter");

    // Benchmark 1: Empty query (no search, just clone+filter)
    group.bench_function("empty_query_global", |b| {
        let mut app = setup_app();
        app.query.clear();
        app.search_dirty = true;
        b.iter(|| {
            app.search_dirty = true;
            app.apply_filter();
        });
    });

    // Benchmark 2: Empty query with agent filter
    group.bench_function("empty_query_agent_claude", |b| {
        let mut app = setup_app();
        app.query.clear();
        app.agent_filter = Some("claude".to_string());
        b.iter(|| {
            app.search_dirty = true;
            app.apply_filter();
        });
    });

    // Benchmark 3: Text query (goes through Tantivy)
    group.bench_function("text_query_niri", |b| {
        let mut app = setup_app();
        app.query = "niri".to_string();
        app.cursor_pos = 4;
        b.iter(|| {
            app.search_dirty = true;
            app.apply_filter();
        });
    });

    // Benchmark 4: Text query with agent filter
    group.bench_function("text_query_agent", |b| {
        let mut app = setup_app();
        app.query = "niri".to_string();
        app.cursor_pos = 4;
        app.agent_filter = Some("claude".to_string());
        b.iter(|| {
            app.search_dirty = true;
            app.apply_filter();
        });
    });

    // Benchmark 5: Empty query with Project scope
    group.bench_function("empty_query_project_scope", |b| {
        let mut app = setup_app();
        app.query.clear();
        app.directory_scope = agents_sesame::tui::app::DirectoryScope::Project;
        app.directory_filter = Some("/home/yuzhounie/src/rust-resume".to_string());
        b.iter(|| {
            app.search_dirty = true;
            app.apply_filter();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_apply_filter);
criterion_main!(benches);
