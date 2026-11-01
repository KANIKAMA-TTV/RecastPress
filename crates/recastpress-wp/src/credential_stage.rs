//! Unfinalized username/email credential stages, not an authentication API.
//!
//! Fixed initial-null/host-supplied-lookup/empty-relevant-filters profile. No previous
//! WP_User, application passwords, multisite/spam, authorization or sessions.
//! The host performs lookup; this module neither runs SQL nor chooses a runtime.

use crate::strip_all_tags::php_trim6;
use crate::{
    HostPasswordWorkLimits, LoginLookupPlan, PasswordDispatchError, PreparedLoginInput,
    dispatcher::Dispatcher, is_email, value::PhpValue, wp_check_password_raw_empty_filters_bytes,
    wp_password_needs_rehash_default_bcrypt_bytes,
};
use std::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CredentialLookupKind {
    Login,
    Email,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CredentialPhase {
    Input,
    Login,
    Email,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum RehashIntent {
    Suggested,
    NotSuggested,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum CredentialUnresolvedReason {
    Lookup,
    Preprocessing,
    UnsupportedHash,
    BcryptCostLimit,
    PortableCostLimit,
    ProviderFailure,
    RehashMetadata,
    EmailProfile,
}

/// A host reply to one consuming lookup request. Lookup failures are never
/// NotFound. R is opaque; the host must bind the returned row to this query.
pub enum CredentialLookup<R> {
    Found(R),
    NotFound,
    LookupUnresolved,
}

/// Raw credential match plus metadata hint only. No principal/ID conversion,
/// role, authorization, rehash/write execution or login/session permission.
pub struct UnfinalizedCredentialMatch<R> {
    record: R,
    via: CredentialLookupKind,
    rehash: RehashIntent,
}
impl<R> UnfinalizedCredentialMatch<R> {
    pub fn record(&self) -> &R {
        &self.record
    }
    pub fn matched_via(&self) -> CredentialLookupKind {
        self.via
    }
    pub fn rehash_intent(&self) -> RehashIntent {
        self.rehash
    }
}

/// NoCredentialMatch is not a WP_Error or an HTTP response. Unresolved stops
/// without trying another identity. Neither terminal is an auth policy.
pub enum CredentialStageOutcome<R> {
    NoCredentialMatch,
    Match(UnfinalizedCredentialMatch<R>),
    Unresolved {
        at: CredentialPhase,
        reason: CredentialUnresolvedReason,
    },
}
pub enum CredentialStage<'a, R> {
    NeedLogin(LoginLookupRequest<'a, R>),
    NeedEmail(EmailLookupRequest<'a, R>),
    Complete(CredentialStageOutcome<R>),
}
struct Context<'a, R> {
    prepared: PreparedLoginInput<'a>,
    limits: HostPasswordWorkLimits,
    hash_getter: fn(&R) -> &[u8],
}
pub struct LoginLookupRequest<'a, R> {
    context: Context<'a, R>,
    key: Vec<u8>,
}
pub struct EmailLookupRequest<'a, R> {
    context: Context<'a, R>,
    key: Vec<u8>,
}

macro_rules! redacted {
    ($($ty:ty => $name:literal),* $(,)?) => { $(
        impl fmt::Debug for $ty {
            fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(concat!($name,"([redacted])")) }
        }
    )* };
}
redacted!(CredentialLookupKind=>"CredentialLookupKind",CredentialPhase=>"CredentialPhase",RehashIntent=>"RehashIntent",CredentialUnresolvedReason=>"CredentialUnresolvedReason");
impl<R> fmt::Debug for CredentialLookup<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialLookup([redacted])")
    }
}
impl<R> fmt::Debug for UnfinalizedCredentialMatch<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnfinalizedCredentialMatch([redacted])")
    }
}
impl<R> fmt::Debug for CredentialStageOutcome<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialStageOutcome([redacted])")
    }
}
impl<R> fmt::Debug for CredentialStage<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialStage([redacted])")
    }
}
impl<R> fmt::Debug for LoginLookupRequest<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LoginLookupRequest([redacted])")
    }
}
impl<R> fmt::Debug for EmailLookupRequest<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EmailLookupRequest([redacted])")
    }
}

fn phase(kind: CredentialLookupKind) -> CredentialPhase {
    match kind {
        CredentialLookupKind::Login => CredentialPhase::Login,
        CredentialLookupKind::Email => CredentialPhase::Email,
    }
}
fn unresolved<'a, R>(
    at: CredentialPhase,
    reason: CredentialUnresolvedReason,
) -> CredentialStage<'a, R> {
    CredentialStage::Complete(CredentialStageOutcome::Unresolved { at, reason })
}
fn no_match<'a, R>() -> CredentialStage<'a, R> {
    CredentialStage::Complete(CredentialStageOutcome::NoCredentialMatch)
}
fn verification_reason(error: PasswordDispatchError) -> CredentialUnresolvedReason {
    match error {
        PasswordDispatchError::UnsupportedHash => CredentialUnresolvedReason::UnsupportedHash,
        PasswordDispatchError::BcryptCostLimit => CredentialUnresolvedReason::BcryptCostLimit,
        PasswordDispatchError::PortableCostLimit => CredentialUnresolvedReason::PortableCostLimit,
        PasswordDispatchError::ProviderFailure => CredentialUnresolvedReason::ProviderFailure,
    }
}
fn empty_email_check(input: &[u8]) -> Result<PhpValue, ()> {
    // No caller-supplied/mutable Dispatcher can install a filter into this scope.
    is_email(&mut Dispatcher::new(), input).map_err(|_| ())
}

impl<'a, R> CredentialStage<'a, R> {
    /// Consume source-derived S1/P1 and both explicit host cost allowances.
    /// The trusted getter reads one hash from the same opaque R; it cannot
    /// substitute a separate verification-reply row. Host truthfulness/freshness
    /// is a precondition, not a security boundary proved by this generic type.
    pub fn begin(
        prepared: PreparedLoginInput<'a>,
        limits: HostPasswordWorkLimits,
        hash_getter: fn(&R) -> &[u8],
    ) -> Self {
        if prepared.username_php_empty() || prepared.password_php_empty() {
            return no_match();
        }
        let plan = prepared.login_lookup_plan();
        let context = Context {
            prepared,
            limits,
            hash_getter,
        };
        match plan {
            Ok(LoginLookupPlan::QueryKey(key)) => {
                Self::NeedLogin(LoginLookupRequest { context, key })
            }
            Ok(LoginLookupPlan::Skipped) => context.email(),
            Err(_) => unresolved(
                CredentialPhase::Login,
                CredentialUnresolvedReason::Preprocessing,
            ),
        }
    }
}
impl<'a, R> Context<'a, R> {
    fn email(self) -> CredentialStage<'a, R> {
        self.email_with(empty_email_check)
    }
    fn email_with(
        self,
        check: impl FnOnce(&[u8]) -> Result<PhpValue, ()>,
    ) -> CredentialStage<'a, R> {
        let s1 = self.prepared.email_lookup_key();
        match check(s1) {
            Ok(PhpValue::Bool(false)) => no_match(),
            Ok(PhpValue::Bytes(value)) if value == s1 => {
                let key = php_trim6(s1);
                let mut owned = Vec::new();
                if owned.try_reserve_exact(key.len()).is_err() {
                    return unresolved(
                        CredentialPhase::Email,
                        CredentialUnresolvedReason::Preprocessing,
                    );
                }
                owned.extend_from_slice(key);
                CredentialStage::NeedEmail(EmailLookupRequest {
                    context: self,
                    key: owned,
                })
            }
            // Preserve no callback text or arbitrary returned data in diagnostics.
            _ => unresolved(
                CredentialPhase::Email,
                CredentialUnresolvedReason::EmailProfile,
            ),
        }
    }
    fn missing(self, kind: CredentialLookupKind) -> CredentialStage<'a, R> {
        match kind {
            CredentialLookupKind::Login => self.email(),
            CredentialLookupKind::Email => no_match(),
        }
    }
    fn found(self, record: R, kind: CredentialLookupKind) -> CredentialStage<'a, R> {
        self.found_with(
            record,
            kind,
            wp_check_password_raw_empty_filters_bytes,
            |hash| wp_password_needs_rehash_default_bcrypt_bytes(hash).map_err(|_| ()),
        )
    }
    fn found_with(
        self,
        record: R,
        kind: CredentialLookupKind,
        verify: impl FnOnce(&[u8], &[u8], HostPasswordWorkLimits) -> Result<bool, PasswordDispatchError>,
        hint: impl FnOnce(&[u8]) -> Result<bool, ()>,
    ) -> CredentialStage<'a, R> {
        // Borrow once: verification and hint refer to precisely this owned row.
        let hash = (self.hash_getter)(&record);
        match verify(self.prepared.password_trimmed(), hash, self.limits) {
            Ok(true) => match hint(hash) {
                Ok(needed) => CredentialStage::Complete(CredentialStageOutcome::Match(
                    UnfinalizedCredentialMatch {
                        record,
                        via: kind,
                        rehash: if needed {
                            RehashIntent::Suggested
                        } else {
                            RehashIntent::NotSuggested
                        },
                    },
                )),
                Err(()) => unresolved(phase(kind), CredentialUnresolvedReason::RehashMetadata),
            },
            Ok(false) => {
                drop(record);
                self.missing(kind)
            }
            Err(error) => unresolved(phase(kind), verification_reason(error)),
        }
    }
    fn respond(
        self,
        reply: CredentialLookup<R>,
        kind: CredentialLookupKind,
    ) -> CredentialStage<'a, R> {
        match reply {
            CredentialLookup::Found(record) => self.found(record, kind),
            CredentialLookup::NotFound => self.missing(kind),
            CredentialLookup::LookupUnresolved => {
                unresolved(phase(kind), CredentialUnresolvedReason::Lookup)
            }
        }
    }
}
impl<'a, R> LoginLookupRequest<'a, R> {
    pub fn query_key(&self) -> &[u8] {
        &self.key
    }
    pub fn respond(self, reply: CredentialLookup<R>) -> CredentialStage<'a, R> {
        self.context.respond(reply, CredentialLookupKind::Login)
    }
}
impl<'a, R> EmailLookupRequest<'a, R> {
    pub fn query_key(&self) -> &[u8] {
        &self.key
    }
    pub fn respond(self, reply: CredentialLookup<R>) -> CredentialStage<'a, R> {
        self.context.respond(reply, CredentialLookupKind::Email)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostMaxBcryptCost, HostMaxPhpassCost};
    use std::{cell::Cell, rc::Rc};
    struct Record {
        hash: Vec<u8>,
        gets: Rc<Cell<usize>>,
    }
    fn hash(record: &Record) -> &[u8] {
        record.gets.set(record.gets.get() + 1);
        &record.hash
    }
    fn context() -> Context<'static, Record> {
        Context {
            prepared: PreparedLoginInput::new(b"member@example.invalid", b"dummy", b"en_US", 128)
                .unwrap(),
            limits: HostPasswordWorkLimits::new(
                HostMaxBcryptCost::new(4).unwrap(),
                HostMaxPhpassCost::new(7).unwrap(),
            ),
            hash_getter: hash,
        }
    }
    #[test]
    fn provider_and_metadata_faults_stop_after_one_getter_without_becoming_email_or_mismatch() {
        for error in [
            PasswordDispatchError::UnsupportedHash,
            PasswordDispatchError::BcryptCostLimit,
            PasswordDispatchError::PortableCostLimit,
            PasswordDispatchError::ProviderFailure,
        ] {
            let gets = Rc::new(Cell::new(0));
            let count = gets.clone();
            let stage = context().found_with(
                Record {
                    hash: b"PRIVATE-hash".to_vec(),
                    gets,
                },
                CredentialLookupKind::Login,
                |_, _, _| Err(error),
                |_| panic!("metadata must not run"),
            );
            assert!(
                matches!(stage,CredentialStage::Complete(CredentialStageOutcome::Unresolved{at:CredentialPhase::Login,reason}) if reason==verification_reason(error))
            );
            assert_eq!(count.get(), 1);
        }
        let gets = Rc::new(Cell::new(0));
        let count = gets.clone();
        let stage = context().found_with(
            Record {
                hash: b"PRIVATE-hash".to_vec(),
                gets,
            },
            CredentialLookupKind::Login,
            |_, _, _| Ok(true),
            |_| Err(()),
        );
        assert!(matches!(
            stage,
            CredentialStage::Complete(CredentialStageOutcome::Unresolved {
                at: CredentialPhase::Login,
                reason: CredentialUnresolvedReason::RehashMetadata
            })
        ));
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn unexpected_email_types_changed_values_and_callback_errors_are_closed_profile_faults() {
        for value in [
            Ok(PhpValue::Bool(true)),
            Ok(PhpValue::Null),
            Ok(PhpValue::Bytes(b"different@example.invalid".to_vec())),
            Err(()),
        ] {
            let stage = context().email_with(|_| value);
            assert!(matches!(
                stage,
                CredentialStage::Complete(CredentialStageOutcome::Unresolved {
                    at: CredentialPhase::Email,
                    reason: CredentialUnresolvedReason::EmailProfile
                })
            ));
        }
        let stage = context().found_with(
            Record {
                hash: b"PRIVATE-hash".to_vec(),
                gets: Rc::new(Cell::new(0)),
            },
            CredentialLookupKind::Login,
            |_, _, _| Ok(true),
            |_| Ok(false),
        );
        assert!(
            matches!(stage,CredentialStage::Complete(CredentialStageOutcome::Match(candidate)) if candidate.rehash_intent()==RehashIntent::NotSuggested)
        );
    }
}
