//! Return-only subsets of WordPress 7.1.3 general-template.php:5515..5610.
//!
//! The shared source helper explicitly casts both values to strings before
//! strict comparison. These APIs model only display=false; PHP's default echo
//! behavior and stdout transport are not implemented. No arbitrary attribute
//! name is accepted, and no general HTML escaping/authorization is provided.

use crate::{CompatibilityError, value::PhpValue};
use std::borrow::Cow;

pub fn checked_return(
    helper: &PhpValue,
    current: &PhpValue,
) -> Result<Vec<u8>, CompatibilityError> {
    attribute_return(helper, current, Attribute::Checked)
}

pub fn selected_return(
    helper: &PhpValue,
    current: &PhpValue,
) -> Result<Vec<u8>, CompatibilityError> {
    attribute_return(helper, current, Attribute::Selected)
}

pub fn disabled_return(
    helper: &PhpValue,
    current: &PhpValue,
) -> Result<Vec<u8>, CompatibilityError> {
    attribute_return(helper, current, Attribute::Disabled)
}

pub fn wp_readonly_return(
    helper: &PhpValue,
    current: &PhpValue,
) -> Result<Vec<u8>, CompatibilityError> {
    attribute_return(helper, current, Attribute::Readonly)
}

enum Attribute {
    Checked,
    Selected,
    Disabled,
    Readonly,
}

fn supported(value: &PhpValue) -> bool {
    matches!(
        value,
        PhpValue::Null | PhpValue::Bool(_) | PhpValue::Integer(_) | PhpValue::Bytes(_)
    )
}

fn attribute_return(
    helper: &PhpValue,
    current: &PhpValue,
    attribute: Attribute,
) -> Result<Vec<u8>, CompatibilityError> {
    // Validate both domains before casting/comparison; an unsupported value is
    // never hidden by an early mismatch or equality shortcut. No PHP diagnostic
    // is silently discarded for float/array/object/resource/reference inputs.
    let helper_supported = supported(helper);
    let current_supported = supported(current);
    if !helper_supported || !current_supported {
        return Err(CompatibilityError::UnsupportedValue);
    }
    let left = string_cast(helper);
    let right = string_cast(current);
    if left != right {
        return Ok(Vec::new());
    }
    Ok(match attribute {
        Attribute::Checked => b" checked='checked'".to_vec(),
        Attribute::Selected => b" selected='selected'".to_vec(),
        Attribute::Disabled => b" disabled='disabled'".to_vec(),
        Attribute::Readonly => b" readonly='readonly'".to_vec(),
    })
}

fn string_cast(value: &PhpValue) -> Cow<'_, [u8]> {
    match value {
        PhpValue::Null | PhpValue::Bool(false) => Cow::Borrowed(b""),
        PhpValue::Bool(true) => Cow::Borrowed(b"1"),
        PhpValue::Integer(integer) => Cow::Owned(integer.to_string().into_bytes()),
        PhpValue::Bytes(bytes) => Cow::Borrowed(bytes),
        _ => unreachable!("both values are validated before casting"),
    }
}
