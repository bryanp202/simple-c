use std::{
    ffi::OsStr,
    fmt::Display,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::{
    BuildFlags, CompileArgs, CompileFlags,
    arena::{Arena, TypedArena},
    compiler::{
        error::{BuildError, CompileError},
        parser::Parser,
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
mod tacky;
mod token;
mod ty;
mod tychk;

pub fn compile(args: CompileArgs) -> Result<(), BuildError> {
    let build_dir = args.output.clone().map_or(PathBuf::new(), |out_path| {
        out_path
            .parent()
            .map_or(PathBuf::new(), |parent| parent.to_path_buf())
    });
    let temp_dir = tempfile::tempdir_in(&build_dir)
        .map_err(|err| BuildError::TempdirError(err.to_string()))?;

    let compile_flags = args.compile_flags();
    let build_flags = args.build_flags();
    let out_path = args.output;

    let (asm_files, compile_errors) =
        compile_units(compile_flags, args.source, temp_dir.path(), &build_dir);

    if compile_errors.is_empty() {
        assemble(build_flags, &asm_files, out_path)
    } else {
        Err(BuildError::CompileErrors(compile_errors))
    }
}

fn compile_units(
    compile_flags: CompileFlags,
    source: Vec<PathBuf>,
    temp_dir: &Path,
    build_dir: &Path,
) -> (Vec<PathBuf>, Vec<CompileError>) {
    let (asm_files, compile_errors): (Vec<_>, Vec<_>) = source
        .into_par_iter()
        .enumerate()
        .map(|(module_num, src_path)| {
            build_unit(compile_flags, src_path, module_num, temp_dir, build_dir)
        })
        .partition(Result::is_ok);

    let asm_files = asm_files.into_iter().map(Result::unwrap).collect();
    let compile_errors = compile_errors.into_iter().map(Result::unwrap_err).collect();

    (asm_files, compile_errors)
}

fn assemble(
    build_flags: BuildFlags,
    asm_files: &[PathBuf],
    out_path: Option<PathBuf>,
) -> Result<(), BuildError> {
    if build_flags.check_only {
        return Ok(());
    }

    let mut output_path = out_path.unwrap_or_else(|| PathBuf::from("out"));
    output_path.set_extension("exe");

    let output = Command::new("gcc")
        .args(
            asm_files
                .iter()
                .map(|path| path.as_os_str())
                .chain([OsStr::new("-o"), output_path.as_os_str()]),
        )
        .output()
        .map_err(|err| BuildError::AssemblerError(err.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(BuildError::AssemblerError(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

fn build_unit(
    compile_flags: CompileFlags,
    src_path: PathBuf,
    module_num: usize,
    out_dir: &Path,
    build_dir: &Path,
) -> Result<PathBuf, CompileError> {
    let temp_name = out_dir.join(format!("_{module_num}"));
    let (src_path, i_path) = preprocess_unit(src_path, temp_name)?;
    generate_unit(compile_flags, src_path, &i_path, build_dir)
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
            OsStr::new("-E"),
            OsStr::new("-P"),
            src_path.as_os_str(),
            OsStr::new("-o"),
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
    build_dir: &Path,
) -> Result<PathBuf, CompileError> {
    let mut id_interner = Interner::new();
    let src = match std::fs::read_to_string(i_path) {
        Ok(src) => src,
        Err(err) => {
            return Err(CompileError::from_io_error(
                src_path,
                err,
                "failed to open preprocessed source file",
            ));
        }
    };

    let asm_path = if compile_flags.emit_asm {
        build_dir
            .join(src_path.file_name().unwrap_or_default())
            .with_extension("s")
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
        .map_err(|err| CompileError::from_io_error(src_path, err, "failed to write asm to file"))?;

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
    let ast_tree = Parser::new(src, id_interner, &mut ty_interner, &ast_arena, types)
        .parse(src_path.to_path_buf())?;
    if compile_flags.show_pretty_ast {
        pretty_print(&ast_tree, "ast", src_path);
    }

    // Semantic pass
    let checked_ast_tree = TyChecker::new(&mut ty_interner, &ast_arena).check(
        src,
        src_path.to_path_buf(),
        ast_tree,
    )?;

    // Convert into a three address code IR
    let tacky_program = ast::Converter::new().convert(checked_ast_tree);
    if compile_flags.show_pretty_tacky {
        pretty_print(&tacky_program, "tacky", src_path);
    }
    Ok(tacky_program)
}

pub fn pretty_print(item: impl Display, name: &str, src_path: &Path) {
    let lock = std::io::stderr().lock();
    let mut writer = BufWriter::new(lock);
    write!(&mut writer, "{}:\n{item}", src_path.display()).unwrap_or_else(|err| {
        eprintln!("{err}: failed to print {name} for: {}", src_path.display());
    });
}
