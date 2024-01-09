use std::borrow::Cow;
use std::path::Path;

use nom::branch::alt;
use nom::bytes::complete::{escaped_transform, tag, take_while1, take_while_m_n};
use nom::character::complete;
use nom::character::complete::{newline, none_of, not_line_ending, one_of, space1};
use nom::combinator::{map, opt, recognize, value, verify};
use nom::multi::count;
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::{AsChar, IResult};
use owo_colors::OwoColorize;

use crate::demangle::LabelKind;
use crate::opts::NameDisplay;
use crate::{color, demangle};

#[derive(Clone, Debug)]
pub enum Statement<'a> {
    Label(Label<'a>),
    Directive(Directive<'a>),
    Instruction(Instruction<'a>),
    Nothing,
    Dunno(&'a str),
}

#[derive(Clone, Debug)]
pub struct Instruction<'a> {
    pub op: &'a str,
    pub args: Option<&'a str>,
}

impl<'a> Instruction<'a> {
    pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
        preceded(tag("\t"), alt((Self::parse_regular, Self::parse_sharp)))(input)
    }

    fn parse_sharp(input: &'a str) -> IResult<&'a str, Self> {
        let sharps = take_while_m_n(1, 2, |c| c == '#');
        let sharp_tag = pair(sharps, not_line_ending);
        map(recognize(sharp_tag), |op| Instruction { op, args: None })(input)
    }

    fn parse_regular(input: &'a str) -> IResult<&'a str, Self> {
        // NOTE: ARM allows `.` inside instruction names e.g. `b.ne` for branch not equal
        //       Wasm also uses `.` in instr names, and uses `_` for `end_function`
        let op = take_while1(|c| AsChar::is_alphanum(c) || matches!(c, '.' | '_'));
        let args = opt(preceded(space1, not_line_ending));
        map(pair(op, args), |(op, args)| Instruction { op, args })(input)
    }
}

impl std::fmt::Display for Instruction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display = NameDisplay::from(&*f);
        if self.op.starts_with("#DEBUG_VALUE:") {
            write!(f, "{}", color!(self.op, OwoColorize::blue))?;
        } else {
            write!(f, "{}", color!(self.op, OwoColorize::bright_blue))?;
        }
        if let Some(args) = self.args {
            let args = demangle::contents(args, display);
            let w_label = demangle::color_local_labels(&args);
            let w_comment = demangle::color_comment(&w_label);
            write!(f, " {w_comment}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Statement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Statement::Label(l) => l.fmt(f),
            Statement::Directive(d) => {
                if f.alternate() {
                    write!(f, "{d:#}")
                } else {
                    write!(f, "{d}")
                }
            }
            Statement::Instruction(i) => {
                if f.sign_minus() {
                    write!(f, "\t{i:-#}")
                } else if f.alternate() {
                    write!(f, "\t{i:#}")
                } else {
                    write!(f, "\t{i}")
                }
            }
            Statement::Nothing => Ok(()),
            Statement::Dunno(l) => write!(f, "{}", demangle::color_comment(l)),
        }
    }
}

impl std::fmt::Display for Directive<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display = NameDisplay::from(&*f);
        match self {
            Directive::File(ff) => ff.fmt(f),
            Directive::Loc(l) => l.fmt(f),
            Directive::Generic(g) => g.fmt(f),
            Directive::Set(g) => {
                f.write_str(&format!(".set {}", color!(g, OwoColorize::bright_cyan)))
            }
            Directive::SectionStart(s) => {
                let dem = demangle::contents(s, display);
                f.write_str(&format!(
                    "{} {}",
                    color!(".section", OwoColorize::bright_red),
                    dem
                ))
            }
            Directive::SubsectionsViaSym => f.write_str(&format!(
                ".{}",
                color!("subsections_via_symbols", OwoColorize::bright_red)
            )),
        }
    }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.as_full_path().display(), f)
    }
}

impl std::fmt::Display for File<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\t.file\t{} {}", self.index, self.path)?;
        if let Some(md5) = self.md5 {
            write!(f, " {md5}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for GenericDirective<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display = NameDisplay::from(&*f);
        write!(
            f,
            "\t.{}",
            color!(
                demangle::contents(self.0, display),
                OwoColorize::bright_magenta
            )
        )
    }
}

impl std::fmt::Display for Loc<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.extra {
            Some(x) => write!(
                f,
                "\t.loc\t{file} {line} {col} {x}",
                file = self.file,
                line = self.line,
                col = self.column,
            ),
            None => write!(
                f,
                "\t.loc\t{file} {line} {col}",
                file = self.file,
                line = self.line,
                col = self.column
            ),
        }
    }
}

impl From<&std::fmt::Formatter<'_>> for NameDisplay {
    fn from(f: &std::fmt::Formatter) -> Self {
        if f.sign_minus() {
            NameDisplay::Mangled
        } else if f.alternate() {
            NameDisplay::Full
        } else {
            NameDisplay::Short
        }
    }
}

impl std::fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display = NameDisplay::from(&*f);
        write!(
            f,
            "{}:",
            color!(
                demangle::contents(self.id, display),
                OwoColorize::bright_yellow
            )
        )
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Label<'a> {
    pub id: &'a str,
    pub kind: LabelKind,
}

impl<'a> Label<'a> {
    pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
        // TODO: label can't start with a digit
        let no_comment = tag(":");
        let comment = terminated(
            tag(":"),
            tuple((
                take_while1(|c| c == ' '),
                tag("# @"),
                take_while1(|c| c != '\n'),
            )),
        );
        map(
            terminated(take_while1(good_for_label), alt((comment, no_comment))),
            |id: &str| Label {
                id,
                kind: demangle::label_kind(id),
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
        // DWARF2 (Unix):      .loc               fileno lineno [column] [options]
        // CodeView (Windows): .cv_loc functionid fileno lineno [column] [prologue_end] [is_stmt value]
        map(
            tuple((
                alt((
                    tag("\t.loc\t"),
                    terminated(tag("\t.cv_loc\t"), tuple((complete::u64, space1))),
                )),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePath {
    FullPath(String),
    PathAndFileName { path: String, name: String },
}

impl FilePath {
    pub fn as_full_path(&self) -> Cow<'_, Path> {
        match self {
            FilePath::FullPath(path) => Cow::Borrowed(Path::new(path)),
            FilePath::PathAndFileName { path, name } => Cow::Owned(Path::new(path).join(name)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File<'a> {
    pub index: u64,
    pub path: FilePath,
    pub md5: Option<&'a str>,
}

fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    // Inverse of MCAsmStreamer::PrintQuotedString() in MCAsmStreamer.cpp in llvm.
    delimited(
        tag("\""),
        escaped_transform(
            none_of("\\\""),
            '\\',
            alt((
                value('\\', tag("\\")),
                value('\"', tag("\"")),
                value('\x08', tag("b")),
                value('\x0c', tag("f")),
                value('\n', tag("n")),
                value('\r', tag("r")),
                value('\t', tag("t")),
                // 3 digits in base 8
                map(count(one_of("01234567"), 3), |digits| {
                    let mut v = 0u8;
                    for c in digits {
                        v = (v << 3) | c.to_digit(8).unwrap() as u8;
                    }
                    char::from(v)
                }),
            )),
        ),
        tag("\""),
    )(input)
}

// Workaround for a problem in llvm code that produces debug symbols on Windows.
// As of the time of writing, CodeViewDebug::getFullFilepath() in CodeViewDebug.cpp
// replaces all occurrences of "\\" with "\".
// This breaks paths that start with "\\?\" (a prefix instructing Windows to skip
// filename parsing) - they turn into "\?\", which is invalid.
// Here we turn "\?\" back into "\\?\".
// Hopefully this will get fixed in llvm, and we'll remove this.
fn fixup_windows_file_path(mut p: String) -> String {
    if p.starts_with("\\?\\") {
        p.insert(0, '\\');
    }
    p
}

impl<'a> File<'a> {
    pub fn parse(input: &'a str) -> IResult<&'a str, Self> {
        // DWARF2/DWARF5 (Unix): .file    fileno [dirname] "filename" [md5]
        // CodeView (Windows):   .cv_file fileno           "filename" ["checksum"] [checksumkind]
        alt((
            map(
                tuple((
                    tag("\t.file\t"),
                    complete::u64,
                    space1,
                    parse_quoted_string,
                    opt(preceded(space1, parse_quoted_string)),
                    opt(preceded(space1, complete::hex_digit1)),
                )),
                |(_, fileno, _, filepath, filename, md5)| File {
                    index: fileno,
                    path: match filename {
                        Some(filename) => FilePath::PathAndFileName {
                            path: filepath,
                            name: filename,
                        },
                        None => FilePath::FullPath(filepath),
                    },
                    md5,
                },
            ),
            map(
                tuple((
                    tag("\t.cv_file\t"),
                    complete::u64,
                    space1,
                    parse_quoted_string,
                    opt(preceded(
                        space1,
                        delimited(tag("\""), complete::hex_digit1, tag("\"")),
                    )),
                    opt(preceded(space1, complete::u64)),
                )),
                |(_, fileno, _, filename, checksum, checksumkind)| File {
                    index: fileno,
                    path: FilePath::FullPath(fixup_windows_file_path(filename)),
                    // FileChecksumKind enum: { None, MD5, SHA1, SHA256 }
                    // (from llvm's CodeView.h)
                    md5: if checksumkind == Some(1) {
                        checksum
                    } else {
                        None
                    },
                },
            ),
        ))(input)
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
                kind: LabelKind::Unknown,
            }
        ))
    );
    assert_eq!(
        Label::parse("__ZN4core3ptr50drop_in_place$LT$rand..rngs..thread..ThreadRng$GT$17hba90ed09529257ccE:"),
        Ok((
            "",
            Label {
                id: "__ZN4core3ptr50drop_in_place$LT$rand..rngs..thread..ThreadRng$GT$17hba90ed09529257ccE",
                kind: LabelKind::Global,
            }
        ))
    );
    assert_eq!(
        Label::parse(".Lexception0:"),
        Ok((
            "",
            Label {
                id: ".Lexception0",
                kind: LabelKind::Local
            }
        ))
    );
    assert_eq!(
        Label::parse("LBB0_1:"),
        Ok((
            "",
            Label {
                id: "LBB0_1",
                kind: LabelKind::Local
            }
        ))
    );
    assert_eq!(
        Label::parse("Ltmp12:"),
        Ok((
            "",
            Label {
                id: "Ltmp12",
                kind: LabelKind::Temp
            }
        ))
    );
    assert_eq!(
        Label::parse("__ZN4core3ptr50drop_in_place$LT$rand..rngs..thread..ThreadRng$GT$17hba90ed09529257ccE: # @\"rand\""),
        Ok((
            "",
            Label {
                id: "__ZN4core3ptr50drop_in_place$LT$rand..rngs..thread..ThreadRng$GT$17hba90ed09529257ccE",
                kind: LabelKind::Global,
            }
        ))
    );
    assert_eq!(
        Label::parse("_ZN44_$LT$$RF$T$u20$as$u20$core..fmt..Display$GT$3fmt17h6557947cc19e5571E: # @\"_ZN44_$LT$$RF$T$u20$as$u20$core..fmt..Display$GT$3fmt17h6557947cc19e5571E\""),
        Ok((
            "",
            Label {
                id: "_ZN44_$LT$$RF$T$u20$as$u20$core..fmt..Display$GT$3fmt17h6557947cc19e5571E",
                kind: LabelKind::Global,
            }
        ))
    );
    assert_eq!(
        Label::parse(
            "_ZN6sample4main17hb59e25bba3071c26E:    # @_ZN6sample4main17hb59e25bba3071c26E"
        ),
        Ok((
            "",
            Label {
                id: "_ZN6sample4main17hb59e25bba3071c26E",
                kind: LabelKind::Global,
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
    assert_eq!(
        Loc::parse("\t.cv_loc\t9 6 1 0"),
        Ok((
            "",
            Loc {
                file: 6,
                line: 1,
                column: 0,
                extra: None,
            }
        ))
    );
    assert_eq!(
        Loc::parse("\t.cv_loc\t9 6 1 0 rest of the line is ignored"),
        Ok((
            "",
            Loc {
                file: 6,
                line: 1,
                column: 0,
                extra: Some("rest of the line is ignored"),
            }
        ))
    );
}

#[test]
fn test_parse_file() {
    let (rest, file) = File::parse("\t.file\t9 \"/home/ubuntu/buf-test/src/main.rs\"").unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 9,
            path: FilePath::FullPath("/home/ubuntu/buf-test/src/main.rs".to_owned()),
            md5: None
        }
    );
    assert_eq!(
        file.path.as_full_path(),
        Path::new("/home/ubuntu/buf-test/src/main.rs")
    );

    let (rest, file) = File::parse("\t.file\t9 \"/home/ubuntu/buf-test\" \"src/main.rs\"").unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 9,
            path: FilePath::PathAndFileName {
                path: "/home/ubuntu/buf-test".to_owned(),
                name: "src/main.rs".to_owned()
            },
            md5: None,
        }
    );
    assert_eq!(
        file.path.as_full_path(),
        Path::new("/home/ubuntu/buf-test/src/main.rs")
    );

    let (rest, file) = File::parse(
        "\t.file\t9 \"/home/ubuntu/buf-test\" \"src/main.rs\" 74ab618651b843a815bf806bd6c50c19",
    )
    .unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 9,
            path: FilePath::PathAndFileName {
                path: "/home/ubuntu/buf-test".to_owned(),
                name: "src/main.rs".to_owned()
            },
            md5: Some("74ab618651b843a815bf806bd6c50c19"),
        }
    );
    assert_eq!(
        file.path.as_full_path(),
        Path::new("/home/ubuntu/buf-test/src/main.rs")
    );

    let (rest, file) = File::parse(
        "\t.file\t9 \"/home/\\000path\\twith\\nlots\\\"of\\runprintable\\147characters\\blike\\\\this\\f\" \"src/main.rs\" 74ab618651b843a815bf806bd6c50c19",
    )
    .unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 9,
            path: FilePath::PathAndFileName {
                path: "/home/\x00path\twith\nlots\"of\runprintable\x67characters\x08like\\this\x0c"
                    .to_owned(),
                name: "src/main.rs".to_owned()
            },
            md5: Some("74ab618651b843a815bf806bd6c50c19"),
        }
    );
    assert_eq!(
        file.path.as_full_path(),
        Path::new("/home/\x00path\twith\nlots\"of\runprintable\x67characters\x08like\\this\x0c/src/main.rs")
    );

    let (rest, file) = File::parse(
        "\t.cv_file\t6 \"\\\\?\\\\C:\\\\Foo\\\\Bar\\\\src\\\\main.rs\" \"778FECDE2D48F9B948BA07E6E0B4AB983123B71B\" 2",
    )
    .unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 6,
            path: FilePath::FullPath("\\\\?\\C:\\Foo\\Bar\\src\\main.rs".to_owned()),
            md5: None,
        }
    );

    let (rest, file) = File::parse(
        "\t.cv_file\t6 \"C:\\\\Foo\\\\Bar\\\\src\\\\main.rs\" \"778FECDE2D48F9B948BA07E6E0B4AB98\" 1",
    )
    .unwrap();
    assert!(rest.is_empty());
    assert_eq!(
        file,
        File {
            index: 6,
            path: FilePath::FullPath("C:\\Foo\\Bar\\src\\main.rs".to_owned()),
            md5: Some("778FECDE2D48F9B948BA07E6E0B4AB98"),
        }
    );
}

#[derive(Clone, Debug)]
pub enum Directive<'a> {
    File(File<'a>),
    Loc(Loc<'a>),
    Generic(GenericDirective<'a>),
    Set(&'a str),
    SubsectionsViaSym,
    SectionStart(&'a str),
}

#[derive(Clone, Debug)]
pub struct GenericDirective<'a>(pub &'a str);

pub fn parse_statement(input: &str) -> IResult<&str, Statement> {
    let label = map(Label::parse, Statement::Label);

    let file = map(File::parse, Directive::File);

    let loc = map(Loc::parse, Directive::Loc);

    let section = map(
        preceded(tag("\t.section"), take_while1(|c| c != '\n')),
        |s: &str| Directive::SectionStart(s.trim()),
    );
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

    let dunno = map(take_while1(|c| c != '\n'), Statement::Dunno);
    // let dunno = |input: &str| todo!("{:?}", &input[..100]);

    let instr = map(Instruction::parse, Statement::Instruction);
    let nothing = map(verify(not_line_ending, str::is_empty), |_| {
        Statement::Nothing
    });

    let dir = map(
        alt((file, loc, set, ssvs, section, generic)),
        Statement::Directive,
    );

    // use terminated on the subparsers so that if the subparser doesn't consume the whole line, it's discarded
    // we assume that each label/instruction/directive will only take one line
    alt((
        terminated(label, newline),
        terminated(dir, newline),
        terminated(instr, newline),
        terminated(nothing, newline),
        terminated(dunno, newline),
    ))(input)
}

fn good_for_label(c: char) -> bool {
    c == '.' || c == '$' || c == '_' || c.is_ascii_alphanumeric()
}
impl Statement<'_> {
    pub(crate) fn is_end_of_fn(&self) -> bool {
        let check_id = |id: &str| id.strip_prefix('.').unwrap_or(id).starts_with("Lfunc_end");
        matches!(self, Statement::Label(Label { id, .. }) if check_id(id))
    }

    pub(crate) fn is_section_start(&self) -> bool {
        matches!(self, Statement::Directive(Directive::SectionStart(_)))
    }

    pub(crate) fn is_global(&self) -> bool {
        match self {
            Statement::Directive(Directive::Generic(GenericDirective(dir))) => {
                dir.starts_with("globl\t")
            }
            _ => false,
        }
    }
}
