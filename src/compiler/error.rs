use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt::Display,
    ops::{Index, Range},
    path::{Path, PathBuf},
};
use unicode_width::UnicodeWidthStr;

pub type SyntaxErrorWithCtx = ErrorWithCtx<SyntaxError>;
pub type SemanticErrorWithCtx = ErrorWithCtx<SemanticError>;

type ContextInner = u32;
#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub struct Context(Range<ContextInner>);

impl Context {
    pub fn dummy() -> Context {
        Self::from(0..0)
    }

    pub fn from_sub(left: &Context, right: &Context) -> Context {
        Self(left.0.start..right.0.end)
    }
}

impl From<Range<usize>> for Context {
    fn from(value: Range<usize>) -> Self {
        Self(value.start as ContextInner..value.end as ContextInner)
    }
}

impl Index<Context> for str {
    type Output = str;
    fn index(&self, index: Context) -> &Self::Output {
        let start = index.0.start as usize;
        let end = index.0.end as usize;
        &self[start..end]
    }
}

impl Index<Context> for String {
    type Output = str;
    fn index(&self, index: Context) -> &Self::Output {
        &self.as_str()[index]
    }
}

pub enum BuildError {
    AssemblerError(String),
    CompileErrors(Vec<CompileError>),
}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssemblerError(err) => write_err_header(f, err),
            Self::CompileErrors(errors) => {
                for err in errors {
                    write!(f, "{err}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub enum CompileError {
    IoError {
        src_path: PathBuf,
        err: std::io::Error,
        msg: &'static str,
    },
    PreprocessorError {
        src_path: PathBuf,
        err: String,
    },
    SyntaxErrors {
        src_path: PathBuf,
        err_cache: ErrorCache<SyntaxError>,
    },
    SemanticErrors {
        src_path: PathBuf,
        err_cache: ErrorCache<SemanticError>,
    },
    // SemanticErrors {
    //     src_path: PathBuf,
    //     lines_buf: String,
    //     errors: Vec<()>,
    // },
}

impl Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError { src_path, err, msg } => {
                write!(f, " --> {}", src_path.display())?;
                write!(f, "io error [{err}]: {msg}")
            }
            Self::PreprocessorError { src_path, err } => {
                write!(f, " --> {}", src_path.display())?;
                write!(f, "error: {err}")
            }
            Self::SyntaxErrors {
                src_path,
                err_cache,
            } => err_cache.display(f, src_path),
            Self::SemanticErrors {
                src_path,
                err_cache,
            } => err_cache.display(f, src_path),
        }
    }
}

impl CompileError {
    pub fn from_syntax_errors(
        src: &str,
        src_path: PathBuf,
        errors: Vec<ErrorWithCtx<SyntaxError>>,
    ) -> Self {
        let err_cache = ErrorCache::from_errors_with_ctx(src, errors);

        Self::SyntaxErrors {
            src_path,
            err_cache,
        }
    }

    pub fn from_semantic_errors(
        src: &str,
        src_path: PathBuf,
        errors: Vec<ErrorWithCtx<SemanticError>>,
    ) -> Self {
        let err_cache = ErrorCache::from_errors_with_ctx(src, errors);

        Self::SemanticErrors {
            src_path,
            err_cache,
        }
    }
}

#[derive(Debug)]
pub struct CachedError<E: Display> {
    line_ids: Range<usize>,
    col: usize,
    start_width: usize,
    end_width: usize,
    err: E,
}

#[derive(Debug)]
pub struct ErrorWithCtx<E: Display> {
    pub(crate) ctx: Context,
    pub(crate) err: E,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyntaxError {
    ExpectedColon,
    ExpectedFunctionArgs,
    ExpectedIdentifier,
    ExpectedOpenParen,
    ExpectedSemicolon,
    ExpectedWhile,
    IntegerLiteralTooLarge,
    InvalidLabel,
    InvalidExpr,
    InvalidIntegerSuffix,
    UnclosedDelimiter,
    UnknownSymbol,
    UnterminatedBlockComment,
}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::ExpectedColon => "expected a ':' after conditional then branch",
            Self::ExpectedFunctionArgs => "expected function args after function identifier",
            Self::ExpectedIdentifier => "expected an identifier",
            Self::ExpectedOpenParen => "expected a '('",
            Self::ExpectedSemicolon => "expected a ';'",
            Self::ExpectedWhile => "expected 'while' after do loop body",
            Self::IntegerLiteralTooLarge => "integer literal too large",
            Self::InvalidLabel => "invalid label",
            Self::InvalidExpr => "invalid expression",
            Self::InvalidIntegerSuffix => "invalid integer suffix",
            Self::UnclosedDelimiter => "unclosed delimiter",
            Self::UnknownSymbol => "unknown symbol",
            Self::UnterminatedBlockComment => "unterminated block comment",
        };
        let error_code = *self as usize;
        write!(f, "[STXE{error_code}] {msg}")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticError {
    DuplicateDecl,
    InvalidBreak,
    InvalidContinue,
    InvalidLValue,
    UndeclaredVar,
}

impl Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::DuplicateDecl => "duplicate declaration",
            Self::InvalidBreak => "break must be in loop or switch",
            Self::InvalidContinue => "continue must be in loop",
            Self::InvalidLValue => "invalid lvalue",
            Self::UndeclaredVar => "use of undeclared variable",
        };
        let error_code = *self as usize;
        write!(f, "[SEME{error_code}] {msg}")
    }
}

#[derive(Clone, Debug)]
pub struct LineInfo {
    num: usize,
    whitespace_end: usize,
    width: usize,
    span: Context,
}

impl LineInfo {
    fn new(src: &str, cache: &mut String, line_ranges: &[Range<usize>], line_num: usize) -> Self {
        let line = &src[line_ranges[line_num].clone()];
        let buf_start = cache.len();
        cache.push_str(line);
        let width = line.width();
        let whitespace_end = line
            .char_indices()
            .find(|(_, c)| !c.is_ascii_whitespace())
            .map_or(line.len(), |(i, _)| i);
        let span = Context::from(buf_start..cache.len());
        Self {
            num: line_num + 1,
            whitespace_end,
            width,
            span,
        }
    }
}

#[derive(Debug)]
pub struct ErrorCache<E: Display> {
    lines: Vec<LineInfo>,
    cache: String,
    errors: Vec<CachedError<E>>,
}

impl<E: Display> ErrorCache<E> {
    fn from_errors_with_ctx(src: &str, mut errors: Vec<ErrorWithCtx<E>>) -> Self {
        let line_ranges = src
            .lines()
            .map(|line| {
                let start = line.as_ptr().addr() - src.as_ptr().addr();
                start..start + line.len()
            })
            .collect::<Vec<_>>();
        let mut unique_lines = BTreeMap::new();
        let mut cache = String::new();

        errors.sort_by(|a, b| match a.ctx.0.start.cmp(&b.ctx.0.start) {
            Ordering::Equal => a.ctx.0.end.cmp(&b.ctx.0.end),
            ord => ord,
        });
        let errors = errors
            .into_iter()
            .map(|err| Self::cache_err(src, &line_ranges, &mut unique_lines, &mut cache, err))
            .collect();
        let lines = unique_lines.into_values().map(|(_, info)| info).collect();
        Self {
            lines,
            cache,
            errors,
        }
    }

    fn cache_err(
        src: &str,
        line_ranges: &[Range<usize>],
        unique_lines: &mut BTreeMap<usize, (usize, LineInfo)>,
        cache: &mut String,
        err: ErrorWithCtx<E>,
    ) -> CachedError<E> {
        let start_line = line_ranges.partition_point(|range| range.end < err.ctx.0.start as usize);
        let end_line = line_ranges.partition_point(|range| range.start < err.ctx.0.end as usize);

        for line_num in start_line..end_line {
            let line_id = unique_lines.len();
            unique_lines.entry(line_num).or_insert_with(|| {
                let info = LineInfo::new(src, cache, line_ranges, line_num);
                (line_id, info)
            });
        }
        let first_line_id = unique_lines.get(&start_line).map_or(0, |&(id, _)| id);
        let line_count = end_line - start_line;
        let line_ids = first_line_id..first_line_id + line_count;

        // Ensures no panic if an empty src is inputted
        let sub_line_ranges = &line_ranges[start_line..end_line];
        let start_line =
            &src[sub_line_ranges.first().map_or(0, |range| range.start)..err.ctx.0.start as usize];
        let col = start_line.chars().count() + 1;
        let start_width = start_line.width();
        let end_width = src
            [sub_line_ranges.last().map_or(0, |range| range.start)..err.ctx.0.end as usize]
            .width();

        CachedError::<E> {
            line_ids,
            col,
            start_width,
            end_width,
            err: err.err,
        }
    }

    /// Returns the `line_num`, `whitespace_end`, its width, and the line substr
    fn lookup(&self, line_id: usize) -> (usize, usize, usize, &str) {
        let LineInfo {
            num,
            whitespace_end,
            width,
            span,
        } = self.lines[line_id].clone();
        let line = &self.cache[span];
        (num, whitespace_end, width, line)
    }

    /// Returns the first line num and the max width in chars of the `line_nums` from `line_id` range
    fn get_start_info(&self, line_ids: &Range<usize>) -> (usize, usize) {
        let (line_num, _, _, _) = self.lookup(line_ids.start);
        let line_num_width = (line_num + line_ids.len() - 1)
            .checked_ilog10()
            .unwrap_or(0) as usize
            + 1;
        (line_num_width, line_num)
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>, src_path: &Path) -> std::fmt::Result {
        for CachedError {
            line_ids,
            col,
            start_width,
            end_width,
            err,
        } in &self.errors
        {
            write_err_header(f, err)?;
            let (line_num_width, start_line) = self.get_start_info(line_ids);
            writeln!(
                f,
                "  \x1b[1m\x1b[36m-->\x1b[0m {}:{start_line}:{col}",
                src_path.display(),
            )?;
            // For errors from empty src files
            if line_ids.is_empty() {
                continue;
            }

            // Write any remaining lines without ^ at the start
            let mut start_width = *start_width;
            let mut skipped = false;
            for line_id in line_ids.clone() {
                let (line_num, whitespace_end, line_width, line) = self.lookup(line_id);
                start_width = start_width.max(whitespace_end);
                if line.is_empty() || start_width == line_width {
                    if !skipped {
                        writeln!(f, "\x1b[1m\x1b[36m...\x1b[0m")?;
                        skipped = true;
                    }
                    continue;
                }
                skipped = false;

                let line_char_count = match line_id + 1 == line_ids.end {
                    true => end_width.saturating_sub(start_width),
                    false => line_width.saturating_sub(start_width),
                };
                writeln!(
                    f,
                    "\x1b[1m\x1b[36m{line_num:>line_num_width$} |\x1b[0m {line}",
                )?;
                // Do with stuff to draw squiggles under
                writeln!(
                    f,
                    "\x1b[1m\x1b[36m{:>line_num_width$} |\x1b[0m {:>start_width$}\x1b[1m\x1b[31m{:~>line_char_count$}\x1b[0m",
                    "", "", ""
                )?;
                start_width = 0;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Prints the header for the error output
///
/// error(bold and red): [err]
fn write_err_header<E: Display>(f: &mut std::fmt::Formatter<'_>, err: E) -> std::fmt::Result {
    writeln!(f, "\x1b[1m\x1b[31merror\x1b[39m: {err}\x1b[0m")
}

#[test]
fn multiline_err_cache_test() {
    let src = "int main()\n\
        printf(\n\
        \"Hello, %s!\",\n\
        name\n\
        );\n\
        wowow\n\
        \n\
        \n\
        \n\
        cool;";
    let errors = vec![
        ErrorWithCtx::<SyntaxError> {
            ctx: Context::from(17..src.len()),
            err: SyntaxError::InvalidExpr,
        },
        ErrorWithCtx::<SyntaxError> {
            ctx: Context::from(11..17),
            err: SyntaxError::InvalidExpr,
        },
    ];
    let error_msg = CompileError::from_syntax_errors(src, "test.c".into(), errors).to_string();

    let mut src_lines = error_msg.lines();
    _ = src_lines.next();
    assert_eq!(
        Some("  \x1b[1m\x1b[36m-->\x1b[0m test.c:2:1"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m2 |\x1b[0m printf("), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m  |\x1b[0m \x1b[1m\x1b[31m~~~~~~\x1b[0m"),
        src_lines.next()
    );
    _ = src_lines.next();
    _ = src_lines.next();

    assert_eq!(
        Some("  \x1b[1m\x1b[36m-->\x1b[0m test.c:2:7"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m 2 |\x1b[0m printf("), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m       \x1b[1m\x1b[31m~\x1b[0m"),
        src_lines.next()
    );
    assert_eq!(
        Some("\x1b[1m\x1b[36m 3 |\x1b[0m \"Hello, %s!\","),
        src_lines.next()
    );
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m \x1b[1m\x1b[31m~~~~~~~~~~~~~\x1b[0m"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m 4 |\x1b[0m name"), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m \x1b[1m\x1b[31m~~~~\x1b[0m"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m 5 |\x1b[0m );"), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m \x1b[1m\x1b[31m~~\x1b[0m"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m 6 |\x1b[0m wowow"), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m \x1b[1m\x1b[31m~~~~~\x1b[0m"),
        src_lines.next()
    );
    assert_eq!(Some("\x1b[1m\x1b[36m...\x1b[0m"), src_lines.next());
    assert_eq!(Some("\x1b[1m\x1b[36m10 |\x1b[0m cool;"), src_lines.next());
    assert_eq!(
        Some("\x1b[1m\x1b[36m   |\x1b[0m \x1b[1m\x1b[31m~~~~~\x1b[0m"),
        src_lines.next()
    );
}
