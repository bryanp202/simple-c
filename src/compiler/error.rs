use std::{
    collections::HashMap,
    fmt::Display,
    ops::Range,
    path::{Path, PathBuf},
};

pub type SyntaxErrorWithCtx = ErrorWithCtx<SyntaxError>;

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
    PreprocessorError {
        src_path: PathBuf,
        err: String,
    },
    SyntaxErrors {
        src_path: PathBuf,
        err_cache: ErrorCache<SyntaxError>,
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
}

#[derive(Debug)]
pub struct CachedError<E: Display> {
    line_ids: Range<usize>,
    start_col: usize,
    end_col: usize,
    err: E,
}

#[derive(Debug)]
pub struct ErrorWithCtx<E: Display> {
    pub(crate) ctx: Range<usize>,
    pub(crate) err: E,
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
pub struct ErrorCache<E: Display> {
    ranges: HashMap<usize, Range<usize>>,
    cache: String,
    errors: Vec<CachedError<E>>,
}

impl<E: Display> ErrorCache<E> {
    fn from_errors_with_ctx(src: &str, errors: Vec<ErrorWithCtx<E>>) -> Self {
        let line_ranges = src
            .lines()
            .map(|line| {
                let start = line.as_ptr() as usize - src.as_ptr() as usize;
                start..start + line.len()
            })
            .collect::<Vec<_>>();
        let mut ranges = HashMap::new();
        let mut cache = String::new();

        let errors = errors
            .into_iter()
            .map(|err| Self::cache_err(src, &line_ranges, &mut ranges, &mut cache, err))
            .collect();

        Self {
            ranges,
            cache,
            errors,
        }
    }

    fn cache_err(
        src: &str,
        line_ranges: &Vec<Range<usize>>,
        ranges: &mut HashMap<usize, Range<usize>>,
        cache: &mut String,
        err: ErrorWithCtx<E>,
    ) -> CachedError<E> {
        let start_line = line_ranges.partition_point(|range| range.end < err.ctx.start);
        let end_line = line_ranges.partition_point(|range| range.start < err.ctx.end);
        let line_ids = start_line..end_line;

        for line_num in line_ids.clone() {
            ranges.entry(line_num).or_insert_with(|| {
                let line = &src[line_ranges[line_num].clone()];
                let buf_start = cache.len();
                cache.push_str(line);
                buf_start..cache.len()
            });
        }

        // Ensures no panic if an empty src is inputted
        let (start_line_start, end_line_start) = if line_ranges.is_empty() {
            (0, 0)
        } else {
            (
                line_ranges[start_line].start,
                line_ranges[end_line.saturating_sub(1)].start,
            )
        };
        let start_col = src[start_line_start..err.ctx.start].chars().count();
        let end_col = src[end_line_start..err.ctx.end].chars().count();

        CachedError::<E> {
            line_ids,
            start_col,
            end_col,
            err: err.err,
        }
    }

    fn lookup(&self, line_num: usize) -> &str {
        let range = self
            .ranges
            .get(&line_num)
            .expect("Looked up out of bounds line");
        &self.cache[range.clone()]
    }

    fn display(&self, f: &mut std::fmt::Formatter<'_>, src_path: &Path) -> std::fmt::Result {
        for CachedError {
            line_ids,
            start_col,
            end_col,
            err,
        } in &self.errors
        {
            write_err_header(f, err)?;
            writeln!(
                f,
                " --> {}:{}:{}",
                src_path.display(),
                line_ids.start + 1,
                start_col + 1,
            )?;

            if line_ids.is_empty() {
                continue;
            }
            let line_num_width = (line_ids.end - 1).checked_ilog10().unwrap_or(0) as usize + 1;
            // Write the first line squiggles with ^ at start
            let line = self.lookup(line_ids.start);
            writeln!(f, " {:>line_num_width$} | {line}", line_ids.start + 1)?;
            // Do with stuff to draw squiqles under
            let len = if line_ids.start + 1 == line_ids.end {
                *end_col - start_col - 1
            } else {
                line.chars().count() - start_col - 1
            };
            writeln!(
                f,
                " {:>line_num_width$} | {:>start_col$}^{:~>len$}",
                "", "", ""
            )?;

            // Write any remaining lines without ^ at the start
            for line_id in line_ids.clone().skip(1) {
                let line = self.lookup(line_id);
                writeln!(f, " {:>line_num_width$} | {line}", line_id + 1)?;
                // Do with stuff to draw squiqles under
                let len = if line_id + 1 == line_ids.end {
                    *end_col
                } else {
                    line.chars().count()
                };
                writeln!(f, " {:>line_num_width$} | {:~>len$}", "", "")?;
            }
        }
        Ok(())
    }
}

/// Prints the header for the error output
///
/// error(bold and red): [err]
fn write_err_header<E: Display>(f: &mut std::fmt::Formatter<'_>, err: E) -> std::fmt::Result {
    writeln!(f, "\x1b[1m\x1b[31merror\x1b[0m: \x1b[1m{}\x1b[0m", err)
}

#[test]
fn multiline_err_cache_test() {
    let src = "int main()\n\
        printf(\n\
        \"Hello, %s!\",\n\
        name\n\
        );";
    let errors = vec![ErrorWithCtx::<SyntaxError> {
        ctx: 0..src.len(),
        err: SyntaxError::AdjacentDigitSeperators,
    }];
    let error_msg = CompileError::from_syntax_errors(src, "test.c".into(), errors).to_string();

    let mut src_lines = error_msg.lines();
    _ = src_lines.next();
    assert_eq!(Some(" --> test.c:1:1"), src_lines.next());
    assert_eq!(Some(" 1 | int main()"), src_lines.next());
    assert_eq!(Some("   | ^~~~~~~~~~"), src_lines.next());
    assert_eq!(Some(" 2 | printf("), src_lines.next());
    assert_eq!(Some("   | ~~~~~~~"), src_lines.next());
    assert_eq!(Some(" 3 | \"Hello, %s!\","), src_lines.next());
    assert_eq!(Some("   | ~~~~~~~~~~~~~"), src_lines.next());
    assert_eq!(Some(" 4 | name"), src_lines.next());
    assert_eq!(Some("   | ~~~~"), src_lines.next());
    assert_eq!(Some(" 5 | );"), src_lines.next());
    assert_eq!(Some("   | ~~"), src_lines.next());
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
