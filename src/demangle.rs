use std::borrow::Cow;

use once_cell::sync::OnceCell;
use owo_colors::OwoColorize;
use regex::{Regex, Replacer};
use rustc_demangle::Demangle;

use crate::color;

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

fn reg() -> &'static Regex {
    static INSTANCE: OnceCell<Regex> = OnceCell::new();
    INSTANCE
        .get_or_init(|| regex::Regex::new(r"_?(_[a-zA-Z0-9_$.]+)").expect("regexp should be valid"))
}

struct Demangler {
    full_name: bool,
}
impl Replacer for Demangler {
    fn replace_append(&mut self, cap: &regex::Captures<'_>, dst: &mut std::string::String) {
        if let Ok(dem) = rustc_demangle::try_demangle(&cap[1]) {
            if self.full_name {
                dst.push_str(&format!("{:?}", color!(dem, OwoColorize::green)));
            } else {
                dst.push_str(&format!("{:#?}", color!(dem, OwoColorize::green)));
            }
        } else {
            dst.push_str(&cap[0]);
        }
    }
}

#[must_use]
pub fn contents(input: &str, full_name: bool) -> Cow<'_, str> {
    reg().replace_all(input, Demangler { full_name })
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
