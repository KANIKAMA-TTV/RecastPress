//! Prepared source-derived credential prefix, never an authenticated principal.
use crate::strip_all_tags::php_trim6;
use crate::{SanitizeUserError, sanitize_user_utf8_no_open_angle_empty_filters_bytes};
use std::fmt;

pub struct PreparedLoginInput<'a> {
    username: Vec<u8>,
    password: &'a [u8],
    locale: &'a [u8],
    cap: usize,
}
impl fmt::Debug for PreparedLoginInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedLoginInput([redacted])")
    }
}
/// Skipped is get_data_by's pre-query empty check, not an authentication result.
pub enum LoginLookupPlan {
    Skipped,
    QueryKey(Vec<u8>),
}
impl fmt::Debug for LoginLookupPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LoginLookupPlan([redacted])")
    }
}
fn php_empty(bytes: &[u8]) -> bool {
    bytes.is_empty() || bytes == b"0"
}
impl<'a> PreparedLoginInput<'a> {
    /// pluggable.php:689 prefix only: sanitize once (S1), trim password (P1).
    /// Password/locale are borrowed, S1 alone is owned; no Clone/Display/identity.
    pub fn new(
        username: &[u8],
        password: &'a [u8],
        locale: &'a [u8],
        max_output_bytes: usize,
    ) -> Result<Self, SanitizeUserError> {
        Ok(Self {
            username: sanitize_user_utf8_no_open_angle_empty_filters_bytes(
                username,
                false,
                locale,
                max_output_bytes,
            )?,
            password: php_trim6(password),
            locale,
            cap: max_output_bytes,
        })
    }
    pub fn username_stage1(&self) -> &[u8] {
        &self.username
    }
    pub fn password_trimmed(&self) -> &[u8] {
        self.password
    }
    pub fn username_php_empty(&self) -> bool {
        php_empty(&self.username)
    }
    pub fn password_php_empty(&self) -> bool {
        php_empty(self.password)
    }
    /// The stage-one email candidate only: no email validation or SQL lookup.
    pub fn email_lookup_key(&self) -> &[u8] {
        &self.username
    }
    /// class-wp-user.php:222/244: trim S1, reject PHP-empty, sanitize again S2.
    /// There is no second empty check after sanitize; an empty S2 is QueryKey.
    /// Password-empty/auth/filter decisions remain separate from this plan.
    pub fn login_lookup_plan(&self) -> Result<LoginLookupPlan, SanitizeUserError> {
        let key = php_trim6(&self.username);
        if php_empty(key) {
            return Ok(LoginLookupPlan::Skipped);
        }
        Ok(LoginLookupPlan::QueryKey(
            sanitize_user_utf8_no_open_angle_empty_filters_bytes(
                key,
                false,
                self.locale,
                self.cap,
            )?,
        ))
    }
}
