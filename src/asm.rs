use crate::demangle::{demangle_contents, demangle_name};
// TODO, use https://sourceware.org/binutils/docs/as/index.html
use crate::opts::Format;

#[derive(Clone, Debug)]
pub enum Statement<'a> {
    Label(Label<'a>),
    Directive(Directive<'a>),
    Instruction(Instruction<'a>),
    Nothing,
}

#[derive(Clone, Debug)]
pub struct Instruction<'a> {
    op: &'a str,
    args: Option<&'a str>,
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
        let (input, op) = take_while1(|c: char| c.is_alphanum())(input)?;
        let (input, args) = opt(preceded(space1, take_while1(|c| c != '\n')))(input)?;
        Ok((input, Instruction { op, args }))
    }
}

impl std::fmt::Display for Instruction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.args {
            Some(args) => write!(f, "{} {}", self.op, args),
            None => write!(f, "{}", self.op),
        }
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
            Directive::Membarrier => f.write_str("\t#MEMBARRIER"),
            Directive::Loc(l) => l.fmt(f),
            Directive::Generic(g) => g.fmt(f),
            Directive::Set(g) => f.write_str(g),
            Directive::SubsectionsViaSym => f.write_str(".subsections_via_symbols"),
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
        write!(f, "\t.{}", self.0)
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
        write!(f, "{}:", self.id)
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct Loc<'a> {
    pub file: u64,
    pub line: u64,
    pub column: u64,
    pub extra: Option<&'a str>,
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
    Membarrier,
    Generic(GenericDirective<'a>),
    Set(&'a str),
    SubsectionsViaSym,
}

#[derive(Clone, Debug)]
pub struct File<'a> {
    index: u64,
    name: &'a str,
}

#[derive(Clone, Debug)]
pub struct GenericDirective<'a>(&'a str);

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while1};
use nom::character::complete::{newline, space1};
use nom::combinator::{consumed, map, opt, verify};
use nom::multi::many0;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::*;

pub fn parse_file(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(parse_statement)(input)
}

fn good_for_label(c: char) -> bool {
    c == '.'
        || c == '$'
        || c == '_'
        || ('a'..='z').contains(&c)
        || ('A'..='Z').contains(&c)
        || ('0'..='9').contains(&c)
}

use nom::character::complete;
use owo_colors::OwoColorize;

fn parse_statement(input: &str) -> IResult<&str, Statement> {
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

pub fn dump_function(
    goal: &str,
    path: &Path,
    sysroot: &Path,
    fmt: &Format,
    items: &mut BTreeSet<String>,
) -> anyhow::Result<bool> {
    let contents = std::fs::read_to_string(path)?;
    let mut show = false;
    let mut seen = false;
    let mut prev_loc = Loc::default();

    let mut files = BTreeMap::new();

    for line in parse_file(&contents).unwrap().1.iter() {
        if let Statement::Label(label) = line {
            if let Some(dem) = demangle_name(label.id) {
                show = dem == goal;
                items.insert(dem);
                seen |= show;
            }
        }

        if fmt.rust {
            if let Statement::Directive(Directive::File(f)) = line {
                let entry = files.entry(f.index);
                if let Entry::Vacant(_) = &entry {
                    if let Ok(payload) = std::fs::read_to_string(f.name) {
                        let cache = line_span::CachedLines::without_ending(payload);
                        entry.or_insert((f.name, cache));
                    } else if f.name.starts_with("/rustc/") {
                        if let Some(x) = f.name.splitn(4, '/').last() {
                            let src = sysroot.join("lib/rustlib/src/rust").join(x);
                            if let Ok(payload) = std::fs::read_to_string(src) {
                                let cache = line_span::CachedLines::without_ending(payload);
                                entry.or_insert((f.name, cache));
                            } else {
                                println!("file not found {:?}", f.name);
                            }
                        } else {
                            println!("file not found {:?}", f.name);
                        }
                    } else {
                        println!("file not found {:?}", f.name);
                    }
                }
            }
        }
        if show {
            match line {
                Statement::Label(l) => {
                    if fmt.color {
                        println!("{}:", demangle_contents(l.id, false).bright_black());
                    } else {
                        println!("{}:", demangle_contents(l.id, false));
                    }
                }
                Statement::Directive(dir) => match dir {
                    Directive::File(_) => {}
                    Directive::Loc(loc) => {
                        if loc.line == 0 || (loc.file == prev_loc.file && loc.line == prev_loc.line)
                        {
                            continue;
                        }
                        prev_loc = *loc;
                        // use owo_colors::OwoColorize;
                        if let Some((_fname, file)) = files.get(&loc.file) {
                            if loc.line != 0 {
                                let line = &file[loc.line as usize - 1];
                                if fmt.color {
                                    println!("\t\t\t{}", line.bright_red());
                                } else {
                                    println!("\t\t\t{}", line);
                                }
                            }
                        }
                    }
                    Directive::Membarrier => todo!(),
                    Directive::Generic(g) => {
                        if fmt.color {
                            println!("\t{}", g.bright_black());
                        } else {
                            println!("\t{}", g);
                        }
                    }
                    Directive::Set(g) => {
                        if fmt.color {
                            println!(".set {}", g.bright_black());
                        } else {
                            println!(".set {}", g);
                        }
                    }
                    Directive::SubsectionsViaSym => {
                        if fmt.color {
                            println!(".{}", "subsections_via_symbols".bright_black());
                        } else {
                            println!(".subsections_via_symbols");
                        }
                    }
                },
                Statement::Instruction(i) => {
                    if fmt.color {
                        match i.args {
                            Some(args) => {
                                println!(
                                    "\t{} {}",
                                    i.op.bright_blue(),
                                    demangle_contents(args, fmt.color)
                                )
                            }
                            None => println!("\t{}", i.op.bright_blue()),
                        }
                    } else {
                        match i.args {
                            Some(args) => {
                                println!("\t{} {}", i.op, demangle_contents(args, fmt.color))
                            }
                            None => println!("\t{}", i.op),
                        }
                    }
                }
                Statement::Nothing => {}
            }
        }

        if let Statement::Directive(Directive::Generic(GenericDirective("cfi_endproc"))) = line {
            if seen {
                return Ok(true);
            }
        }
    }
    Ok(seen)
}
