//! Explicit URL configuration. A snapshot does not depend on mutable global
//! state.

use serde::{Deserialize, Serialize};

/// URL templates use `{}` for an identifier and `${url_prefix}` for the base
/// URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Base URL, initialized from `ANTHOLOGY_PREFIX` when present.
    pub url_prefix: String,
    /// Paper landing page template.
    pub paper_page_template: String,
    /// PDF file template.
    pub pdf_location_template: String,
    /// PDF thumbnail template.
    pub pdf_thumbnail_location_template: String,
    /// Attachment template.
    pub attachment_location_template: String,
    /// Event-related file template (also used for URL references).
    pub event_location_template: String,
    /// Event landing page template.
    pub event_page_template: String,
    /// Video template.
    pub video_location_template: String,
    /// Volume landing page template.
    pub volume_page_template: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url_prefix: std::env::var("ANTHOLOGY_PREFIX")
                .unwrap_or_else(|_| "https://aclanthology.org".into()),
            paper_page_template: "${url_prefix}/{}/".into(),
            pdf_location_template: "${url_prefix}/{}.pdf".into(),
            pdf_thumbnail_location_template: "${url_prefix}/thumb/{}.jpg".into(),
            attachment_location_template: "${url_prefix}/attachments/{}".into(),
            event_location_template: "${url_prefix}/{}".into(),
            event_page_template: "${url_prefix}/events/{}/".into(),
            video_location_template: "${url_prefix}/{}".into(),
            volume_page_template: "${url_prefix}/{}/".into(),
        }
    }
}

impl Config {
    /// Resolve a URL template with this configuration.
    pub fn format(&self, template: &str, id: &str) -> String {
        template
            .replace("${url_prefix}", &self.url_prefix)
            .replace("{}", id)
    }
}
