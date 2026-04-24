//! Google Meet — stub adapter.
//!
//! Meet runs entirely in the browser. There is no local log or native
//! window to tail, and scraping a Chrome tab's accessibility tree from
//! the Meetily process is fragile and permission-heavy. The clean
//! answer is a small companion browser extension that watches the Meet
//! DOM and posts events over a localhost WebSocket. That lives in a
//! separate bundle; this adapter always reports `Unsupported` so the
//! Settings UI can render a clear "Install the Meetily browser
//! extension" CTA.

use anyhow::{anyhow, Result};

use super::{AdapterSnapshot, AdapterStatus, IntegratedAdapter};

pub struct MeetStubAdapter;

impl MeetStubAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl IntegratedAdapter for MeetStubAdapter {
    fn id(&self) -> &'static str {
        "meet"
    }

    fn status(&self) -> AdapterStatus {
        AdapterStatus::Unsupported {
            reason: "Install the Meetily browser extension to enable Google Meet integration."
                .to_string(),
        }
    }

    fn snapshot(&self) -> Result<AdapterSnapshot> {
        Err(anyhow!(
            "meet: no native integration — install the browser extension"
        ))
    }
}
