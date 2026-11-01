//! Development-only JSON probe. Never linked into the public HTTP process.
mod common;
use common::{decode, encode, text};
use recastpress_wp::maybe_serialize;
use recastpress_wp::maybe_unserialize;
use recastpress_wp::{absint, wp_parse_id_list};
use recastpress_wp::{
    dispatcher::{Dispatcher, DispatcherCallback},
    hooks::CallbackId,
};
use recastpress_wp::{is_serialized, is_serialized_string};
use recastpress_wp::{sanitize_key, wp_checkdate};
use recastpress_wp::{trailingslashit_bytes, untrailingslashit_bytes, wp_normalize_path_bytes};
use recastpress_wp::{
    value::{PhpArray, PhpKey, PhpValue},
    wp_array_slice_assoc, wp_is_numeric_array, wp_list_pluck, wp_parse_list, wp_slash, wp_unslash,
    wp_validate_boolean,
};
use serde_json::{Value, json};
use std::io::{self, Read};

fn run(case: &Value, unique_ids: &mut recastpress_wp::UniqueIds) -> Result<Value, String> {
    let input = decode(&case["input"])?;
    let value = match text(case, "api")? {
        "wp_kses_bad_protocol" => {
            const DOMAIN: &str = "Protocol-outer fixture requires byte content and an array of byte-string protocols";
            let PhpValue::Bytes(bytes) = input else {
                return Err(DOMAIN.into());
            };
            let PhpValue::Array(protocols) = decode(case.get("allowed_protocols").ok_or(DOMAIN)?)?
            else {
                return Err(DOMAIN.into());
            };
            if protocols
                .iter()
                .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
            {
                return Err(DOMAIN.into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_bad_protocol_c_bytes(&bytes, &protocols, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_bad_protocol_once" => {
            const DOMAIN: &str = "Protocol-once fixture requires byte content, byte protocol array, and default count";
            let PhpValue::Bytes(bytes) = input else {
                return Err(DOMAIN.into());
            };
            if case.get("count").is_some() {
                return Err(DOMAIN.into());
            }
            let PhpValue::Array(protocols) = decode(case.get("allowed_protocols").ok_or(DOMAIN)?)?
            else {
                return Err(DOMAIN.into());
            };
            if protocols
                .iter()
                .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
            {
                return Err(DOMAIN.into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_bad_protocol_once_c_bytes(&bytes, &protocols, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_bad_protocol_once2" => {
            const DOMAIN: &str =
                "Protocol fixture requires byte scheme and an array of byte-string protocols";
            let PhpValue::Bytes(bytes) = input else {
                return Err(DOMAIN.into());
            };
            let PhpValue::Array(protocols) = decode(case.get("allowed_protocols").ok_or(DOMAIN)?)?
            else {
                return Err(DOMAIN.into());
            };
            if protocols
                .iter()
                .any(|(_, value)| !matches!(value, PhpValue::Bytes(_)))
            {
                return Err(DOMAIN.into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_bad_protocol_once2_c_bytes(&bytes, &protocols, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_stripslashes" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("KSES slash fixture requires byte-string input".into());
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_stripslashes_bytes(&bytes, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_no_null" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("Null-removal fixture requires byte-string input and explicit remove/keep option".into());
            };
            let mode = match case.get("slash_zero").and_then(Value::as_str) {
                Some("remove") => recastpress_wp::SlashZeroMode::Remove,
                Some("keep") => recastpress_wp::SlashZeroMode::Keep,
                _ => return Err("Null-removal fixture requires byte-string input and explicit remove/keep option".into()),
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_no_null_bytes(&bytes, mode, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_decode_entities" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("Entity decoder fixture requires byte-string input".into());
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_kses_decode_entities_byte_values(&bytes, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "sanitize_text_field" | "sanitize_textarea_field" => {
            const DOMAIN: &str = "Unsupported UTF8-site text sanitizer fixture or host cap";
            let PhpValue::Bytes(raw) = input else {
                return Err(DOMAIN.into());
            };
            let wire = case.get("input").and_then(Value::as_object).ok_or(DOMAIN)?;
            if wire.len() != 2
                || wire.get("type").and_then(Value::as_str) != Some("string")
                || wire
                    .get("hex")
                    .and_then(Value::as_str)
                    .is_none_or(|hex| hex.len() > 8192)
                || case.get("text_profile").and_then(Value::as_str)
                    != Some("raw_no_lt_fixed_utf8_no_mbstring_empty_filters")
                || case.get("empty_filters").and_then(Value::as_bool) != Some(true)
            {
                return Err(DOMAIN.into());
            }
            let limits = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            let values = [
                ("max_input_bytes", 4096_u64),
                ("max_output_bytes", 4096),
                ("max_percent_rounds", 1365),
            ];
            if limits.len() != 3
                || values
                    .into_iter()
                    .any(|(name, value)| limits.get(name).and_then(Value::as_u64) != Some(value))
            {
                return Err(DOMAIN.into());
            }
            let limits = recastpress_wp::SanitizeTextLimits::new(4096, 4096, 1365);
            let result = if text(case, "api")? == "sanitize_text_field" {
                recastpress_wp::sanitize_text_field_utf8_site_no_open_angle_empty_filters_bytes(
                    &raw, limits,
                )
            } else {
                recastpress_wp::sanitize_textarea_field_utf8_site_no_open_angle_empty_filters_bytes(
                    &raw, limits,
                )
            }
            .map_err(|error| error.to_string())?;
            encode(&PhpValue::Bytes(result))?
        }
        "wp_parse_auth_cookie" => {
            const DOMAIN: &str = "Unsupported explicit cookie parser fixture or host cap";
            if case
                .get("auth_cookie_parse_profile")
                .and_then(Value::as_str)
                != Some("explicit_php_truthy_cookie_arbitrary_byte_scheme")
                || case.get("explicit_cookie").and_then(Value::as_bool) != Some(true)
            {
                return Err(DOMAIN.into());
            }
            let bytes_field = |name: &str| -> Result<Vec<u8>, String> {
                let wire = case.get(name).ok_or(DOMAIN)?;
                let fields = wire.as_object().ok_or(DOMAIN)?;
                let hex = fields.get("hex").and_then(Value::as_str).ok_or(DOMAIN)?;
                if fields.len() != 2
                    || fields.get("type").and_then(Value::as_str) != Some("string")
                    || hex.len() > 131072
                {
                    return Err(DOMAIN.into());
                }
                match decode(wire)? {
                    PhpValue::Bytes(value) => Ok(value),
                    _ => Err(DOMAIN.into()),
                }
            };
            let limits = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            if limits.len() != 1
                || limits.get("max_input_bytes").and_then(Value::as_u64) != Some(65536)
            {
                return Err(DOMAIN.into());
            }
            let cookie = bytes_field("input")?;
            let scheme = bytes_field("scheme")?;
            match recastpress_wp::wp_parse_auth_cookie_explicit_bytes(&cookie, &scheme, 65536)
                .map_err(|error| error.to_string())?
            {
                None => encode(&PhpValue::Bool(false))?,
                Some(parsed) => {
                    let mut fields = PhpArray::new();
                    for (name, value) in [
                        ("username", parsed.username()),
                        ("expiration", parsed.expiration_bytes()),
                        ("token", parsed.token()),
                        ("hmac", parsed.hmac_bytes()),
                        ("scheme", parsed.scheme_label()),
                    ] {
                        // Development wire copies borrowed public fixture fields;
                        // it is not the native view's ownership or secret ABI.
                        fields.insert(
                            PhpKey::Bytes(name.as_bytes().to_vec()),
                            PhpValue::Bytes(value.to_vec()),
                        );
                    }
                    encode(&PhpValue::Array(fields))?
                }
            }
        }
        "wp_generate_auth_cookie" => {
            const DOMAIN: &str = "Unsupported explicit auth cookie fixture or host cap";
            let bytes_field = |name: &str| -> Result<Vec<u8>, String> {
                let wire = case.get(name).ok_or(DOMAIN)?;
                let fields = wire.as_object().ok_or(DOMAIN)?;
                let hex = fields.get("hex").and_then(Value::as_str).ok_or(DOMAIN)?;
                if fields.len() != 2
                    || fields.get("type").and_then(Value::as_str) != Some("string")
                    || hex.len() > 131072
                {
                    return Err(DOMAIN.into());
                }
                match decode(wire)? {
                    PhpValue::Bytes(bytes) => Ok(bytes),
                    _ => Err(DOMAIN.into()),
                }
            };
            let integer_field = |name: &str| -> Result<i64, String> {
                let wire = case.get(name).ok_or(DOMAIN)?;
                let fields = wire.as_object().ok_or(DOMAIN)?;
                if fields.len() != 2
                    || fields.get("type").and_then(Value::as_str) != Some("int")
                    || fields.get("value").and_then(Value::as_str).is_none()
                {
                    return Err(DOMAIN.into());
                }
                match decode(wire)? {
                    PhpValue::Integer(value) => Ok(value),
                    _ => Err(DOMAIN.into()),
                }
            };
            if case.get("auth_cookie_profile").and_then(Value::as_str)
                != Some("prepared_existing_user_resolved_salt_truthy_token_empty_filters")
                || case.get("empty_filters").and_then(Value::as_bool) != Some(true)
                || !matches!(
                    case.get("scheme").and_then(Value::as_str),
                    Some("auth" | "secure_auth" | "logged_in")
                )
            {
                return Err(DOMAIN.into());
            }
            let limits = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            if limits.len() != 2
                || ["max_input_bytes", "max_output_bytes"]
                    .into_iter()
                    .any(|name| limits.get(name).and_then(Value::as_u64) != Some(65536))
            {
                return Err(DOMAIN.into());
            }
            let login = bytes_field("input")?;
            let password_hash = bytes_field("password_hash")?;
            let token = bytes_field("token")?;
            let salt = bytes_field("resolved_salt")?;
            let owner = integer_field("user_id")?;
            let expiration = integer_field("expiration")?;
            // Scheme chooses the caller-resolved salt in the PHP fixture only.
            // Native does not resolve configuration or adopt a cookie policy.
            let user = recastpress_wp::PreparedAuthCookieUser::new(owner, &login, &password_hash)
                .map_err(|error| error.to_string())?;
            let cookie =
                recastpress_wp::wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes(
                    &user,
                    expiration,
                    &token,
                    &salt,
                    recastpress_wp::AuthCookieLimits::new(65536, 65536),
                )
                .map_err(|error| error.to_string())?;
            // Development output is intentionally public dummy fixture data;
            // this serialization copy is not a production secret-owner ABI.
            encode(&PhpValue::Bytes(cookie.as_cookie_bytes().to_vec()))?
        }
        "get_the_title" => {
            const DOMAIN: &str = "Unsupported prepared default title fixture or host cap";
            let PhpValue::Bytes(bytes) = input else {
                return Err(DOMAIN.into());
            };
            if case.get("title_profile").and_then(Value::as_str)
                != Some("prepared_raw_publish_frontend_default_english_no_html")
                || case.get("post_filter").and_then(Value::as_str) != Some("raw")
                || case.get("frontend").and_then(Value::as_bool) != Some(true)
            {
                return Err(DOMAIN.into());
            }
            let PhpValue::Integer(id) = decode(case.get("post_id").ok_or(DOMAIN)?)? else {
                return Err(DOMAIN.into());
            };
            let PhpValue::Bytes(password) = decode(case.get("post_password").ok_or(DOMAIN)?)?
            else {
                return Err(DOMAIN.into());
            };
            let PhpValue::Bytes(status) = decode(case.get("post_status").ok_or(DOMAIN)?)? else {
                return Err(DOMAIN.into());
            };
            let limits = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            let names = [
                ("max_input_bytes", 65536_u64),
                ("max_work_bytes", 1048576),
                ("max_output_bytes", 65536),
            ];
            if limits.len() != names.len()
                || names
                    .into_iter()
                    .any(|(name, cap)| limits.get(name).and_then(Value::as_u64) != Some(cap))
            {
                return Err(DOMAIN.into());
            }
            let post = recastpress_wp::PreparedTitlePost::new(id, &bytes, &password, &status)
                .map_err(|error| error.to_string())?;
            encode(&PhpValue::Bytes(
                recastpress_wp::get_the_title_prepared_frontend_default_english_no_html_bytes(
                    &post,
                    recastpress_wp::TexturizeLimits::new(65536, 1048576, 65536),
                )
                .map_err(|error| error.to_string())?,
            ))?
        }
        "wptexturize" => {
            const DOMAIN: &str = "Unsupported texturize fixture or host cap";
            let PhpValue::Bytes(bytes) = input else {
                return Err(DOMAIN.into());
            };
            if case.get("texturize_profile").and_then(Value::as_str)
                != Some("valid_utf8_no_lt_default_english_empty_filters")
                || case.get("reset").and_then(Value::as_bool) != Some(true)
            {
                return Err(DOMAIN.into());
            }
            let limits = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            let names = [
                ("max_input_bytes", 65536_u64),
                ("max_work_bytes", 1048576),
                ("max_output_bytes", 65536),
            ];
            if limits.len() != names.len()
                || names
                    .into_iter()
                    .any(|(name, cap)| limits.get(name).and_then(Value::as_u64) != Some(cap))
            {
                return Err(DOMAIN.into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::wptexturize_default_english_no_html_bytes(
                    &bytes,
                    recastpress_wp::TexturizeLimits::new(65536, 1048576, 65536),
                )
                .map_err(|error| error.to_string())?,
            ))?
        }
        "get_post_meta" => {
            const DOMAIN: &str = "Unsupported cached single post metadata fixture or host cap";
            let PhpValue::Integer(owner) = input else {
                return Err(DOMAIN.into());
            };
            if case.get("post_meta_profile").and_then(Value::as_str)
                != Some("complete_cached_single_empty_filters")
                || ["single", "complete_cache", "empty_filters"]
                    .into_iter()
                    .any(|name| case.get(name).and_then(Value::as_bool) != Some(true))
            {
                return Err(DOMAIN.into());
            }
            let PhpValue::Bytes(key) = decode(case.get("meta_key").ok_or(DOMAIN)?)? else {
                return Err(DOMAIN.into());
            };
            let PhpValue::Array(cache) = decode(case.get("cache").ok_or(DOMAIN)?)? else {
                return Err(DOMAIN.into());
            };
            let allowances = case
                .get("limits")
                .and_then(Value::as_object)
                .ok_or(DOMAIN)?;
            const NAMES: [&str; 4] = [
                "max_input_bytes",
                "max_array_entries",
                "max_owned_string_bytes",
                "max_array_depth",
            ];
            if allowances.len() != NAMES.len()
                || NAMES.iter().any(|name| !allowances.contains_key(*name))
            {
                return Err(DOMAIN.into());
            }
            let mut limits = [0; 4];
            for (index, name) in NAMES.into_iter().enumerate() {
                limits[index] = allowances[name]
                    .as_i64()
                    .filter(|value| *value >= 0)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(DOMAIN)?;
            }
            let limits = recastpress_wp::serialization::UnserializeLimits::new(
                limits[0], limits[1], limits[2], limits[3],
            )
            .map_err(|_| DOMAIN)?;
            let cache = recastpress_wp::post_meta_cached::PreparedPostMetaCache::new(owner, &cache)
                .map_err(|_| DOMAIN)?;
            // The optional witness is only the PHP oracle's canonical-domain
            // preflight. Native results are always read and decoded from cache.
            encode(
                &recastpress_wp::post_meta_cached::get_post_meta_single_cached_no_filters(
                    &cache, &key, limits,
                )
                .map_err(|_| DOMAIN)?,
            )?
        }
        "_truncate_post_slug" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("Truncate slug requires Bytes input".into());
            };
            let length = case
                .get("length")
                .and_then(Value::as_i64)
                .filter(|value| *value >= 0)
                .ok_or("Truncate slug requires nonnegative i64 length")?;
            let length =
                usize::try_from(length).map_err(|_| "Truncate slug length outside host usize")?;
            if case.get("truncate_profile").and_then(Value::as_str)
                != Some("conditional_decoded_utf8_no_mbstring")
            {
                return Err("Truncate slug profile differs".into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::truncate_post_slug_conditional_decoded_utf8_bytes(
                    &raw, length, 65536,
                )
                .map_err(|_| "Unsupported truncate slug input or host cap")?,
            ))?
        }
        "sanitize_title" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("Default title requires Bytes input".into());
            };
            let PhpValue::Bytes(fallback) = decode(
                case.get("fallback")
                    .ok_or("Default title fallback required")?,
            )?
            else {
                return Err("Default title fallback must be Bytes".into());
            };
            let PhpValue::Bytes(locale) =
                decode(case.get("locale").ok_or("Default title locale required")?)?
            else {
                return Err("Default title locale must be Bytes".into());
            };
            if case.get("default_title_profile").and_then(Value::as_str)
                != Some("valid_utf8_no_lt_explicit_locale_default_title_callback")
            {
                return Err("Default title profile differs".into());
            }
            let context = match case.get("context").and_then(Value::as_str) {
                Some("save") => recastpress_wp::SanitizeTitleContext::Save,
                Some("display") => recastpress_wp::SanitizeTitleContext::Display,
                Some("query") => recastpress_wp::SanitizeTitleContext::Query,
                _ => return Err("Default title context unsupported".into()),
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::sanitize_title_utf8_default_callback_explicit_locale_bytes(
                    &raw, &fallback, context, &locale, 65536,
                )
                .map_err(|_| "Unsupported default title input or host cap")?,
            ))?
        }
        "utf8_uri_encode" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("URI fixture requires byte input".into());
            };
            let length = case
                .get("length")
                .and_then(Value::as_i64)
                .filter(|value| *value >= 0)
                .ok_or("URI fixture requires nonnegative i64 length")?;
            let length = usize::try_from(length).map_err(|_| "URI length outside host usize")?;
            let flag = case
                .get("encode_ascii")
                .and_then(Value::as_bool)
                .ok_or("URI fixture requires exact bool")?;
            if case.get("uri_profile").and_then(Value::as_str)
                != Some("valid_utf8_explicit_length_bool_no_mbstring")
            {
                return Err("URI profile differs".into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::utf8_uri_encode_utf8_bytes(&raw, length, flag, 65536)
                    .map_err(|_| "Unsupported URI input or host bound")?,
            ))?
        }
        "sanitize_title_with_dashes" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("Title fixture requires byte input".into());
            };
            let PhpValue::Bytes(raw_title) =
                decode(case.get("raw_title").ok_or("Title raw argument required")?)?
            else {
                return Err("Title raw argument must be bytes".into());
            };
            if !raw_title.is_empty()
                || case.get("title_profile").and_then(Value::as_str)
                    != Some("no_lt_post_nul_utf8_explicit_context_no_mbstring")
            {
                return Err("Title profile or raw argument differs".into());
            }
            let context = match case.get("context").and_then(Value::as_str) {
                Some("display") => recastpress_wp::TitleDashContext::Display,
                Some("save") => recastpress_wp::TitleDashContext::Save,
                _ => return Err("Title context unsupported".into()),
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes(
                    &raw, context, 65536,
                )
                .map_err(|_| "Unsupported title input or host bound")?,
            ))?
        }
        "wp_strip_all_tags" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("Strip fixture requires byte input".into());
            };
            let flag = case
                .get("remove_breaks")
                .and_then(Value::as_bool)
                .ok_or("Strip fixture requires bool flag")?;
            if case.get("strip_profile").and_then(Value::as_str) != Some("no_literal_lt") {
                return Err("Strip fixture requires explicit profile".into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_strip_all_tags_no_open_angle_bytes(&raw, flag, 65536)
                    .map_err(|_| "Unsupported strip input or host bound")?,
            ))?
        }
        "sanitize_user" => {
            let PhpValue::Bytes(raw) = input else {
                return Err("User fixture requires byte input".into());
            };
            let PhpValue::Bytes(locale) = decode(case.get("locale").ok_or("Missing user locale")?)?
            else {
                return Err("User fixture requires byte locale".into());
            };
            let strict = case
                .get("strict")
                .and_then(Value::as_bool)
                .ok_or("User fixture requires strict bool")?;
            if case.get("user_profile").and_then(Value::as_str)
                != Some("no_lt_utf8_resolved_locale_c_empty_filters")
            {
                return Err("User fixture requires explicit profile".into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::sanitize_user_utf8_no_open_angle_empty_filters_bytes(
                    &raw, strict, &locale, 65536,
                )
                .map_err(|_| "Unsupported user input or host bound")?,
            ))?
        }
        "remove_accents" => {
            let PhpValue::Bytes(text) = input else {
                return Err("Accent fixture requires byte text".into());
            };
            let PhpValue::Bytes(locale) =
                decode(case.get("locale").ok_or("missing accent locale")?)?
            else {
                return Err("Accent fixture requires byte locale".into());
            };
            if case.get("accent_profile").and_then(Value::as_str)
                != Some("valid_utf8_explicit_locale_no_intl")
            {
                return Err("Accent fixture requires explicit no-Intl profile".into());
            }
            encode(&PhpValue::Bytes(
                recastpress_wp::remove_accents_utf8_explicit_locale_bytes(&text, &locale, 65536)
                    .map_err(|_| "Unsupported accent input or output bound")?,
            ))?
        }
        "wp_password_needs_rehash" => {
            let PhpValue::Bytes(hash) = input else {
                return Err("Rehash fixture requires byte hashes".into());
            };
            if case.get("rehash_profile").and_then(Value::as_str) != Some("default_bcrypt_empty") {
                return Err("Rehash fixture requires explicit default empty profile".into());
            }
            json!(
                recastpress_wp::wp_password_needs_rehash_default_bcrypt_bytes(&hash)
                    .map_err(|_| "Unsupported rehash input")?
            )
        }
        "wp_check_password" => {
            let PhpValue::Bytes(password) = input else {
                return Err("WP bcrypt fixture requires byte passwords".into());
            };
            let PhpValue::Bytes(hash) =
                decode(case.get("password_hash").ok_or("missing WP bcrypt hash")?)?
            else {
                return Err("WP bcrypt fixture requires byte hashes".into());
            };
            if case.get("password_profile").and_then(Value::as_str) != Some("wp_bcrypt_empty") {
                return Err("WP bcrypt fixture requires explicit empty profile".into());
            }
            let cost = case
                .get("max_cost")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or("WP bcrypt fixture requires typed host max cost")?;
            let maximum = recastpress_wp::HostMaxBcryptCost::new(cost)
                .map_err(|_| "invalid host max cost")?;
            json!(
                recastpress_wp::wp_check_password_wp_bcrypt_bytes(&password, &hash, maximum)
                    .map_err(|error| error.to_string())?
            )
        }
        api @ ("esc_html" | "esc_attr") => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("Escaping fixture requires byte-string input".into());
            };
            if case.get("filter_profile").and_then(Value::as_str) != Some("empty") {
                return Err("Escaping fixture requires explicit empty filter profile".into());
            }
            encode(&PhpValue::Bytes(
                (if api == "esc_html" {
                    recastpress_wp::esc_html_utf8_empty_filters_bytes(&bytes, 65536)
                } else {
                    recastpress_wp::esc_attr_utf8_empty_filters_bytes(&bytes, 65536)
                })
                .map_err(|error| error.to_string())?,
            ))?
        }
        "_wp_specialchars" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("Specialchars fixture requires byte-string input".into());
            };
            use recastpress_wp::SpecialCharsQuoteStyle as Style;
            let style = match case.get("quote_style").and_then(Value::as_str) {
                Some("noquotes") => Style::NoQuotes,
                Some("compat") => Style::Compat,
                Some("quotes") => Style::Quotes,
                Some("single") => Style::Single,
                Some("double") => Style::Double,
                Some("xml1") => Style::Xml1,
                _ => {
                    return Err(
                        "Specialchars fixture requires an explicit selected quote style".into(),
                    );
                }
            };
            let double = case
                .get("double_encode")
                .and_then(Value::as_bool)
                .ok_or("Specialchars fixture requires an explicit boolean double_encode")?;
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_specialchars_utf8_bytes(&bytes, style, double, 65536)
                    .map_err(|error| error.to_string())?,
            ))?
        }
        "wp_kses_normalize_entities" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err(
                    "Entity fixture requires byte-string input and explicit HTML context".into(),
                );
            };
            let context = case.get("context").and_then(Value::as_str);
            if !matches!(context, Some("html" | "xml")) {
                return Err(
                    "Entity fixture requires byte-string input and explicit HTML context".into(),
                );
            }
            encode(&PhpValue::Bytes(
                (if context == Some("xml") {
                    recastpress_wp::wp_kses_normalize_entities_default_xml_utf8_bytes(&bytes, 65536)
                } else {
                    recastpress_wp::wp_kses_normalize_entities_default_html_bytes(&bytes, 65536)
                })
                .map_err(|_| "entity normalization host output allowance exceeded")?,
            ))?
        }
        "valid_unicode" => {
            let PhpValue::Integer(value) = input else {
                return Err(
                    "Unicode character fixture requires signed 64-bit integer input".into(),
                );
            };
            json!(recastpress_wp::valid_unicode_i64(value))
        }
        "wp_convert_hr_to_bytes" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("Byte conversion fixture requires ASCII digits and an optional single K M G suffix, at most 256 significant digits".into());
            };
            let bytes = recastpress_wp::wp_convert_hr_to_bytes_default_units_bytes(&bytes)
                .map_err(|_| "Byte conversion fixture requires ASCII digits and an optional single K M G suffix, at most 256 significant digits")?;
            encode(&PhpValue::Integer(bytes))?
        }
        "zeroise" => {
            if !matches!(input, PhpValue::Integer(_) | PhpValue::Bytes(_)) {
                return Err("Zeroise fixture requires integer or byte-string input".into());
            }
            let width = case
                .get("width")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|width| *width <= 4096)
                .ok_or("Zeroise fixture requires integer width from 0 through 4096")?;
            encode(&PhpValue::Bytes(
                recastpress_wp::zeroise(&input, width, 65536)
                    .map_err(|_| "unsupported zeroise input or host output allowance")?,
            ))?
        }
        "wp_is_json_request" | "wp_is_xml_request" => {
            if !matches!(input, PhpValue::Null) {
                return Err("Media request fixture requires a null input sentinel".into());
            }
            let accept = request_header(case, "accept")?;
            let content_type = request_header(case, "content_type")?;
            json!(if text(case, "api")? == "wp_is_json_request" {
                recastpress_wp::wp_is_json_request_c_bytes(
                    accept.as_deref(),
                    content_type.as_deref(),
                )
            } else {
                recastpress_wp::wp_is_xml_request_bytes(accept.as_deref(), content_type.as_deref())
            })
        }
        "wp_is_json_media_type" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported JSON media type input".into());
            };
            json!(recastpress_wp::wp_is_json_media_type_c_bytes(&bytes))
        }
        "is_utf8_charset" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported explicit charset input".into());
            };
            json!(recastpress_wp::is_utf8_charset_bytes(&bytes))
        }
        "wp_check_invalid_utf8" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported UTF-8 checking input".into());
            };
            let strip = case
                .get("strip")
                .and_then(Value::as_bool)
                .ok_or("UTF-8 checking fixture requires explicit boolean strip")?;
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_check_invalid_utf8_utf8_no_mbstring_bytes(&bytes, strip),
            ))?
        }
        "wp_check_jsonp_callback" => json!(recastpress_wp::wp_check_jsonp_callback(&input)),
        "checked" | "selected" | "disabled" | "wp_readonly" => {
            if case["display"].as_bool() != Some(false) {
                return Err("form attribute comparison requires display=false".into());
            }
            let current = decode(&case["current"])?;
            let bytes = match text(case, "api")? {
                "checked" => recastpress_wp::checked_return(&input, &current),
                "selected" => recastpress_wp::selected_return(&input, &current),
                "disabled" => recastpress_wp::disabled_return(&input, &current),
                _ => recastpress_wp::wp_readonly_return(&input, &current),
            }
            .map_err(|_| "unsupported form attribute string cast")?;
            encode(&PhpValue::Bytes(bytes))?
        }
        "sanitize_hex_color" | "sanitize_hex_color_no_hash" | "maybe_hash_hex_color" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported color input".into());
            };
            let output = match text(case, "api")? {
                "sanitize_hex_color" => recastpress_wp::sanitize_hex_color_bytes(&bytes)
                    .map_or(PhpValue::Null, PhpValue::Bytes),
                "sanitize_hex_color_no_hash" => {
                    recastpress_wp::sanitize_hex_color_no_hash_bytes(&bytes)
                        .map_or(PhpValue::Null, PhpValue::Bytes)
                }
                _ => PhpValue::Bytes(recastpress_wp::maybe_hash_hex_color_bytes(&bytes)),
            };
            encode(&output)?
        }
        "wp_make_link_relative" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported relative link input".into());
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_make_link_relative_bytes(&bytes),
            ))?
        }
        "url_shorten" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported shortened URL input".into());
            };
            let length = text(case, "length")?
                .parse::<i64>()
                .map_err(|_| "unsupported URL length")?;
            encode(&PhpValue::Bytes(
                recastpress_wp::url_shorten_bytes(&bytes, length)
                    .map_err(|_| "unsupported shortened URL domain")?,
            ))?
        }
        "wp_replace_in_html_tags" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported HTML replacement input".into());
            };
            let PhpValue::Array(pairs) = decode(&case["replacements"])? else {
                return Err("unsupported HTML replacement map".into());
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::wp_replace_in_html_tags_bytes(&bytes, &pairs)
                    .map_err(|_| "unsupported HTML replacement domain")?,
            ))?
        }
        "wp_html_split" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported HTML split input".into());
            };
            let mut output = recastpress_wp::value::PhpArray::new();
            for (index, part) in recastpress_wp::wp_html_split_bytes(&bytes)
                .into_iter()
                .enumerate()
            {
                output.insert(
                    PhpKey::Integer(i64::try_from(index).map_err(|_| "HTML split key overflow")?),
                    PhpValue::Bytes(part),
                );
            }
            encode(&PhpValue::Array(output))?
        }
        "normalize_whitespace" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported whitespace input".into());
            };
            encode(&PhpValue::Bytes(
                recastpress_wp::normalize_whitespace_bytes(&bytes),
            ))?
        }
        "wp_is_uuid" => {
            let version = if case["version"].is_null() {
                None
            } else {
                Some(
                    text(case, "version")?
                        .parse::<i64>()
                        .map_err(|_| "unsupported UUID version")?,
                )
            };
            json!(
                recastpress_wp::wp_is_uuid(&input, version)
                    .map_err(|_| "unsupported UUID version diagnostic")?
            )
        }
        "wp_validate_boolean" => {
            json!(wp_validate_boolean(&input).map_err(|_| "unsupported value")?)
        }
        "wp_is_numeric_array" => json!(wp_is_numeric_array(&input)),
        "wp_json_encode" => {
            let flags = if let Some(flags) = case.get("flags") {
                let bits = flags
                    .as_u64()
                    .and_then(|bits| u32::try_from(bits).ok())
                    .ok_or("unsupported JSON flag encoding")?;
                recastpress_wp::JsonFlags::from_bits(bits)
                    .map_err(|error| format!("unsupported JSON flags: {error:?}"))?
            } else {
                recastpress_wp::JsonFlags::default()
            };
            let output = match case.get("utf8_profile") {
                None if case.get("flags").is_none() => {
                    recastpress_wp::wp_json_encode(&input, usize::MAX)
                }
                None => recastpress_wp::wp_json_encode_with_flags(&input, flags, usize::MAX),
                Some(profile) if profile.as_str() == Some("utf8_no_mbstring") => {
                    recastpress_wp::wp_json_encode_utf8_no_mbstring(&input, flags, usize::MAX)
                }
                Some(_) => return Err("unsupported explicit JSON UTF-8 profile".into()),
            }
            .map_err(|error| format!("unsupported JSON input/output: {error:?}"))?;
            encode(&PhpValue::Bytes(output))?
        }
        "maybe_unserialize" => {
            encode(&maybe_unserialize(&input).map_err(|_| "unsupported unserialization domain")?)?
        }
        "maybe_serialize" => {
            encode(&maybe_serialize(&input).map_err(|_| "unsupported serialization domain")?)?
        }
        "absint" => encode(&absint(&input).map_err(|_| "unsupported absint input")?)?,
        "wp_parse_id_list" => encode(&PhpValue::Array(
            wp_parse_id_list(&input).map_err(|_| "unsupported ID list input/scalar")?,
        ))?,
        "is_serialized" => json!(
            is_serialized(
                &input,
                case.get("strict")
                    .and_then(Value::as_bool)
                    .ok_or("missing strict flag")?
            )
            .map_err(|_| "unsupported serialized input")?
        ),
        "is_serialized_string" => {
            json!(is_serialized_string(&input).map_err(|_| "unsupported serialized string input")?)
        }
        "wp_checkdate" => {
            let PhpValue::Bytes(source) = input else {
                return Err("unsupported date source".into());
            };
            let month = text(case, "month")?
                .parse::<i64>()
                .map_err(|_| "unsupported month")?;
            let day = text(case, "day")?
                .parse::<i64>()
                .map_err(|_| "unsupported day")?;
            let year = text(case, "year")?
                .parse::<i64>()
                .map_err(|_| "unsupported year")?;
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &wp_checkdate(&mut dispatcher, month, day, year, &source)
                    .map_err(|error| format!("filter failed: {error:?}"))?,
            )?
        }
        "wp_is_valid_utf8" | "wp_scrub_utf8" | "wp_has_noncharacters" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported UTF-8 input".into());
            };
            match text(case, "api")? {
                "wp_is_valid_utf8" => json!(recastpress_wp::wp_is_valid_utf8_bytes(&bytes)),
                "wp_has_noncharacters" => json!(recastpress_wp::wp_has_noncharacters_bytes(&bytes)),
                _ => encode(&PhpValue::Bytes(recastpress_wp::wp_scrub_utf8_bytes(
                    &bytes,
                )))?,
            }
        }
        "wp_basename" => {
            let PhpValue::Bytes(path) = input else {
                return Err("unsupported basename path".into());
            };
            let PhpValue::Bytes(suffix) = decode(&case["suffix"])? else {
                return Err("unsupported basename suffix".into());
            };
            encode(&PhpValue::Bytes(recastpress_wp::wp_basename_bytes(
                &path, &suffix,
            )))?
        }
        "trailingslashit" | "untrailingslashit" | "wp_normalize_path" => {
            let PhpValue::Bytes(input) = input else {
                return Err("unsupported path input".into());
            };
            let output = match text(case, "api")? {
                "trailingslashit" => trailingslashit_bytes(&input),
                "untrailingslashit" => untrailingslashit_bytes(&input),
                _ => wp_normalize_path_bytes(&input).map_err(|_| "unsupported stream path")?,
            };
            encode(&PhpValue::Bytes(output))?
        }
        "wp_list_pluck" => {
            let field = decode_key(&case["field"])?;
            let index = if case["index"].is_null() {
                None
            } else {
                Some(decode_key(&case["index"])?)
            };
            encode(&PhpValue::Array(
                wp_list_pluck(&input, &field, index.as_ref())
                    .map_err(|_| "unsupported pluck input")?,
            ))?
        }
        "wp_slash" => encode(&wp_slash(&input).map_err(|_| "unsupported slash value")?)?,
        "wp_unslash" => encode(&wp_unslash(&input).map_err(|_| "unsupported unslash value")?)?,
        "wp_parse_list" => encode(&PhpValue::Array(
            wp_parse_list(&input).map_err(|_| "unsupported parse list input")?,
        ))?,
        "wp_parse_str" => {
            let PhpValue::Bytes(bytes) = input else {
                return Err("unsupported query input".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &recastpress_wp::wp_parse_str(&mut dispatcher, &bytes)
                    .map_err(|error| format!("parse failed: {error:?}"))?,
            )?
        }
        "wp_parse_args" => {
            let PhpValue::Array(defaults) = decode(&case["defaults"])? else {
                return Err("unsupported defaults".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &recastpress_wp::wp_parse_args(&mut dispatcher, &input, &defaults)
                    .map_err(|error| format!("parse failed: {error:?}"))?,
            )?
        }
        "wp_array_slice_assoc" => {
            let PhpValue::Array(input) = input else {
                return Err("unsupported slice input".into());
            };
            let PhpValue::Array(keys) = decode(&case["keys"])? else {
                return Err("unsupported slice keys".into());
            };
            let keys: Result<Vec<_>, String> = keys
                .iter()
                .map(|(_, key)| match key {
                    PhpValue::Integer(value) => Ok(PhpKey::Integer(*value)),
                    PhpValue::Bytes(value) => Ok(PhpKey::from_bytes(value)),
                    _ => Err("unsupported slice key".into()),
                })
                .collect();
            encode(&PhpValue::Array(
                wp_array_slice_assoc(&input, &keys?).map_err(|_| "unsupported slice value")?,
            ))?
        }
        "sanitize_key" => {
            let PhpValue::Bytes(input) = input else {
                return Err("unsupported sanitize input".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &sanitize_key(&mut dispatcher, &input)
                    .map_err(|error| format!("filter failed: {error:?}"))?,
            )?
        }
        "sanitize_locale_name" | "sanitize_mime_type" => {
            let PhpValue::Bytes(input) = input else {
                return Err("unsupported locale/MIME input".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            let output = if text(case, "api")? == "sanitize_locale_name" {
                recastpress_wp::sanitize_locale_name(&mut dispatcher, &input)
            } else {
                recastpress_wp::sanitize_mime_type(&mut dispatcher, &input)
            }
            .map_err(|error| format!("filter failed: {error:?}"))?;
            encode(&output)?
        }
        "wp_unique_id" | "wp_unique_prefixed_id" => {
            let PhpValue::Bytes(prefix) = input else {
                return Err("unsupported unique ID prefix".into());
            };
            let output = if text(case, "api")? == "wp_unique_id" {
                unique_ids.wp_unique_id(&prefix)
            } else {
                unique_ids.wp_unique_prefixed_id(&prefix)
            }
            .map_err(|_| "unsupported unique ID counter overflow")?;
            encode(&PhpValue::Bytes(output))?
        }
        "is_email" => {
            let PhpValue::Bytes(input) = input else {
                return Err("unsupported email input".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &recastpress_wp::is_email(&mut dispatcher, &input)
                    .map_err(|error| format!("filter failed: {error:?}"))?,
            )?
        }
        "sanitize_html_class" => {
            let PhpValue::Bytes(input) = input else {
                return Err("unsupported HTML class input".into());
            };
            let PhpValue::Bytes(fallback) = decode(&case["fallback"])? else {
                return Err("unsupported HTML class fallback".into());
            };
            let mut dispatcher = filter_dispatcher(case)?;
            encode(
                &recastpress_wp::sanitize_html_class(&mut dispatcher, &input, &fallback)
                    .map_err(|error| format!("filter failed: {error:?}"))?,
            )?
        }
        _ => return Err("unknown API".into()),
    };
    Ok(json!({"id": text(case, "id")?, "value": value}))
}

fn filter_dispatcher(case: &Value) -> Result<Dispatcher, String> {
    let mut dispatcher = Dispatcher::new();
    let mode = case.get("filter").and_then(Value::as_str).unwrap_or("");
    let class_arguments = mode == "class_arguments" || mode == "email_arguments";
    if !mode.is_empty() {
        let constant = if mode == "constant" {
            Some(decode(&case["filter_return"])?)
        } else {
            None
        };
        if mode != "constant"
            && mode != "append_raw"
            && mode != "class_arguments"
            && mode != "email_arguments"
        {
            return Err("unknown filter mode".into());
        }
        dispatcher.add(
            if text(case, "api")? == "wp_parse_args" {
                "wp_parse_str"
            } else {
                text(case, "api")?
            },
            10,
            DispatcherCallback::new(
                CallbackId {
                    module: "fixture".into(),
                    callback: "filter".into(),
                },
                if class_arguments { 3 } else { 2 },
                move |_, args| {
                    if class_arguments {
                        let mut values = recastpress_wp::value::PhpArray::new();
                        for (index, value) in args.iter().enumerate() {
                            values.insert(PhpKey::Integer(index as i64), value.clone());
                        }
                        return Ok(PhpValue::Array(values));
                    }
                    if let Some(value) = &constant {
                        return Ok(value.clone());
                    }
                    let mut value = match &args[0] {
                        PhpValue::Bytes(value) => value.clone(),
                        PhpValue::Bool(value) => vec![if *value { b'1' } else { b'0' }],
                        _ => unreachable!("known adapter input"),
                    };
                    value.push(0);
                    let PhpValue::Bytes(raw) = &args[1] else {
                        unreachable!("known raw bytes")
                    };
                    value.extend_from_slice(raw);
                    Ok(PhpValue::Bytes(value))
                },
            ),
        );
    }
    Ok(dispatcher)
}

fn request_header(case: &Value, name: &str) -> Result<Option<Vec<u8>>, String> {
    match case.get(name).map(decode).transpose()? {
        None | Some(PhpValue::Null) => Ok(None),
        Some(PhpValue::Bytes(bytes)) => Ok(Some(bytes)),
        _ => Err("Media request fixture requires byte-string or null headers".into()),
    }
}

fn decode_key(value: &Value) -> Result<PhpKey, String> {
    match decode(value)? {
        PhpValue::Integer(value) => Ok(PhpKey::Integer(value)),
        PhpValue::Bytes(value) => Ok(PhpKey::from_bytes(&value)),
        _ => Err("unsupported field key".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut input)?;
    if input.len() > 16 * 1024 * 1024 {
        return Err("probe input too large".into());
    }
    let cases: Vec<Value> = serde_json::from_slice(&input)?;
    // Each probe process corresponds to one oracle PHP process/input batch.
    // Both preserve the two functions' independent state across these cases.
    let mut unique_ids = recastpress_wp::UniqueIds::new();
    let result: Result<Vec<_>, _> = cases
        .iter()
        .map(|case| run(case, &mut unique_ids))
        .collect();
    println!(
        "{}",
        serde_json::to_string(&result.map_err(io::Error::other)?)?
    );
    Ok(())
}
