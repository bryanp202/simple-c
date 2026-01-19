use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use clap::builder::OsStr;

use crate::{
    CompileArgs, CompileFlags,
    compiler::{
        error::{BuildError, CompileError},
        parser::Parser,
    },
};

mod ast;
mod error;
mod lexer;
mod parser;
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
        assemble(args, asm_files)
    } else {
        Err(BuildError::CompileErrors(compile_errors))
    }
}

fn assemble(args: CompileArgs, asm_files: Vec<OsString>) -> Result<(), BuildError> {
    let mut output_path = args.output.unwrap_or_else(|| PathBuf::from("out"));
    output_path.set_extension("exe");

    let output = Command::new("gcc")
        .args(
            asm_files
                .iter()
                .map(|file| file.as_os_str())
                .chain([&OsStr::from("-o"), output_path.as_os_str()]),
        )
        .output()
        .map_err(|err| BuildError::AssemblerError(err.to_string()))?;

    if !output.status.success() {
        return Err(BuildError::AssemblerError(
            String::from_utf8(output.stderr).unwrap_or_else(|_| "Assembler failed".to_string()),
        ));
    }

    for asm_file in &asm_files {
        std::fs::remove_file(asm_file).expect("Build unit returned bad path");
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
    generate_unit(&compile_flags, src_path, &i_path)
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

    if !output.status.success() {
        Err(CompileError::PreprocessorError {
            src_path,
            err: String::from_utf8(output.stderr)
                .unwrap_or_else(|_| "Preprocessor failed".to_string()),
        })
    } else {
        Ok((src_path, i_path))
    }
}

fn generate_unit(
    compile_flags: &CompileFlags,
    src_path: PathBuf,
    i_path: &Path,
) -> Result<PathBuf, CompileError> {
    let src = std::fs::read_to_string(i_path).expect("Preprocessor failed to output to temp name");
    std::fs::remove_file(i_path).expect("Preprocessed source was removed early");

    let ast_tree = Parser::new(&src).parse(src_path)?;
    let asm_path = i_path.with_extension("s");
    Ok(asm_path)
}
