//! Microsoft Teams — Windows UIAutomation adapter.
//!
//! Walks the accessibility tree of the Teams top-level window and
//! extracts candidate participant display names. Teams v2 is WebView2-
//! based, so tiles render as DOM elements that UIA surfaces as Text /
//! TextBlock / Group controls. Display names are stable targets
//! because Teams needs them for screen-reader compliance.
//!
//! Read-only by design — we never click or modify any Teams UI.
//! Windows-only: the non-windows fallback below just reports
//! `Unsupported` so the adapter compiles cross-platform.

use anyhow::Result;

use super::{AdapterSnapshot, AdapterStatus, IntegratedAdapter};

pub struct TeamsUiaAdapter;

impl TeamsUiaAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "windows"))]
impl IntegratedAdapter for TeamsUiaAdapter {
    fn id(&self) -> &'static str {
        "teams"
    }
    fn status(&self) -> AdapterStatus {
        AdapterStatus::Unsupported {
            reason: "UIAutomation adapter is Windows-only.".to_string(),
        }
    }
    fn snapshot(&self) -> Result<AdapterSnapshot> {
        anyhow::bail!("teams/a11y: Windows-only adapter")
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use anyhow::{anyhow, Context};
    use std::collections::HashSet;
    use uiautomation::{UIAutomation, UIElement};

    impl IntegratedAdapter for TeamsUiaAdapter {
        fn id(&self) -> &'static str {
            "teams"
        }

        fn status(&self) -> AdapterStatus {
            match find_teams_window() {
                Ok(Some(_)) => AdapterStatus::Ready,
                _ => AdapterStatus::NotDetected,
            }
        }

        fn snapshot(&self) -> Result<AdapterSnapshot> {
            let root = find_teams_window()
                .context("Looking for Teams window")?
                .ok_or_else(|| anyhow!("No Microsoft Teams window is open."))?;

            let mut names: HashSet<String> = HashSet::new();
            walk_names(&root, &mut names, 0, 8);

            // Drop obvious UI chrome labels.
            let cleaned: Vec<String> = names
                .into_iter()
                .filter(|n| is_plausible_name(n))
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            if cleaned.is_empty() {
                return Err(anyhow!(
                    "teams/a11y: Teams window is open but no participant names were found in the accessibility tree. Open the meeting's participants pane or roster view and try again."
                ));
            }

            let mut sorted = cleaned;
            sorted.sort();

            Ok(AdapterSnapshot {
                participants: sorted,
                current_speaker: None,
                source: "teams/a11y".to_string(),
            })
        }
    }

    /// Top-level window whose name contains "Microsoft Teams". Returns
    /// None when Teams isn't running. Returns Ok(Some(_)) for the main
    /// window we can walk.
    fn find_teams_window() -> Result<Option<UIElement>> {
        let automation =
            UIAutomation::new().map_err(|e| anyhow!("Failed to init UIAutomation: {:?}", e))?;
        let root = automation
            .get_root_element()
            .map_err(|e| anyhow!("Failed to get UIA root: {:?}", e))?;
        let walker = automation
            .get_control_view_walker()
            .map_err(|e| anyhow!("UIA walker: {:?}", e))?;
        let mut child = walker
            .get_first_child(&root)
            .ok();
        while let Some(c) = child {
            let name = c.get_name().unwrap_or_default();
            if name.contains("Microsoft Teams") || name.contains("Microsoft Teams (work or school)") {
                return Ok(Some(c));
            }
            child = walker.get_next_sibling(&c).ok();
        }
        Ok(None)
    }

    /// Depth-first walk of the element tree, collecting Name strings.
    fn walk_names(element: &UIElement, names: &mut HashSet<String>, depth: usize, max_depth: usize) {
        if depth > max_depth {
            return;
        }
        if let Ok(name) = element.get_name() {
            let trimmed = name.trim();
            if !trimmed.is_empty() && trimmed.len() <= 80 {
                names.insert(trimmed.to_string());
            }
        }
        if let Ok(auto_id) = element.get_automation_id() {
            // Skip descending into decorative / container nodes that
            // never hold participant names — cheap pruning.
            if auto_id == "UI_CHROME_DECORATION" {
                return;
            }
        }
        let automation = match UIAutomation::new() {
            Ok(a) => a,
            Err(_) => return,
        };
        let walker = match automation.get_control_view_walker() {
            Ok(w) => w,
            Err(_) => return,
        };
        let mut child = walker.get_first_child(element).ok();
        while let Some(c) = child {
            walk_names(&c, names, depth + 1, max_depth);
            child = walker.get_next_sibling(&c).ok();
        }
    }

    /// Heuristic filter: drop UI strings that couldn't be a person's name.
    fn is_plausible_name(s: &str) -> bool {
        if s.is_empty() || s.len() > 80 || s.len() < 2 {
            return false;
        }
        // Must contain at least one whitespace (first + last) OR be a
        // plausible single Given name.
        let word_count = s.split_whitespace().count();
        if word_count > 6 {
            return false; // Long strings are UI copy, not names.
        }
        // Reject if the string looks like a sentence / action.
        if s.contains("..")
            || s.contains("click")
            || s.contains("Click")
            || s.contains('?')
            || s.contains('!')
            || s.contains(':')
        {
            return false;
        }
        // Needs to be mostly letters + spaces.
        let letters = s
            .chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace() || *c == '\'' || *c == '-')
            .count();
        if letters * 10 < s.len() * 8 {
            return false;
        }
        // First char must be uppercase — common human-name shape.
        s.chars().next().map_or(false, |c| c.is_uppercase())
    }
}
