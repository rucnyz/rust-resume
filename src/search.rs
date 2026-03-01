use std::collections::HashMap;

use crate::adapter::AgentAdapter;
use crate::adapters::ClaudeAdapter;
use crate::index::TantivyIndex;
use crate::query::{parse_query, Filter};
use crate::session::Session;

pub struct SessionSearch {
    adapters: Vec<Box<dyn AgentAdapter>>,
    sessions_by_id: HashMap<String, Session>,
    index: TantivyIndex,
}

impl SessionSearch {
    pub fn new() -> Self {
        Self {
            adapters: vec![Box::new(ClaudeAdapter::new())],
            sessions_by_id: HashMap::new(),
            index: TantivyIndex::new(),
        }
    }

    /// Get all sessions, using incremental updates.
    pub fn get_all_sessions(&mut self, force_refresh: bool) -> Vec<Session> {
        let known = if force_refresh {
            HashMap::new()
        } else {
            self.index.get_known_sessions()
        };

        if force_refresh {
            self.index.clear();
        }

        let mut all_new: Vec<Session> = Vec::new();
        let mut all_deleted: Vec<String> = Vec::new();

        for adapter in &self.adapters {
            let (new_sessions, deleted) =
                adapter.find_sessions_incremental(&known, &None, &None);
            all_new.extend(new_sessions);
            all_deleted.extend(deleted);
        }

        // Apply changes
        if !all_deleted.is_empty() {
            self.index.delete_sessions(&all_deleted);
        }
        if !all_new.is_empty() {
            self.index.update_sessions(&all_new);
        }

        // Load from index
        let mut sessions = self.index.get_all_sessions();
        for s in &sessions {
            self.sessions_by_id.insert(s.id.clone(), s.clone());
        }
        sessions.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
        sessions
    }

    /// Search sessions with query and filters.
    pub fn search(
        &mut self,
        query: &str,
        agent_filter: Option<&str>,
        directory_filter: Option<&str>,
        limit: usize,
        sort_by_time: bool,
    ) -> Vec<Session> {
        let parsed = parse_query(query);

        let effective_agent = if let Some(agent) = agent_filter {
            Some(Filter {
                include: vec![agent.to_string()],
                exclude: vec![],
            })
        } else {
            parsed.agent
        };

        let effective_dir = if let Some(dir) = directory_filter {
            Some(Filter {
                include: vec![dir.to_string()],
                exclude: vec![],
            })
        } else {
            parsed.directory
        };

        let results = self.index.search(
            &parsed.text,
            effective_agent.as_ref(),
            effective_dir.as_ref(),
            parsed.date.as_ref(),
            limit,
            sort_by_time,
        );

        results
            .into_iter()
            .filter_map(|(id, _)| self.sessions_by_id.get(&id).cloned())
            .collect()
    }

    /// Get the resume command for a session.
    pub fn get_resume_command(&self, session: &Session, yolo: bool) -> Vec<String> {
        for adapter in &self.adapters {
            if adapter.name() == session.agent {
                return adapter.get_resume_command(session, yolo);
            }
        }
        vec![]
    }
}
