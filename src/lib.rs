#![doc = include_str!("../README.md")]

use std::collections::BTreeMap;
pub mod asm;
pub mod cached_lines;
pub mod demangle;
pub mod llvm;
pub mod mir;
pub mod opts;

#[macro_export]
macro_rules! color {
    ($item:expr, $color:expr) => {
        owo_colors::OwoColorize::if_supports_color(&$item, owo_colors::Stream::Stdout, $color)
    };
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Item {
    /// demangled name
    pub name: String,
    /// demangled name with hash
    pub hashed: String,
    /// sequential number of demangled name
    pub index: usize,
    /// number of lines
    pub len: usize,
}

pub fn suggest_name<'a>(
    search: &str,
    full: bool,
    items: impl IntoIterator<Item = &'a Item>,
) -> anyhow::Result<()> {
    let mut count = 0;
    let names = items.into_iter().fold(BTreeMap::new(), |mut m, item| {
        count += 1;
        m.entry(if full { &item.hashed } else { &item.name })
            .or_insert_with(Vec::new)
            .push(item.len);
        m
    });

    if names.is_empty() {
        #[allow(clippy::redundant_else)]
        if search.is_empty() {
            anyhow::bail!("This target defines no functions")
        } else {
            anyhow::bail!("No matching functions, try relaxing your search request")
        }
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    let width = (count as f64).log10().ceil() as usize;

    println!("Try one of those by name or a sequence number");
    let mut ix = 0;
    for (name, lens) in names.iter() {
        println!(
            "{ix:width$} {:?} {:?}",
            color!(name, owo_colors::OwoColorize::green),
            color!(lens, owo_colors::OwoColorize::cyan),
        );
        ix += lens.len();
    }

    std::process::exit(1);
}
