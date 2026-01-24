#![feature(allocator_api)]
#![feature(dropck_eyepatch)]
#![feature(hash_set_entry)]

use clap::Parser;
use std::process::ExitCode;

use crate::compiler::compile;

mod arena;
mod compiler;
pub mod intern;

/// A simple C lang implementation
#[derive(Debug, Parser)]
struct CompileArgs {
    /// Source files
    #[arg(required = true, num_args = 1..)]
    source: Vec<std::path::PathBuf>,
    /// Output destination
    #[arg(short = 'o', long = "output")]
    output: Option<std::path::PathBuf>,
    /// Stop after lexing stage
    #[arg(long = "lex")]
    stop_after_lex: bool,
    /// Stop after parsing stage
    #[arg(long = "parse")]
    stop_after_parse: bool,
    /// Stop after code generation stage
    #[arg(long = "codegen")]
    stop_after_codegen: bool,
    /// Emit assembly files
    #[arg(short = 'S')]
    emit_asm: bool,
}

struct CompileFlags {
    stop_after_lex: bool,
    stop_after_parse: bool,
    stop_after_codegen: bool,
    emit_asm: bool,
}

impl CompileArgs {
    fn flags(&self) -> CompileFlags {
        CompileFlags {
            stop_after_codegen: self.stop_after_codegen,
            stop_after_lex: self.stop_after_lex,
            stop_after_parse: self.stop_after_parse,
            emit_asm: self.emit_asm,
        }
    }
}

fn main() -> ExitCode {
    let start = std::time::Instant::now();
    let args = CompileArgs::parse();
    let exit_code = match compile(args) {
        Err(build_err) => {
            eprint!("{build_err}");
            ExitCode::FAILURE
        }
        Ok(_) => ExitCode::SUCCESS,
    };
    println!("Time to run: {:?}", start.elapsed());
    exit_code
}
