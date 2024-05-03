use crate::{FetchHeaders, fetch_options::ContentType};

#[derive(Default, Debug, Clone)]
pub struct FetchConfig {
    pub timeout_ms: Option<u64>,
    pub headers: Option<FetchHeaders>,
    /// What content-type should these requests accept (overrideable via FetchOptions)
    pub accept: ContentType,
    /// What content-type does do these requests send (overrideable via FetchOptions)
    pub content_type: ContentType
}
