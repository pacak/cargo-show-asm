use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use nom::branch::alt;
use nom::bytes::complete::{escaped_transform, tag, take_while1, take_while_m_n};
use nom::character::complete::{self, newline, none_of, not_line_ending, one_of, space0, space1};
use nom::combinator::{map, opt, recognize, value, verify};
use nom::multi::count;
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::{AsChar, IResult};
use owo_colors::OwoColorize;
use regex::Regex;

use crate::demangle::LabelKind;
use crate::opts::NameDisplay;
use crate::{color, demangle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement<'a> {
    Label(Label<'a>),
    Directive(Directive<'a>),
    Instruction(Instruction<'a>),
    Nothing,
    Dunno(&'a str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

fn parse_data_dec(input: &str) -> IResult<&str, Directive> {
    static DATA_DEC: OnceLock<Regex> = OnceLock::new();
    // all of those can insert something as well... Not sure if it's a full list or not
    // .long, .short .octa, .quad, .word,
    // .single .double .float
    // .ascii, .asciz, .string, .string8 .string16 .string32 .string64
    // .2byte .4byte .8byte
    // .dc
    // .inst .insn
    let reg = DATA_DEC.get_or_init(|| {
        // regexp is inspired by the compiler explorer
        Regex::new(
            "^\\s*\\.(ascii|asciz|[1248]?byte|dc(?:\\.[abdlswx])?|dcb(?:\\.[bdlswx])?\
            |ds(?:\\.[bdlpswx])?|double|dword|fill|float|half|hword|int|long|octa|quad|\
            short|single|skip|space|string(?:8|16|32|64)?|value|word|xword|zero)\\s+([^\\n]+)",
        )
        .expect("regexp should be valid")
    });

    let Some(cap) = reg.captures(input) else {
        use nom::error::*;
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Eof)));
    };
    let (Some(instr), Some(data)) = (cap.get(1), cap.get(2)) else {
        panic!("regexp should be valid and capture found something");
    };
    Ok((
        &input[data.range().end..],
        Directive::Data(instr.as_str(), data.as_str()),
    ))
}

impl<'a> Statement<'a> {
    /// Should we skip it for --simplify output?
    pub fn boring(&self) -> bool {
        if let Statement::Directive(Directive::SetValue(_, _)) = self {
            return false;
        }
        if let Statement::Directive(Directive::SectionStart(name)) = self {
            if name.starts_with(".data") || name.starts_with(".rodata") {
                return false;
            }
        }
        matches!(self, Statement::Directive(_) | Statement::Dunno(_))
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
            write!(f, " {w_label}")?;
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
            Statement::Dunno(l) => write!(f, "{l}"),
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
            Directive::SetValue(key, val) => {
                let key = demangle::contents(key, display);
                let val = demangle::contents(val, display);
                write!(
                    f,
                    ".{} {}, {}",
                    color!("set", OwoColorize::bright_magenta),
                    color!(key, OwoColorize::bright_cyan),
                    color!(val, OwoColorize::bright_cyan)
                )
            }
            Directive::SectionStart(s) => {
                let dem = demangle::contents(s, display);
                write!(f, "{} {dem}", color!(".section", OwoColorize::bright_red))
            }
            Directive::SubsectionsViaSym => write!(
                f,
                ".{}",
                color!("subsections_via_symbols", OwoColorize::bright_red)
            ),
            Directive::SymIsFun(s) => {
                let dem = demangle::contents(s, display);
                write!(
                    f,
                    ".{}\t{dem},@function",
                    color!("type", OwoColorize::bright_magenta)
                )
            }
            Directive::Data(ty, data) => {
                let data = demangle::contents(data, display);
                let w_label = demangle::color_local_labels(&data);
                write!(
                    f,
                    "\t.{}\t{}",
                    color!(ty, OwoColorize::bright_magenta),
                    color!(w_label, OwoColorize::bright_cyan)
                )
            }
            Directive::Global(data) => {
                let data = demangle::contents(data, display);
                let w_label = demangle::color_local_labels(&data);
                write!(
                    f,
                    "\t.{}\t{}",
                    color!("globl", OwoColorize::bright_magenta),
                    color!(w_label, OwoColorize::bright_cyan)
                )
            }
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

#[test]
fn parse_function_alias() {
    assert_eq!(
        parse_statement("\t.type\ttwo,@function\n").unwrap().1,
        Statement::Directive(Directive::SymIsFun("two"))
    );

    assert_eq!(
        parse_statement(".set\ttwo,\tone_plus_one\n").unwrap().1,
        Statement::Directive(Directive::SetValue("two", "one_plus_one"))
    )
}

#[test]
fn parse_data_decl() {
    assert_eq!(
        parse_statement("  .asciz  \"sample_merged\"\n").unwrap().1,
        Statement::Directive(Directive::Data("asciz", "\"sample_merged\""))
    );
    assert_eq!(
        parse_statement("          .byte   0\n").unwrap().1,
        Statement::Directive(Directive::Data("byte", "0"))
    );
    assert_eq!(
        parse_statement("\t.long   .Linfo_st\n").unwrap().1,
        Statement::Directive(Directive::Data("long", ".Linfo_st"))
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Directive<'a> {
    File(File<'a>),
    Loc(Loc<'a>),
    Global(&'a str),
    Generic(GenericDirective<'a>),
    SymIsFun(&'a str),
    SetValue(&'a str, &'a str),
    SubsectionsViaSym,
    SectionStart(&'a str),
    Data(&'a str, &'a str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        tuple((
            tag(".set"),
            space1,
            take_while1(good_for_label),
            tag(","),
            space0,
            take_while1(|c| c != '\n'),
        )),
        |(_, _, name, _, _, val)| Directive::SetValue(name, val),
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

    let typ = map(
        tuple((
            tag("\t.type"),
            space1,
            take_while1(good_for_label),
            tag(",@function"),
        )),
        |(_, _, id, _)| Directive::SymIsFun(id),
    );

    let global = map(
        tuple((
            space0,
            alt((tag(".globl"), tag(".global"))),
            space1,
            take_while1(|c| good_for_label(c) || c == '@'),
        )),
        |(_, _, _, name)| Directive::Global(name),
    );
    let dir = map(
        alt((
            file,
            global,
            loc,
            set,
            ssvs,
            section,
            typ,
            parse_data_dec,
            generic,
        )),
        Statement::Directive,
    );

    // use terminated on the subparsers so that if the subparser doesn't consume the whole line, it's discarded
    // we assume that each label/instruction/directive will only take one line
    terminated(alt((label, dir, instr, nothing, dunno)), newline)(input)
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
        matches!(self, Statement::Directive(Directive::Global(_)))
    }
}
