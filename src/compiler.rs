use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use clap::builder::OsStr;

use crate::{
    CompileArgs, CompileFlags,
    arena::Arena,
    compiler::{
        ast::TackyConverter,
        error::{BuildError, CompileError},
        parser::Parser,
        tacky::AsmConverter,
    },
    intern::Interner,
};

mod asm;
mod ast;
mod error;
mod lexer;
mod parser;
mod pretty;
mod tacky;
mod token;

pub fn compile(args: CompileArgs) -> Result<(), BuildError> {
    let mut asm_files = Vec::new();
    let mut compile_errors = Vec::new();

    for (module_num, src_path) in args.source.iter().enumerate() {
        match build_unit(args.flags(), src_path.clone(), module_num) {
            Ok(unit_asm_file) => asm_files.push(unit_asm_file.into_os_string()),
            Err(err) => compile_errors.push(err),
        }
    }

    if compile_errors.is_empty() {
        assemble(args, &asm_files)
    } else {
        Err(BuildError::CompileErrors(compile_errors))
    }
}

fn assemble(args: CompileArgs, asm_files: &[OsString]) -> Result<(), BuildError> {
    let mut output_path = args.output.unwrap_or_else(|| PathBuf::from("out"));
    output_path.set_extension("exe");

    let output = Command::new("gcc")
        .args(
            asm_files
                .iter()
                .map(std::ffi::OsString::as_os_str)
                .chain([&OsStr::from("-o"), output_path.as_os_str()]),
        )
        .output()
        .map_err(|err| BuildError::AssemblerError(err.to_string()))?;

    if !output.status.success() {
        return Err(BuildError::AssemblerError(
            String::from_utf8(output.stderr).expect("gcc fail message invalid utf8"),
        ));
    }

    if !args.emit_asm {
        for asm_file in asm_files {
            std::fs::remove_file(asm_file).expect("Build unit returned bad path");
        }
    }

    Ok(())
}

fn build_unit(
    compile_flags: CompileFlags,
    src_path: PathBuf,
    module_num: usize,
) -> Result<PathBuf, CompileError> {
    let temp_name = PathBuf::new().with_file_name(format!("_{module_num}"));

    let (src_path, i_path) = preprocess_unit(src_path, temp_name)?;
    generate_unit(compile_flags, &src_path, &i_path)
}

fn preprocess_unit(
    src_path: PathBuf,
    mut temp_name: PathBuf,
) -> Result<(PathBuf, PathBuf), CompileError> {
    let i_path = {
        temp_name.set_extension("i");
        temp_name
    };

    let preprocess_result = Command::new("gcc")
        .args([
            &OsStr::from("-E"),
            &OsStr::from("-P"),
            src_path.as_os_str(),
            &OsStr::from("-o"),
            i_path.as_os_str(),
        ])
        .output();
    let output = match preprocess_result {
        Ok(output) => output,
        Err(err) => {
            return Err(CompileError::PreprocessorError {
                src_path,
                err: err.to_string(),
            });
        }
    };

    if output.status.success() {
        Ok((src_path, i_path))
    } else {
        Err(CompileError::PreprocessorError {
            src_path,
            err: String::from_utf8(output.stderr)
                .unwrap_or_else(|_| "Preprocessor failed".to_string()),
        })
    }
}

fn generate_unit(
    compile_flags: CompileFlags,
    src_path: &Path,
    i_path: &Path,
) -> Result<PathBuf, CompileError> {
    let src = std::fs::read_to_string(i_path).expect("Preprocessor failed to output to temp name");
    std::fs::remove_file(i_path).expect("Preprocessed source was removed early");

    let asm_path = if compile_flags.emit_asm {
        src_path.with_extension("s")
    } else {
        i_path.with_extension("s")
    };

    let mut id_interner = Interner::new();
    let ast_arena = Arena::new();

    // Parse into an ast
    let ast_tree = Parser::new(&src, &mut id_interner, &ast_arena).parse(src_path.to_path_buf())?;
    if compile_flags.show_pretty_ast {
        eprintln!("{}: {ast_tree}", src_path.display());
    }

    // Convert into a three address code IR
    let tacky_program = TackyConverter::new().convert(ast_tree);
    if compile_flags.show_pretty_tacky {
        eprintln!("{}: {tacky_program}", src_path.display());
    }

    // Convert into x86_64 asm IR
    let asm_program = AsmConverter::new().convert(tacky_program);
    if compile_flags.show_pretty_asm {
        eprintln!("{}:", src_path.display());
        eprintln!("{asm_program}");
    }

    asm_program
        .generate(&asm_path)
        .expect("Failed to write to file");

    Ok(asm_path)
}
