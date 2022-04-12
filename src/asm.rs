use crate::{color, demangle};
// TODO, use https://sourceware.org/binutils/docs/as/index.html
use crate::opts::Format;

struct CachedLines {
    content: String,
    splits: Vec<Range<usize>>,
}

impl CachedLines {
    fn without_ending(content: String) -> Self {
        let splits = content.line_spans().map(|s| s.range()).collect::<Vec<_>>();
        Self { splits, content }
    }
}

impl Index<usize> for CachedLines {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        &self.content[self.splits[index].clone()]
    }
}

// {{{
mod statements {
    use nom::branch::alt;
    use nom::bytes::complete::{tag, take_while, take_while1};
    use nom::character::complete;
    use nom::character::complete::{newline, space1};
    use nom::combinator::{consumed, map, opt, verify};
    use nom::sequence::{delimited, preceded, terminated, tuple};
    use nom::*;
    use owo_colors::OwoColorize;

    use crate::{color, demangle};

    #[derive(Clone, Debug)]
    pub enum Statement<'a> {
        Label(Label<'a>),
        Directive(Directive<'a>),
        Instruction(Instruction<'a>),
        Nothing,
    }

    #[derive(Clone, Debug)]
    pub struct Instruction<'a> {
        pub op: &'a str,
        pub args: Option<&'a str>,
    }

    impl<'a> Instruction<'a> {
        pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
            alt((Self::parse_regular, Self::parse_sharp))(input)
        }

        fn parse_sharp(input: &'a str) -> IResult<&'a str, Self> {
            let sharp_tag = tuple((tag("#"), opt(tag("#")), take_while1(|c: char| c != '\n')));
            map(preceded(tag("\t"), consumed(sharp_tag)), |(op, _)| {
                Instruction { op, args: None }
            })(input)
        }

        fn parse_regular(input: &'a str) -> IResult<&'a str, Self> {
            let (input, _) = tag("\t")(input)?;
            let (input, op) = take_while1(AsChar::is_alphanum)(input)?;
            let (input, args) = opt(preceded(space1, take_while1(|c| c != '\n')))(input)?;
            Ok((input, Instruction { op, args }))
        }
    }

    impl std::fmt::Display for Instruction<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", color!(self.op, |t| t.bright_blue()))?;
            if let Some(args) = self.args {
                write!(f, " {}", demangle::contents(args))?
            }
            Ok(())
        }
    }

    impl std::fmt::Display for Statement<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Statement::Label(l) => l.fmt(f),
                Statement::Directive(d) => d.fmt(f),
                Statement::Instruction(i) => write!(f, "\t{i}"),
                Statement::Nothing => Ok(()),
            }
        }
    }

    impl std::fmt::Display for Directive<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Directive::File(ff) => ff.fmt(f),
                Directive::Loc(l) => l.fmt(f),
                Directive::Generic(g) => g.fmt(f),
                Directive::Set(g) => {
                    f.write_str(&format!(".set {}", color!(g, |t| t.bright_black())))
                }
                Directive::SubsectionsViaSym => f.write_str(&format!(
                    ".{}",
                    color!("subsections_via_symbols", |t| t.bright_black())
                )),
            }
        }
    }

    impl std::fmt::Display for File<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "\t.file\t{} {}", self.index, self.name)
        }
    }

    impl std::fmt::Display for GenericDirective<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "\t.{}", color!(self.0, |t| t.bright_black()))
        }
    }

    impl std::fmt::Display for Loc<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.extra {
                Some(x) => write!(
                    f,
                    "\t.loc\t{} {} {} {}",
                    self.file, self.line, self.column, x
                ),
                None => write!(f, "\t.loc\t{} {} {}", self.file, self.line, self.column),
            }
        }
    }

    impl std::fmt::Display for Label<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}:",
                color!(demangle::contents(self.id), |t| t.bright_black())
            )
        }
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct Label<'a> {
        pub id: &'a str,
        pub local: bool,
    }

    impl<'a> Label<'a> {
        pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
            // TODO: label can't start with a digit
            map(
                terminated(take_while1(good_for_label), tag(":")),
                |id: &str| {
                    let local = id.starts_with(".L");
                    Label { id, local }
                },
            )(input)
        }
    }

    #[derive(Copy, Clone, Debug, Eq, Default)]
    pub struct Loc<'a> {
        pub file: u64,
        pub line: u64,
        pub column: u64,
        pub extra: Option<&'a str>,
    }

    impl<'a> PartialEq for Loc<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.file == other.file && self.line == other.line
        }
    }

    impl<'a> Loc<'a> {
        pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
            map(
                tuple((
                    tag("\t.loc\t"),
                    complete::u64,
                    space1,
                    complete::u64,
                    space1,
                    complete::u64,
                    opt(preceded(tag(" "), take_while1(|c| c != '\n'))),
                )),
                |(_, file, _, line, _, column, extra)| Loc {
                    file,
                    line,
                    column,
                    extra,
                },
            )(input)
        }
    }

    #[test]
    fn test_parse_label() {
        assert_eq!(
            Label::parse("GCC_except_table0:"),
            Ok((
                "",
                Label {
                    id: "GCC_except_table0",
                    local: false,
                }
            ))
        );
        assert_eq!(
            Label::parse(".Lexception0:"),
            Ok((
                "",
                Label {
                    id: ".Lexception0",
                    local: true
                }
            ))
        );
    }

    #[test]
    fn test_parse_loc() {
        assert_eq!(
            Loc::parse("\t.loc\t31 26 29"),
            Ok((
                "",
                Loc {
                    file: 31,
                    line: 26,
                    column: 29,
                    extra: None
                }
            ))
        );
        assert_eq!(
            Loc::parse("\t.loc\t31 26 29 is_stmt 0"),
            Ok((
                "",
                Loc {
                    file: 31,
                    line: 26,
                    column: 29,
                    extra: Some("is_stmt 0")
                }
            ))
        );
        assert_eq!(
            Loc::parse("\t.loc\t31 26 29 prologue_end"),
            Ok((
                "",
                Loc {
                    file: 31,
                    line: 26,
                    column: 29,
                    extra: Some("prologue_end")
                }
            ))
        );
    }

    #[derive(Clone, Debug)]
    pub enum Directive<'a> {
        File(File<'a>),
        Loc(Loc<'a>),
        Generic(GenericDirective<'a>),
        Set(&'a str),
        SubsectionsViaSym,
    }

    #[derive(Clone, Debug)]
    pub struct File<'a> {
        pub index: u64,
        pub name: &'a str,
    }

    #[derive(Clone, Debug)]
    pub struct GenericDirective<'a>(pub &'a str);

    pub fn parse_statement(input: &str) -> IResult<&str, Statement> {
        let label = map(Label::parse, Statement::Label);

        let filename = delimited(tag("\""), take_while1(|c| c != '"'), tag("\""));

        let file = map(
            tuple((tag("\t.file\t"), complete::u64, space1, filename)),
            |(_, fileno, _, filename)| {
                Directive::File(File {
                    index: fileno,
                    name: filename,
                })
            },
        );

        let loc = map(Loc::parse, Directive::Loc);

        let generic = map(preceded(tag("\t."), take_while1(|c| c != '\n')), |s| {
            Directive::Generic(GenericDirective(s))
        });
        let set = map(
            preceded(tag(".set"), take_while1(|c| c != '\n')),
            Directive::Set,
        );
        let ssvs = map(tag(".subsections_via_symbols"), |_| {
            Directive::SubsectionsViaSym
        });

        let dunno = |input: &str| todo!("{:?}", &input[..100]);

        let instr = map(Instruction::parse, Statement::Instruction);
        let nothing = map(
            verify(take_while(|c| c != '\n'), |s: &str| s.is_empty()),
            |_| Statement::Nothing,
        );

        let dir = map(alt((file, loc, set, ssvs, generic)), Statement::Directive);

        terminated(alt((label, dir, instr, nothing, dunno)), newline)(input)
    }

    fn good_for_label(c: char) -> bool {
        c == '.'
            || c == '$'
            || c == '_'
            || ('a'..='z').contains(&c)
            || ('A'..='Z').contains(&c)
            || ('0'..='9').contains(&c)
    }
}
// }}}

use owo_colors::OwoColorize;
use statements::*;

use std::collections::BTreeMap;
use std::ops::{Index, Range};
use std::path::Path;

use line_span::LineSpans;

use nom::multi::many0;
use nom::IResult;

pub fn parse_file(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(parse_statement)(input)
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

pub fn dump_function(
    goal: (&str, usize),
    path: &Path,
    sysroot: &Path,
    fmt: &Format,
    items: &mut Vec<Item>,
) -> anyhow::Result<bool> {
    let contents = std::fs::read_to_string(path)?;
    let mut show = false;
    let mut seen = false;
    let mut prev_loc = Loc::default();

    let mut files = BTreeMap::new();
    let mut names = BTreeMap::new();

    let mut current_item = None;
    for (ix, line) in parse_file(&contents)
        .expect("Should be able to parse file")
        .1
        .iter()
        .enumerate()
    {
        if let Statement::Label(label) = line {
            if let Some(dem) = demangle::demangled(label.id) {
                let hashed = format!("{dem:?}");
                let name = format!("{dem:#?}");
                let name_entry = names.entry(name.clone()).or_insert(0);

                show = (name.as_ref(), *name_entry) == goal;
                current_item = Some(Item {
                    name,
                    hashed,
                    index: *name_entry,
                    len: ix,
                });
                *name_entry += 1;
                seen |= show;
            }
        }

        if let Statement::Directive(Directive::File(f)) = line {
            if !fmt.rust {
                continue;
            }
            files.entry(f.index).or_insert_with(|| {
                if let Ok(payload) = std::fs::read_to_string(f.name) {
                    return (f.name, CachedLines::without_ending(payload));
                } else if f.name.starts_with("/rustc/") {
                    if let Some(x) = f.name.splitn(4, '/').last() {
                        let src = sysroot.join("lib/rustlib/src/rust").join(x);
                        if let Ok(payload) = std::fs::read_to_string(src) {
                            return (f.name, CachedLines::without_ending(payload));
                        }
                    }
                }
                // if file is not found - ust create a dummy
                (f.name, CachedLines::without_ending(String::new()))
            });
        }
        if show {
            if let Statement::Directive(Directive::Loc(loc)) = &line {
                if !fmt.rust {
                    continue;
                }
                if loc.line == 0 {
                    continue;
                }
                if loc == &prev_loc {
                    continue;
                }
                prev_loc = *loc;
                if let Some((fname, file)) = files.get(&loc.file) {
                    let rust_line = &file[loc.line as usize - 1];
                    let pos = format!("\t\t// {} : {}", fname, loc.line);
                    println!("{}", color!(pos, OwoColorize::cyan));
                    println!(
                        "\t\t{}",
                        color!(rust_line.trim_start(), OwoColorize::bright_red)
                    );
                }
            } else {
                println!("{line}");
            }
        }

        if let Statement::Directive(Directive::Generic(GenericDirective("cfi_endproc"))) = line {
            if let Some(mut cur) = current_item.take() {
                cur.len = ix - cur.len;
                items.push(cur);
            }
            if seen {
                return Ok(true);
            }
        }
    }
    Ok(seen)
}
