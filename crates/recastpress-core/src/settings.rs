//! Shared settings for migration and the single RecastPress settings screen.

/// OIDC policy. Linking-session recovery is intentionally outside this type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SsoPolicy {
    #[default]
    Off,
    Optional,
    Required,
}

/// New-site defaults. There is deliberately no manual cache-capacity field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SiteSettings {
    pub cache_enabled: bool,
    pub sso: SsoPolicy,
    pub comments_open: bool,
}

impl Default for SiteSettings {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            sso: SsoPolicy::Off,
            comments_open: false,
        }
    }
}

impl SiteSettings {
    /// Imported comment policy must not be overwritten with new-site defaults (R28).
    pub fn from_migration(comments_open: bool, cache_enabled: bool, sso: SsoPolicy) -> Self {
        Self {
            cache_enabled,
            sso,
            comments_open,
        }
    }
}
