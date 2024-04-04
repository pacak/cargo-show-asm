use crate::{color, opts::NameDisplay};
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
// however it's important to detect them, so they can be colored and possibly removed
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
    Lazy::new(|| Regex::new(GLOBAL_LABELS_REGEX).expect("regexp should be valid"));

pub(crate) static LOCAL_LABELS: Lazy<Regex> =
    Lazy::new(|| Regex::new(LOCAL_LABELS_REGEX).expect("regexp should be valid"));

static LABEL_KINDS: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new([LOCAL_LABELS_REGEX, GLOBAL_LABELS_REGEX, TEMP_LABELS_REGEX])
        .expect("regexp should be valid")
});

static COMMENT_ARGS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:\s|^)(#.+)").expect("regexp should be valid"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Global,
    Local,
    Temp,
    Unknown,
}

pub fn local_labels(input: &str) -> regex::Matches {
    LOCAL_LABELS.find_iter(input)
}

#[must_use]
pub fn label_kind(input: &str) -> LabelKind {
    match LABEL_KINDS.matches(input).into_iter().next() {
        Some(1) => LabelKind::Global,
        Some(0) => LabelKind::Local,
        Some(2) => LabelKind::Temp,
        _ => LabelKind::Unknown,
    }
}

struct LabelColorizer;
impl Replacer for LabelColorizer {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        use std::fmt::Write;
        write!(dst, "{}", color!(&caps[0], OwoColorize::bright_yellow)).unwrap();
    }
}

pub fn color_local_labels(input: &str) -> Cow<'_, str> {
    LOCAL_LABELS.replace_all(input, LabelColorizer)
}

struct CommentColorizer;
impl Replacer for CommentColorizer {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        use std::fmt::Write;
        write!(dst, "{}", color!(&caps[0], OwoColorize::blue)).unwrap();
    }
}

pub fn color_comment(input: &str) -> Cow<'_, str> {
    COMMENT_ARGS.replace_all(input, CommentColorizer)
}

struct Demangler {
    display: NameDisplay,
}
impl Replacer for Demangler {
    fn replace_append(&mut self, cap: &regex::Captures<'_>, dst: &mut String) {
        if let Ok(dem) = rustc_demangle::try_demangle(&cap[1]) {
            use std::fmt::Write;
            match self.display {
                NameDisplay::Full => {
                    write!(dst, "{:?}", color!(dem, OwoColorize::green)).unwrap();
                }
                NameDisplay::Short => {
                    write!(dst, "{:#?}", color!(dem, OwoColorize::green)).unwrap();
                }
                NameDisplay::Mangled => {
                    write!(dst, "{}", color!(&cap[1], OwoColorize::green)).unwrap();
                }
            }
        } else {
            dst.push_str(&cap[0]);
        }
    }
}

#[must_use]
pub fn contents(input: &str, display: NameDisplay) -> Cow<'_, str> {
    GLOBAL_LABELS.replace_all(input, Demangler { display })
}

#[must_use]
pub fn global_reference(input: &str) -> Option<&str> {
    GLOBAL_LABELS.find(input).map(|m| m.as_str())
}

#[cfg(test)]
mod test {
    use owo_colors::set_override;

    use crate::opts::NameDisplay;

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
    fn linux_no_demangle_call() {
        set_override(true);
        let x = contents(CALL_L, NameDisplay::Mangled);
        assert_eq!(
            "[rip + \u{1b}[32m_ZN58_$LT$nom..error..ErrorKind$u20$as$u20$core..fmt..Debug$GT$3fmt17hb98704099c11c31fE\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn linux_demangle_call() {
        set_override(true);
        let x = contents(CALL_L, NameDisplay::Short);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn mac_demangle_call() {
        set_override(true);
        let x = contents(CALL_M, NameDisplay::Short);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn mac_demangle_call2() {
        set_override(true);
        let x = contents(CALL_M, NameDisplay::Full);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt::hb98704099c11c31f\u{1b}[39m]",
            x
        );
    }
}
