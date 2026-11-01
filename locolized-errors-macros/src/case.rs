//! Identifier casing helpers used to derive i18n translation keys.
//!
//! The macro converts enum and variant names (`PascalCase`, `snake_case`,
//! `SCREAMING_SNAKE_CASE`) into kebab-case segments of translation keys, e.g.
//! `SwitchAlreadyTriggered` becomes `switch-already-triggered`.

/// Split boundaries of a `PascalCase` / `snake_case` identifier.
fn is_delimiter(c: char) -> bool {
    matches!(c, '_' | '-' | ' ')
}

/// Classify a character into the categories relevant for word splitting:
/// `0` = lower, `1` = upper, `2` = digit.
fn char_kind(c: char) -> u8 {
    if c.is_ascii_digit() {
        2
    } else {
        u8::from(c.is_uppercase())
    }
}

/// Convert an identifier to `kebab-case`.
///
/// Word boundaries are detected between lower->upper transitions and inside
/// acronym runs that are followed by a capitalized word (`XMLHttpRequest`).
/// Digits glue to their neighbouring word (`Base64`, `V2Rocket`), matching the
/// behaviour of the `convert_case` crate this helper replaces. Delimiters
/// (`_`, `-`, spaces) are collapsed. Non-ASCII letters participate like their
/// ASCII counterparts.
#[must_use]
pub(crate) fn to_kebab_case(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 4);
    let mut started = false;
    let mut prev_kind: Option<u8> = None;

    for (index, &current) in chars.iter().enumerate() {
        if is_delimiter(current) {
            prev_kind = None;
            continue;
        }
        let current_kind = char_kind(current);
        let next_is_lower = chars
            .get(index + 1)
            .is_some_and(|next| !is_delimiter(*next) && char_kind(*next) == 0);

        let boundary = match prev_kind {
            None => started,
            // lower -> Upper, or inside an acronym run ending before a
            // capitalized word (`XMLHttp` -> `xml-http`).
            Some(0 | 1) if current_kind == 1 && next_is_lower => true,
            _ => false,
        };

        if boundary && started {
            out.push('-');
        }
        started = true;
        for lower in current.to_lowercase() {
            out.push(lower);
        }
        prev_kind = Some(current_kind);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::to_kebab_case;

    #[test]
    fn converts_common_shapes() {
        for (input, expected) in [
            ("SwitchAlreadyTriggered", "switch-already-triggered"),
            ("UserNotFound", "user-not-found"),
            ("ParseError", "parse-error"),
            ("InternalServerError", "internal-server-error"),
            ("HTTPServerError", "http-server-error"),
            ("XMLHttpRequest", "xml-http-request"),
            ("snake_case", "snake-case"),
            ("SCREAMING_SNAKE", "screaming-snake"),
            ("already-kebab", "already-kebab"),
            ("Base64", "base64"),
            ("V2Rocket", "v2rocket"),
            ("A", "a"),
            ("", ""),
            ("__weird__name__", "weird-name"),
            ("TrailingDelimiter ", "trailing-delimiter"),
            ("ÜnïcodeÜnïcode", "ünïcode-ünïcode"),
        ] {
            assert_eq!(to_kebab_case(input), expected, "input: {input:?}");
        }
    }
}
