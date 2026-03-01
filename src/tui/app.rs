use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tui_scrollbar::{ScrollBar, ScrollBarInteraction, ScrollCommand};

use crate::search::{LoadingMsg, SessionSearch};
use crate::session::Session;

use super::icons::IconManager;

use super::results_list::ResultsState;

pub struct App {
    pub query: String,
    pub cursor_pos: usize,
    pub sessions: Vec<Session>,
    pub filtered: Vec<Session>,
    pub results_state: ResultsState,
    pub preview_scroll: u16,
    pub show_preview: bool,
    pub preview_bottom: bool, // true = bottom, false = right
    pub agent_filter: Option<String>,
    pub agent_counts: HashMap<String, usize>,
    pub total_count: usize,
    pub sort_by_time: bool,
    pub directory_filter: Option<String>,
    pub search_engine: SessionSearch,
    pub should_quit: bool,
    pub resume_session: Option<Session>,
    pub yolo: bool,
    pub status_msg: Option<String>,
    pub search_dirty: bool,
    pub last_search_time: Option<Duration>,
    /// Area where results list is rendered (for mouse click mapping).
    pub results_area: ratatui::layout::Rect,
    /// Area where preview is rendered (for mouse scroll).
    pub preview_area: ratatui::layout::Rect,
    /// Icon manager for rendering agent icons.
    pub icons: Option<IconManager>,
    /// Whether mouse capture is enabled (toggle with Ctrl+M).
    pub mouse_captured: bool,
    /// Set when mouse capture state changed, to apply in run_loop.
    pub mouse_toggle_pending: bool,
    /// Total physical lines in preview content (set during render).
    pub preview_total_lines: usize,
    /// tui-scrollbar interaction state for results list.
    pub results_sb_interaction: ScrollBarInteraction,
    /// tui-scrollbar interaction state for preview.
    pub preview_sb_interaction: ScrollBarInteraction,
    /// Current results scrollbar widget (set during render for mouse events).
    pub results_scrollbar: Option<ScrollBar>,
    /// Current preview scrollbar widget (set during render for mouse events).
    pub preview_scrollbar: Option<ScrollBar>,
    /// Scrollbar area for results (set during render).
    pub results_sb_area: Rect,
    /// Scrollbar area for preview (set during render).
    pub preview_sb_area: Rect,
    /// Whether preview should auto-scroll to first match on next render.
    pub preview_auto_scroll: bool,
    /// Whether sessions are still loading in background.
    pub loading: bool,
    /// Receiver for progressive background session loading.
    pub loading_rx: Option<std::sync::mpsc::Receiver<LoadingMsg>>,
}

impl App {
    pub fn new(yolo: bool) -> Self {
        Self {
            query: String::new(),
            cursor_pos: 0,
            sessions: Vec::new(),
            filtered: Vec::new(),
            results_state: ResultsState::default(),
            preview_scroll: 0,
            show_preview: true,
            preview_bottom: true,
            agent_filter: None,
            agent_counts: HashMap::new(),
            total_count: 0,
            sort_by_time: false,
            directory_filter: None,
            search_engine: SessionSearch::new(),
            should_quit: false,
            resume_session: None,
            yolo,
            status_msg: None,
            search_dirty: false,
            last_search_time: None,
            results_area: ratatui::layout::Rect::default(),
            preview_area: ratatui::layout::Rect::default(),
            icons: None,
            mouse_captured: true,
            mouse_toggle_pending: false,
            preview_total_lines: 0,
            results_sb_interaction: ScrollBarInteraction::new(),
            preview_sb_interaction: ScrollBarInteraction::new(),
            results_scrollbar: None,
            preview_scrollbar: None,
            results_sb_area: Rect::default(),
            preview_sb_area: Rect::default(),
            preview_auto_scroll: false,
            loading: false,
            loading_rx: None,
        }
    }

    /// Start loading sessions in a background thread (progressive).
    pub fn start_loading(&mut self) {
        self.loading = true;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut engine = SessionSearch::new();
            engine.load_progressive(false, &tx);
            let _ = tx.send(LoadingMsg::Done(Box::new(engine)));
        });
        self.loading_rx = Some(rx);
    }

    /// Check for progressive loading updates (non-blocking, drains all available).
    pub fn check_loading(&mut self) {
        if self.loading_rx.is_none() {
            return;
        }

        let mut got_update = false;
        let mut done_engine: Option<SessionSearch> = None;

        // Drain all available messages
        loop {
            let msg = self.loading_rx.as_ref().unwrap().try_recv();
            match msg {
                Ok(LoadingMsg::Sessions(sessions)) => {
                    self.sessions = sessions;
                    self.total_count = self.sessions.len();
                    self.update_agent_counts();
                    got_update = true;
                }
                Ok(LoadingMsg::Done(engine)) => {
                    done_engine = Some(*engine);
                    got_update = true;
                    break;
                }
                Err(_) => break,
            }
        }

        if let Some(engine) = done_engine {
            self.search_engine = engine;
            self.loading = false;
            self.loading_rx = None;
        }

        if got_update {
            self.apply_filter();
        }
    }

    fn update_agent_counts(&mut self) {
        self.agent_counts.clear();
        for s in &self.sessions {
            *self.agent_counts.entry(s.agent.clone()).or_insert(0) += 1;
        }
    }

    pub fn apply_filter(&mut self) {
        let start = Instant::now();

        if self.query.is_empty() {
            self.filtered = self.sessions.clone();
            if let Some(ref agent) = self.agent_filter {
                self.filtered.retain(|s| &s.agent == agent);
            }
            if let Some(ref dir) = self.directory_filter {
                let lower = dir.to_lowercase();
                self.filtered
                    .retain(|s| s.directory.to_lowercase().contains(&lower));
            }
        } else {
            self.filtered = self.search_engine.search(
                &self.query,
                self.agent_filter.as_deref(),
                self.directory_filter.as_deref(),
                200,
                self.sort_by_time,
            );
        }

        self.last_search_time = Some(start.elapsed());
        self.results_state.select_first();
        self.preview_scroll = 0;
        self.preview_auto_scroll = true;
        self.search_dirty = false;
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.filtered.get(self.results_state.selected)
    }

    pub fn handle_events(&mut self) -> std::io::Result<()> {
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => self.handle_key(key),
                Event::Mouse(mouse) => self.handle_mouse(mouse),
                _ => {}
            }
        }

        if self.search_dirty {
            self.apply_filter();
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            // Quit
            KeyCode::Esc => {
                self.should_quit = true;
            }

            // Ctrl+key combinations
            KeyCode::Char(c) if ctrl => match c {
                'c' => self.should_quit = true,
                '`' => self.show_preview = !self.show_preview,
                'p' => self.preview_bottom = !self.preview_bottom,
                's' => {
                    self.sort_by_time = !self.sort_by_time;
                    self.search_dirty = true;
                }
                'w' => {
                    self.delete_word_backward();
                    self.search_dirty = true;
                }
                'u' => {
                    self.query.clear();
                    self.cursor_pos = 0;
                    self.search_dirty = true;
                }
                'e' => {
                    self.mouse_captured = !self.mouse_captured;
                    self.mouse_toggle_pending = true;
                }
                _ => {}
            },

            // Resume session
            KeyCode::Enter => {
                if let Some(session) = self.selected_session().cloned() {
                    self.resume_session = Some(session);
                    self.should_quit = true;
                }
            }

            // Navigation
            KeyCode::Down => {
                self.results_state.select_next(self.filtered.len());
                self.preview_scroll = 0;
                self.preview_auto_scroll = true;
            }
            KeyCode::Up => {
                self.results_state.select_prev();
                self.preview_scroll = 0;
                self.preview_auto_scroll = true;
            }
            KeyCode::PageDown => {
                self.results_state.page_down(10, self.filtered.len());
                self.preview_scroll = 0;
                self.preview_auto_scroll = true;
            }
            KeyCode::PageUp => {
                self.results_state.page_up(10);
                self.preview_scroll = 0;
                self.preview_auto_scroll = true;
            }

            // Tab: cycle agent filter
            KeyCode::Tab => {
                self.cycle_agent_filter();
                self.search_dirty = true;
            }
            KeyCode::BackTab if shift => {
                self.cycle_agent_filter_back();
                self.search_dirty = true;
            }

            // Ctrl+Backspace: delete word backward (same as Ctrl+W)
            KeyCode::Backspace if ctrl => {
                self.delete_word_backward();
                self.search_dirty = true;
            }

            // Search input
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let prev = self.query[..self.cursor_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.query.drain(prev..self.cursor_pos);
                    self.cursor_pos = prev;
                    self.search_dirty = true;
                }
            }
            KeyCode::Left if ctrl => {
                self.cursor_pos = self.word_boundary_left();
            }
            KeyCode::Right if ctrl => {
                self.cursor_pos = self.word_boundary_right();
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    let prev = self.query[..self.cursor_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.cursor_pos = prev;
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.query.len() {
                    let next = self.query[self.cursor_pos..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor_pos + i)
                        .unwrap_or(self.query.len());
                    self.cursor_pos = next;
                }
            }
            KeyCode::Home => self.cursor_pos = 0,
            KeyCode::End => self.cursor_pos = self.query.len(),

            // Typing
            KeyCode::Char(c) => {
                self.query.insert(self.cursor_pos, c);
                self.cursor_pos += c.len_utf8();
                self.search_dirty = true;
            }

            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        // Delegate to tui-scrollbar for scrollbar interactions (drag, click, wheel)
        let handled = self.handle_scrollbar_mouse(mouse);
        if handled {
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                let area = self.results_area;
                // Check if click is inside results area (skip border + header = +2 rows)
                let content_y = area.y + 2; // 1 border + 1 header
                let content_bottom = area.y + area.height.saturating_sub(1); // bottom border
                if mouse.column >= area.x
                    && mouse.column < area.x + area.width
                    && mouse.row >= content_y
                    && mouse.row < content_bottom
                {
                    let row_in_view = (mouse.row - content_y) as usize;
                    let new_selected = self.results_state.offset + row_in_view;
                    if new_selected < self.filtered.len() {
                        self.results_state.selected = new_selected;
                        self.preview_scroll = 0;
                        self.preview_auto_scroll = true;
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.is_in_area(mouse.column, mouse.row, self.preview_area) {
                    self.preview_scroll = self.preview_scroll.saturating_add(3);
                } else {
                    self.results_state.select_next(self.filtered.len());
                    self.preview_scroll = 0;
                    self.preview_auto_scroll = true;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.is_in_area(mouse.column, mouse.row, self.preview_area) {
                    self.preview_scroll = self.preview_scroll.saturating_sub(3);
                } else {
                    self.results_state.select_prev();
                    self.preview_scroll = 0;
                    self.preview_auto_scroll = true;
                }
            }
            _ => {}
        }
    }

    /// Delegate mouse events to tui-scrollbar widgets. Returns true if handled.
    fn handle_scrollbar_mouse(&mut self, mouse: MouseEvent) -> bool {
        // Try results scrollbar
        if let Some(ref sb) = self.results_scrollbar.clone() {
            let area = self.results_sb_area;
            if let Some(ScrollCommand::SetOffset(offset)) =
                sb.handle_mouse_event(area, mouse, &mut self.results_sb_interaction)
            {
                let visible_rows = self.results_area.height.saturating_sub(3) as usize;
                let max_offset = self.filtered.len().saturating_sub(visible_rows);
                let new_offset = offset.min(max_offset);
                self.results_state.offset = new_offset;
                self.results_state.selected =
                    self.results_state.selected.clamp(new_offset, new_offset + visible_rows.saturating_sub(1));
                self.preview_scroll = 0;
                self.preview_auto_scroll = true;
                return true;
            }
        }

        // Try preview scrollbar
        if let Some(ref sb) = self.preview_scrollbar.clone() {
            let area = self.preview_sb_area;
            if let Some(ScrollCommand::SetOffset(offset)) =
                sb.handle_mouse_event(area, mouse, &mut self.preview_sb_interaction)
            {
                self.preview_scroll = offset as u16;
                return true;
            }
        }

        false
    }

    fn is_in_area(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> bool {
        x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
    }

    fn word_boundary_left(&self) -> usize {
        if self.cursor_pos == 0 {
            return 0;
        }
        let text = &self.query[..self.cursor_pos];
        // Skip trailing whitespace, then skip word chars
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            return 0;
        }
        trimmed
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn word_boundary_right(&self) -> usize {
        if self.cursor_pos >= self.query.len() {
            return self.query.len();
        }
        let text = &self.query[self.cursor_pos..];
        // Skip leading whitespace, then skip word chars
        let after_space = text.trim_start();
        let space_len = text.len() - after_space.len();
        let word_end = after_space
            .find(|c: char| c.is_whitespace())
            .unwrap_or(after_space.len());
        self.cursor_pos + space_len + word_end
    }

    fn delete_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let text = &self.query[..self.cursor_pos];
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            self.query.drain(..self.cursor_pos);
            self.cursor_pos = 0;
            return;
        }
        let boundary = trimmed
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        self.query.drain(boundary..self.cursor_pos);
        self.cursor_pos = boundary;
    }

    fn cycle_agent_filter(&mut self) {
        let agents = self.available_agents();
        match &self.agent_filter {
            None => {
                if let Some(first) = agents.first() {
                    self.agent_filter = Some(first.clone());
                }
            }
            Some(current) => {
                let pos = agents.iter().position(|a| a == current);
                match pos {
                    Some(i) if i + 1 < agents.len() => {
                        self.agent_filter = Some(agents[i + 1].clone());
                    }
                    _ => self.agent_filter = None,
                }
            }
        }
    }

    fn cycle_agent_filter_back(&mut self) {
        let agents = self.available_agents();
        match &self.agent_filter {
            None => {
                if let Some(last) = agents.last() {
                    self.agent_filter = Some(last.clone());
                }
            }
            Some(current) => {
                let pos = agents.iter().position(|a| a == current);
                match pos {
                    Some(0) | None => self.agent_filter = None,
                    Some(i) => self.agent_filter = Some(agents[i - 1].clone()),
                }
            }
        }
    }

    fn available_agents(&self) -> Vec<String> {
        let mut agents: Vec<String> = self
            .agent_counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(name, _)| name.clone())
            .collect();
        agents.sort();
        agents
    }
}
