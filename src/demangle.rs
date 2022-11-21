use crate::color;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use regex::{Regex, RegexSet, Replacer};
use rustc_demangle::Demangle;
use std::borrow::Cow;

#[must_use]
pub fn name(input: &str) -> Option<String> {
    Some(format!("{:#?}", demangled(input)?))
}

#[must_use]
pub fn demangled(input: &str) -> Option<Demangle> {
    let name = if input.starts_with("__") {
        #[allow(clippy::string_slice)]
        rustc_demangle::try_demangle(&input[1..]).ok()?
    } else {
        rustc_demangle::try_demangle(input).ok()?
    };
    Some(name)
}

const GLOBAL_LABELS_REGEX: &str = r"\b_?(_[a-zA-Z0-9_$\.]+)";

// This regex is two parts
// 1. \.L[a-zA-Z0-9_$\.]+
// 2. LBB[0-9_]+
// Label kind 1. is a standard label format for GCC and Clang (LLVM)
// Label kinds 2. was detected in the wild, and don't seem to be a normal label format
// however it's important to detect them so they can be colored and possibly removed
//
// Note on `(?:[^\w\d\$\.]|^)`. This is to prevent the label from matching in the middle of some other word
// since \b doesn't match before a `.` we can't use \b. So instead we're using a negation of any character
// that could come up in the label OR the beginning of the text. It's not matching because we don't care what's
// there  as long as it doesn't look like a label.
//
// Note: this rejects "labels" like `H.Lfoo` but accepts `.Lexception` and `[some + .Label]`
const LOCAL_LABELS_REGEX: &str = r"(?:[^\w\d\$\.]|^)(\.L[a-zA-Z0-9_\$\.]+|\bLBB[0-9_]+)";

// temporary labels
const TEMP_LABELS_REGEX: &str = r"\b(Ltmp[0-9]+)\b";

static GLOBAL_LABELS: Lazy<Regex> =
    Lazy::new(|| regex::Regex::new(GLOBAL_LABELS_REGEX).expect("regexp should be valid"));

static LOCAL_LABELS: Lazy<Regex> =
    Lazy::new(|| regex::Regex::new(LOCAL_LABELS_REGEX).expect("regexp should be valid"));

static LABEL_KINDS: Lazy<RegexSet> = Lazy::new(|| {
    regex::RegexSet::new([LOCAL_LABELS_REGEX, GLOBAL_LABELS_REGEX, TEMP_LABELS_REGEX])
        .expect("regexp should be valid")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Gobal,
    Local,
    Temp,
    Unknown,
}

pub fn local_labels(input: &str) -> regex::Matches {
    LOCAL_LABELS.find_iter(input)
}

pub fn label_kind(input: &str) -> LabelKind {
    match LABEL_KINDS.matches(input).into_iter().next() {
        Some(1) => LabelKind::Gobal,
        Some(0) => LabelKind::Local,
        Some(2) => LabelKind::Temp,
        _ => LabelKind::Unknown,
    }
}

struct LabelColorizer;
impl Replacer for LabelColorizer {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        use std::fmt::Write;
        write!(dst, "{}", color!(&caps[0], OwoColorize::bright_black)).unwrap();
    }
}

pub fn color_local_labels(input: &str) -> Cow<'_, str> {
    LOCAL_LABELS.replace_all(input, LabelColorizer)
}

struct Demangler {
    full_name: bool,
}
impl Replacer for Demangler {
    fn replace_append(&mut self, cap: &regex::Captures<'_>, dst: &mut std::string::String) {
        if let Ok(dem) = rustc_demangle::try_demangle(&cap[1]) {
            use std::fmt::Write;
            if self.full_name {
                write!(dst, "{:?}", color!(dem, OwoColorize::green)).unwrap();
            } else {
                write!(dst, "{:#?}", color!(dem, OwoColorize::green)).unwrap();
            }
        } else {
            dst.push_str(&cap[0]);
        }
    }
}

#[must_use]
pub fn contents(input: &str, full_name: bool) -> Cow<'_, str> {
    GLOBAL_LABELS.replace_all(input, Demangler { full_name })
}

#[cfg(test)]
mod test {
    use owo_colors::set_override;

    use super::{contents, name};
    const MAC: &str =
        "__ZN58_$LT$nom..error..ErrorKind$u20$as$u20$core..fmt..Debug$GT$3fmt17hb98704099c11c31fE";
    const LINUX: &str =
        "_ZN58_$LT$nom..error..ErrorKind$u20$as$u20$core..fmt..Debug$GT$3fmt17hb98704099c11c31fE";
    const CALL_M: &str = "[rip + __ZN58_$LT$nom..error..ErrorKind$u20$as$u20$core..fmt..Debug$GT$3fmt17hb98704099c11c31fE]";
    const CALL_L: &str = "[rip + _ZN58_$LT$nom..error..ErrorKind$u20$as$u20$core..fmt..Debug$GT$3fmt17hb98704099c11c31fE]";

    #[test]
    fn linux_demangle() {
        assert!(name(LINUX).is_some());
    }

    #[test]
    fn mac_demangle() {
        assert!(name(MAC).is_some());
    }

    #[test]
    fn linux_demangle_call() {
        set_override(true);
        let x = contents(CALL_L, false);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn mac_demangle_call() {
        set_override(true);
        let x = contents(CALL_M, false);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn mac_demangle_call2() {
        set_override(true);
        let x = contents(CALL_M, true);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt::hb98704099c11c31f\u{1b}[39m]",
            x
        );
    }
}
