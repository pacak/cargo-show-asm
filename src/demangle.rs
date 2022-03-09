use std::borrow::Cow;

use once_cell::sync::OnceCell;
use owo_colors::OwoColorize;
use regex::{Regex, Replacer};

#[must_use]
pub fn name(input: &str) -> Option<String> {
    let name = if input.starts_with("__") {
        #[allow(clippy::string_slice)]
        rustc_demangle::try_demangle(&input[1..]).ok()?
    } else {
        rustc_demangle::try_demangle(input).ok()?
    };
    Some(format!("{name:#?}"))
}

fn reg() -> &'static Regex {
    static INSTANCE: OnceCell<Regex> = OnceCell::new();
    INSTANCE
        .get_or_init(|| regex::Regex::new(r"_?(_[a-zA-Z0-9_$.]+)").expect("regexp should be valid"))
}

struct Demangle(bool);
impl Replacer for Demangle {
    fn replace_append(&mut self, cap: &regex::Captures<'_>, dst: &mut std::string::String) {
        if let Ok(dem) = rustc_demangle::try_demangle(&cap[1]) {
            let demangled = if self.0 {
                format!("{:#?}", dem.green())
            } else {
                format!("{:#?}", dem)
            };
            dst.push_str(&demangled);
        } else {
            dst.push_str(&cap[0]);
        }
    }
}

#[must_use]
pub fn contents(input: &str, color: bool) -> Cow<'_, str> {
    reg().replace_all(input, Demangle(color))
}

#[cfg(test)]
mod test {
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
        let x = contents(CALL_L, true);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }

    #[test]
    fn mac_demangle_call() {
        let x = contents(CALL_M, true);
        assert_eq!(
            "[rip + \u{1b}[32m<nom::error::ErrorKind as core::fmt::Debug>::fmt\u{1b}[39m]",
            x
        );
    }
}
