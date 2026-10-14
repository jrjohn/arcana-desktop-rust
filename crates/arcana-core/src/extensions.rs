//! String helpers from C# `StringExtensions` that have no direct std equivalent.
//!
//! The rest of C# `StringExtensions` / `CollectionExtensions` maps onto std:
//! `IsNullOrWhiteSpace` → `s.trim().is_empty()`, `Batch` → `slice::chunks`,
//! `ForEach` → `for`, `OrEmpty` → `Option::unwrap_or_default`.

/// Shorten `value` to at most `max_length` characters, ending with `suffix` (C# `Truncate`).
///
/// Lengths count `char`s. When `max_length` is shorter than the suffix, C# throws; here the
/// result is the first `max_length` characters of the suffix.
#[must_use]
pub fn truncate(value: &str, max_length: usize, suffix: &str) -> String {
    if value.chars().count() <= max_length {
        return value.to_owned();
    }
    let keep = max_length.saturating_sub(suffix.chars().count());
    let mut out: String = value.chars().take(keep).collect();
    out.extend(suffix.chars().take(max_length - keep));
    out
}

/// Lower-case everything, then upper-case the first letter of each word (C# `ToTitleCase`).
#[must_use]
pub fn to_title_case(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut at_word_start = true;
    for c in value.chars() {
        if c.is_alphanumeric() {
            if at_word_start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(c);
            at_word_start = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_cases() {
        assert_eq!(truncate("", 3, "..."), "");
        assert_eq!(truncate("hello", 5, "..."), "hello");
        assert_eq!(truncate("hello world", 8, "..."), "hello...");
        assert_eq!(truncate("hello world", 2, "..."), "..");
    }

    #[test]
    fn title_case_cases() {
        assert_eq!(to_title_case(""), "");
        assert_eq!(to_title_case("hELLO wORLD"), "Hello World");
        assert_eq!(to_title_case("a-b c"), "A-B C");
    }
}
