#![feature(allocator_api)]
#![feature(dropck_eyepatch)]
#![feature(hash_set_entry)]

use clap::Parser;
use std::{
    io::{BufWriter, Write},
    process::ExitCode,
};

use crate::compiler::compile;

mod arena;
mod compiler;
mod intern;

/// A simple C lang implementation
#[derive(Debug, Parser)]
struct CompileArgs {
    /// Source files
    #[arg(required = true, num_args = 1..)]
    source: Vec<std::path::PathBuf>,
    /// Output destination
    #[arg(short = 'o', long = "output")]
    output: Option<std::path::PathBuf>,
    /// Pretty print the ast
    #[arg(long)]
    pretty_ast: bool,
    /// Pretty print TACKY IR
    #[arg(long)]
    pretty_tacky: bool,
    /// Pretty print ASM IR
    #[arg(long)]
    pretty_asm: bool,
    /// Emit assembly files
    #[arg(short = 'S')]
    emit_asm: bool,
}

#[derive(Clone, Copy)]
struct CompileFlags {
    show_pretty_ast: bool,
    show_pretty_tacky: bool,
    show_pretty_asm: bool,
    emit_asm: bool,
}

impl CompileArgs {
    fn flags(&self) -> CompileFlags {
        CompileFlags {
            show_pretty_ast: self.pretty_ast,
            show_pretty_tacky: self.pretty_tacky,
            show_pretty_asm: self.pretty_asm,
            emit_asm: self.emit_asm,
        }
    }
}

fn main() -> ExitCode {
    let start = std::time::Instant::now();
    let args = CompileArgs::parse();
    let exit_code = match compile(args) {
        Err(build_err) => {
            let mut buf = BufWriter::new(std::io::stderr());
            match write!(buf, "{build_err}") {
                Err(err) => eprintln!("failed to print build err: {err}"),
                Ok(()) => {}
            }
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    };
    eprintln!("Time to run: {:?}", start.elapsed());
    exit_code
}
