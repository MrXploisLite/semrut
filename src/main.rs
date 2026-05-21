//! SemRut Compiler — smrc
//!
//! Assembly + Rust = SemRut.
//! Hardware-level control with memory safety.

mod lexer;
mod parser;
mod sema;
mod ownership;
mod mir;
mod llvm;

use std::path::PathBuf;
use clap::Parser;
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(name = "smrc")]
#[command(about = "SemRut Compiler — Assembly + Rust = SemRut")]
struct Args {
    /// Input source file
    input: PathBuf,

    /// Output binary path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print tokens and exit
    #[arg(long)]
    dump_tokens: bool,

    /// Print AST and exit
    #[arg(long)]
    dump_ast: bool,

    /// Print checked types and exit
    #[arg(long)]
    dump_types: bool,

    /// Print MIR and exit
    #[arg(long)]
    dump_mir: bool,

    /// Print LLVM IR and exit
    #[arg(long)]
    dump_llvm: bool,

    /// Optimization level (0-3)
    #[arg(short = 'O', default_value = "0")]
    opt_level: u8,
}

fn main() {
    let args = Args::parse();

    // Read source
    let source = match std::fs::read_to_string(&args.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    let filename = args.input.to_string_lossy().to_string();

    // Phase 1: Lexing
    let tokens = match lexer::scan(&source, &filename) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if args.dump_tokens {
        for tok in &tokens {
            println!("{:?}", tok);
        }
        return;
    }

    // Phase 2: Parsing
    let ast = match parser::parse(&tokens, &filename) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if args.dump_ast {
        println!("{:#?}", ast);
        return;
    }

    // Phase 3: Semantic analysis + type checking
    let checked = match sema::check(&ast) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if args.dump_types {
        for func in &checked.functions {
            eprintln!("fn {}(", func.name);
            for (i, (name, ty)) in func.params.iter().enumerate() {
                if i > 0 { eprint!(", "); }
                eprint!("{}: {}", name, ty);
            }
            eprintln!(") -> {} {{", func.ret_type);
            eprintln!("  // {} statements", func.body.stmts.len());
            eprintln!("}}");
        }
        return;
    }

    // Phase 4: Ownership checking
    match ownership::OwnershipChecker::check(&checked) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{} {}", "error:".red().bold(), e);
            std::process::exit(1);
        }
    }

    // Phase 5: MIR construction
    let mir = mir::build(&checked);

    if args.dump_mir {
        println!("{}", mir);
        return;
    }

    // Phase 6: LLVM codegen
    let llvm_module = match llvm::codegen(&mir, args.opt_level) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if args.dump_llvm {
        println!("{}", llvm_module);
        return;
    }

    // Phase 7: Emit binary
    let output = args.output.unwrap_or_else(|| {
        let stem = args.input.file_stem().unwrap().to_string_lossy();
        PathBuf::from(format!("{}", stem))
    });

    match llvm::emit_binary(&llvm_module, &output) {
        Ok(_) => {
            eprintln!("{} compiled to {}", "success".green().bold(), output.display());
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
