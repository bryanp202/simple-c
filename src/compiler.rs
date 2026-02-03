use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use clap::builder::OsStr;

use crate::{
    CompileArgs, CompileFlags,
    arena::{Arena, TypedArena},
    compiler::{
        error::{BuildError, CompileError},
        parser::Parser,
        pretty::pretty_print,
        tacky::AsmConverter,
        ty::built_in_types,
        tychk::TyChecker,
    },
    intern::{InternedArena, Interner},
};

mod asm;
mod ast;
mod error;
mod lexer;
mod parser;
mod pretty;
mod tacky;
mod token;
mod ty;
mod tychk;

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
            String::from_utf8_lossy(&output.stderr).to_string(),
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
    generate_unit(compile_flags, src_path, &i_path)
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
    src_path: PathBuf,
    i_path: &Path,
) -> Result<PathBuf, CompileError> {
    let mut id_interner = Interner::new();
    let src = match std::fs::read_to_string(&src_path) {
        Ok(src) => src,
        Err(err) => {
            return Err(CompileError::IoError {
                src_path,
                err,
                msg: "failed to open preprocessed source file",
            });
        }
    };
    _ = std::fs::remove_file(i_path);

    let asm_path = if compile_flags.emit_asm {
        src_path.with_extension("s")
    } else {
        i_path.with_extension("s")
    };

    // Put into function so ty and ast arena are dropped earlier
    let tacky_program = generate_tacky(compile_flags, &src, &src_path, &mut id_interner)?;

    // Convert into x86_64 asm IR
    let asm_program = AsmConverter::new().convert(tacky_program);
    if compile_flags.show_pretty_asm {
        pretty_print(&asm_program, "asm", &src_path);
    }

    // Output x86_64 asm to file
    asm_program
        .generate(&asm_path)
        .map_err(|err| CompileError::IoError {
            src_path,
            err,
            msg: "failed to write asm to file",
        })?;

    Ok(asm_path)
}

fn generate_tacky<'src>(
    compile_flags: CompileFlags,
    src: &'src str,
    src_path: &Path,
    id_interner: &'src mut Interner<'src, str>,
) -> Result<tacky::Program<'src>, CompileError> {
    let ty_arena = TypedArena::new();
    let mut ty_interner = InternedArena::new(&ty_arena);
    let ast_arena = Arena::new();

    // Parse into an ast
    let types = built_in_types(id_interner, &mut ty_interner);
    let ast_tree = Parser::new(&src, id_interner, &mut ty_interner, &ast_arena, types)
        .parse(src_path.to_path_buf())?;
    if compile_flags.show_pretty_ast {
        pretty_print(&ast_tree, "ast", &src_path);
    }

    // Semantic pass
    let checked_ast_tree = TyChecker::new().check(&src, src_path.to_path_buf(), ast_tree)?;

    // Convert into a three address code IR
    let tacky_program = ast::Converter::new().convert(checked_ast_tree);
    if compile_flags.show_pretty_tacky {
        pretty_print(&tacky_program, "tacky", &src_path);
    }
    Ok(tacky_program)
}
