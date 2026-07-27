use std::env;
use std::fs;
use std::path::Path;
use std::process;

use ir::parse;
use query::Session;

fn print_usage(program_name: &str) {
    eprintln!("DQIR Compiler CLI Driver");
    eprintln!("Usage:");
    eprintln!("  {} run <file.dqir> [args...]", program_name);
    eprintln!("  {} compile <file.dqir> [-o <output.bin>]", program_name);
    eprintln!("  {} check <file.dqir>", program_name);
}

fn check_extension(filepath: &str) {
    if !filepath.ends_with(".dqir") {
        eprintln!(
            "Warning: Input file '{}' does not have the conventional '.dqir' file extension.",
            filepath
        );
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage(&args[0]);
        process::exit(1);
    }

    let command = &args[1];
    let filepath = &args[2];
    check_extension(filepath);

    let source_text = match fs::read_to_string(filepath) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filepath, e);
            process::exit(1);
        }
    };

    let session = Session::new(&source_text);

    match command.as_str() {
        "check" => {
            match parse(&source_text) {
                Ok(module) => {
                    println!("Successfully parsed .dqir file '{}': {} function(s) defined.", filepath, module.functions.len());
                }
                Err(e) => {
                    eprintln!("Syntax error in .dqir file '{}': {}", filepath, e);
                    process::exit(1);
                }
            }
        }
        "compile" => {
            let mut out_path = if let Some(stem) = Path::new(filepath).file_stem() {
                format!("{}.bin", stem.to_string_lossy())
            } else {
                "out.bin".to_string()
            };

            let mut i = 3;
            while i < args.len() {
                if args[i] == "-o" && i + 1 < args.len() {
                    out_path = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Unknown argument: {}", args[i]);
                    process::exit(1);
                }
            }

            // Trigger compilation via Salsa query session
            let bytes = match session.compile_module() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Compilation error in '{}': {}", filepath, e);
                    process::exit(1);
                }
            };

            if let Err(e) = fs::write(&out_path, &bytes) {
                eprintln!("Error writing output binary '{}': {}", out_path, e);
                process::exit(1);
            }
            println!("Compiled '{}' to machine code binary '{}' ({} bytes).", filepath, out_path, bytes.len());
        }
        "run" => {
            let module = match parse(&source_text) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Syntax error in .dqir file '{}': {}", filepath, e);
                    process::exit(1);
                }
            };

            let (_, func) = match module.functions.iter().next() {
                Some(res) => res,
                None => {
                    eprintln!("Error: No functions defined in '{}'", filepath);
                    process::exit(1);
                }
            };

            let exec = match session.compile_fn(&func.name) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Compilation failure: {}", e);
                    process::exit(1);
                }
            };

            let mut call_args = Vec::new();
            for arg_str in &args[3..] {
                match arg_str.parse::<u64>() {
                    Ok(val) => call_args.push(val),
                    Err(_) => {
                        eprintln!("Error: argument '{}' is not a valid 64-bit integer", arg_str);
                        process::exit(1);
                    }
                }
            }

            let result = exec.run(&call_args);
            println!("{}", result);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage(&args[0]);
            process::exit(1);
        }
    }
}
