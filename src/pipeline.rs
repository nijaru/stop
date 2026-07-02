//! Process filtering pipeline: search → filter → sort → truncate.

use crate::data::{self, ProcessInfo, DEFAULT_TOP_N};
use crate::filter::FilterExpr;

/// A reusable pipeline that transforms a process list.
///
/// Applied in order: search filter → structured filter → sort → top-N truncate.
/// Both one-shot and watch mode use the same pipeline, eliminating duplication.
pub struct Pipeline {
    search: Option<String>,
    filter: Option<FilterExpr>,
    sort_by: String,
    limit: usize,
}

impl Pipeline {
    /// Creates a new pipeline with the given parameters.
    ///
    /// Defaults sort to `"cpu"` and limit to `DEFAULT_TOP_N` when not specified.
    pub fn new(
        search: Option<String>,
        filter: Option<FilterExpr>,
        sort_by: Option<String>,
        limit: Option<usize>,
    ) -> Self {
        Self {
            search,
            filter,
            sort_by: sort_by.unwrap_or_else(|| "cpu".to_string()),
            limit: limit.unwrap_or(DEFAULT_TOP_N),
        }
    }

    /// Returns true if any filtering (search or structured filter) is active.
    pub fn has_active_filter(&self) -> bool {
        self.search.is_some() || self.filter.is_some()
    }

    /// Applies all pipeline stages to a process list in-place.
    pub fn apply(&self, processes: &mut Vec<ProcessInfo>) {
        // 1. Search filter — case-insensitive substring in name or command.
        //    Always excludes the current process to avoid self-reference.
        if let Some(ref search_term) = self.search {
            let search_lower = search_term.to_lowercase();
            let current_pid = std::process::id();
            processes.retain(|p| {
                if p.pid == current_pid {
                    return false;
                }
                p.name.to_lowercase().contains(&search_lower)
                    || p.command.to_lowercase().contains(&search_lower)
            });
        }

        // 2. Structured filter expression
        if let Some(ref f) = self.filter {
            processes.retain(|p| f.matches(p));
        }

        // 3. Sort
        data::sort_processes(processes, &self.sort_by);

        // 4. Truncate to top-N
        processes.truncate(self.limit);
    }
}
