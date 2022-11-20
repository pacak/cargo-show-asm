use crate::color;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use regex::{Regex, Replacer};
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

static GLOBAL_LABELS: Lazy<Regex> =
    Lazy::new(|| regex::Regex::new(r"_?(_[a-zA-Z0-9_$\.]+)").expect("regexp should be valid"));

static LOCAL_LABELS: Lazy<Regex> = Lazy::new(|| {
    // This regex is three parts
    // 1. \.L[a-zA-Z0-9_$\.]+
    // 2. Ltmp[0-9]+
    // 3. LBB[0-9_]+
    // Label kind 1. is a standard label format for GCC and Clang (LLVM)
    // Label kinds 2. and 3. were detected in the wild, and don't seem to be a normal label format
    // however it's important to detect them so they can be colored and possibily removed
    regex::Regex::new(r"(\.L[a-zA-Z0-9_$\.]+|Ltmp[0-9]+|LBB[0-9_]+)")
        .expect("regexp should be valid")
});

pub fn local_labels(input: &str) -> regex::Matches {
    LOCAL_LABELS.find_iter(input)
}

struct LabelColorizer;
impl Replacer for LabelColorizer {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        use std::fmt::Write;
        write!(dst, "{}", color!(&caps[0], OwoColorize::bright_black)).unwrap();
    }
}

pub fn color_labels(input: &str) -> Cow<'_, str> {
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
