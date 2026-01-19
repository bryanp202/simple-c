use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::Display,
    ops::Range,
    path::{Path, PathBuf},
};

use crate::compiler::token::Token;

pub enum BuildError {
    AssemblerError(String),
    CompileErrors(Vec<CompileError>),
}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssemblerError(err) => write!(f, "{err}"),
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
    PreprocessorError {
        src_path: PathBuf,
        err: String,
    },
    SyntaxErrors {
        src_path: PathBuf,
        lines_buf: LineBuf,
        errors: Vec<CachedError<SyntaxError>>,
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
            Self::PreprocessorError { src_path, err } => {
                write!(f, " --> {}", src_path.display())?;
                write!(f, "error: {err}")
            }
            Self::SyntaxErrors {
                src_path,
                lines_buf,
                errors,
            } => display_error_messages(f, src_path, lines_buf, errors.iter()),
        }
    }
}

impl CompileError {
    pub fn from_syntax_errors(
        src: &str,
        src_path: PathBuf,
        errors: Vec<SyntaxErrorWithCtx>,
    ) -> Self {
        let mut line_buf_builder = LineBufBuilder::new(src);

        let errors = errors
            .into_iter()
            .map(|SyntaxErrorWithCtx { ctx, err }| {
                let (line_ids, col, len) = line_buf_builder.cache(src, ctx.range);
                CachedError::<SyntaxError> {
                    line_ids,
                    col,
                    len,
                    err,
                }
            })
            .collect();
        let lines_buf = line_buf_builder.build();

        Self::SyntaxErrors {
            src_path,
            lines_buf,
            errors,
        }
    }
}

#[derive(Debug)]
pub struct CachedError<T> {
    line_ids: Range<usize>,
    col: usize,
    len: usize,
    err: T,
}

#[derive(Debug)]
pub struct SyntaxErrorWithCtx {
    pub(crate) ctx: Token,
    pub(crate) err: SyntaxError,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyntaxError {
    AdjacentDigitSeperators,
    InvalidIntegerSuffix,
    UnknownSymbol,
    UnterminatedBlockComment,
}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Syntax error")
    }
}

#[derive(Debug)]
struct LineBufBuilder {
    line_starts: Vec<usize>,
    line_ends: Vec<usize>,
    ranges: HashMap<usize, Range<usize>>,
    cache: String,
}

impl LineBufBuilder {
    fn new(src: &str) -> Self {
        let mut line_starts = Vec::new();
        let mut line_ends = Vec::new();

        for line in src.lines() {
            let start = line.as_ptr() as usize - src.as_ptr() as usize;
            let end = start + line.len();
            line_starts.push(start);
            line_ends.push(end);
        }

        Self {
            line_starts,
            line_ends,
            ranges: HashMap::new(),
            cache: String::new(),
        }
    }

    fn build(self) -> LineBuf {
        LineBuf {
            ranges: self.ranges,
            cache: self.cache,
        }
    }

    /// Caches any new lines to the buf
    ///
    /// Returns: The range of lines that input `range` is in
    /// and the col of the start of the range
    /// and the len of the range in chars
    fn cache(&mut self, src: &str, range: Range<usize>) -> (Range<usize>, usize, usize) {
        let start_line = self
            .line_starts
            .binary_search_by(|&probe| {
                if range.start < probe {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .unwrap_or_else(|a| a) - 1;
        let end_line = self
            .line_ends
            .binary_search_by(|&probe| {
                if range.end < probe {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .unwrap_or_else(|a| a) + 1;
        let line_ids = start_line..end_line;

        for line_num in line_ids.clone() {
            self.ranges.entry(line_num).or_insert_with(|| {
                let line = &src[self.line_starts[line_num]..self.line_ends[line_num]];
                let buf_start = self.cache.len();
                self.cache.push_str(line);
                buf_start..self.cache.len()
            });
        }

        let from_line = &src[self.line_starts[line_ids.start]..];
        let (col, _) = from_line
            .char_indices()
            .enumerate()
            .find(|(i, _)| {
                range.start
                    == (i + (from_line.as_ptr() as usize - src.as_ptr() as usize))
            })
            .expect("Token out of bounds");

        let len = src[range].chars().count() - line_ids.len();

        (line_ids, col + 1, len)
    }
}

#[derive(Debug)]
pub struct LineBuf {
    ranges: HashMap<usize, Range<usize>>,
    cache: String,
}

impl LineBuf {
    fn lookup(&self, line_num: usize) -> &str {
        let range = self
            .ranges
            .get(&line_num)
            .expect("Looked up out of bounds line");
        &self.cache[range.clone()]
    }
}

fn display_error_messages<'err, E: 'err + Display>(
    f: &mut std::fmt::Formatter<'_>,
    src_path: &Path,
    lines_buf: &LineBuf,
    errors: impl Iterator<Item = &'err CachedError<E>>,
) -> std::fmt::Result {
    for CachedError { line_ids, col, len, err } in errors {
        writeln!(f, "error: {}", err)?;
        writeln!(f, " --> {}:{}:{col}", src_path.display(), line_ids.start + 1)?;

        let line_num_width = (((line_ids.end + 1) as f64).log10().ceil() as usize).max(1);

        for line_id in line_ids.clone() {
            let line = lines_buf.lookup(line_id);
            eprintln!(" {:^width$} | {line}", line_id + 1, width = line_num_width);
            // Do with stuff to draw squiqles under
        }
    }
    Ok(())
}
// eprint!("error: ");
// match err {
//     crate::error::LexerError::UnknownSymbol => eprintln!("unknown symbol"),
//     _ => {}
// }

// let line_start = src[..token.range.start]
//     .rfind('\n')
//     .map(|i| i + 1)
//     .unwrap_or(0);
// let line_end = src[token.range.start..]
//     .find('\n')
//     .map(|i| token.range.start + i)
//     .unwrap_or(src.len());

// let line = &src[line_start..line_end];

// let line_num = src[..line_start].chars().filter(|&c| c == '\n').count() + 1;
// let column = src[line_start..token.range.start].chars().count() + 1;
// let line_num_width = ((line_num as f64).log10().ceil() as usize).max(1);
// let len = src[token.range.clone()].chars().count() - 1;

// eprintln!(" --> {}:{line_num}:{column}", src_path.display());
// eprintln!(" {:^width$} | {line}", line_num, width = line_num_width);
// eprintln!(
//     " {:^width$} | {:>shift$}^{:~>len$}",
//     "",
//     "",
//     "",
//     width = line_num_width,
//     shift = column - 1,
//     len = len
// );
// eprintln!();
