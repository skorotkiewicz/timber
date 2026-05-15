use std::env;
use std::fs;
use std::io;
use std::path::Path;

use timber_core::{
    decode_tree, encode_dir, parse_tree_text, try_parse_timber_json, try_parse_tree_j,
};

fn parse_encode_args(args: &[String]) -> (Option<usize>, &str, Vec<String>, bool) {
    let mut depth_limit = None;
    let mut path = ".";
    let mut exclude = Vec::new();
    let mut no_content = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--depth" || args[i] == "-d" {
            if i + 1 < args.len() {
                depth_limit = args[i + 1].parse().ok();
                if depth_limit == Some(0) {
                    depth_limit = None;
                }
                i += 2;
            } else {
                i += 1;
            }
        } else if args[i] == "--exclude" || args[i] == "-e" {
            if i + 1 < args.len() {
                exclude.push(args[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        } else if args[i] == "--no-content" || args[i] == "-n" {
            no_content = true;
            i += 1;
        } else {
            path = &args[i];
            i += 1;
        }
    }
    (depth_limit, path, exclude, no_content)
}

fn cmd_encode(args: &[String]) -> io::Result<()> {
    let (depth_limit, path, exclude, no_content) = parse_encode_args(args);
    let path = Path::new(path);

    if !path.exists() {
        eprintln!("Error: path does not exist: {}", path.display());
        std::process::exit(1);
    }

    let node = encode_dir(path, depth_limit, &exclude, no_content)?;
    let json = serde_json::to_string_pretty(&node)?;
    println!("{json}");

    Ok(())
}

fn cmd_decode(args: &[String]) -> io::Result<()> {
    let input_file = args.first().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing input file argument")
    })?;

    let content = fs::read_to_string(input_file)?;

    if let Some(node) = try_parse_timber_json(&content) {
        let output_dir = args.get(1).cloned().unwrap_or_else(|| "output".to_string());
        fs::create_dir_all(&output_dir)?;
        decode_tree(&node, Path::new(&output_dir))?;
        println!("Decoded tree to: {}", output_dir);
        return Ok(());
    }

    if let Some(node) = try_parse_tree_j(&content) {
        let output_dir = args.get(1).cloned().unwrap_or_else(|| "output".to_string());
        fs::create_dir_all(&output_dir)?;
        decode_tree(&node, Path::new(&output_dir))?;
        println!("Decoded tree to: {}", output_dir);
        return Ok(());
    }

    let node = parse_tree_text(&content);
    let output_dir = args.get(1).cloned().unwrap_or_else(|| "output".to_string());
    fs::create_dir_all(&output_dir)?;
    decode_tree(&node, Path::new(&output_dir))?;
    println!("Decoded tree to: {}", output_dir);

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!(
            "  timber encode [path] [--depth N] [--exclude PATTERN] [--no-content]   Encode directory tree to JSON"
        );
        eprintln!(
            "    --depth N / -d N          Stop after N subdirectory levels (default: unlimited)"
        );
        eprintln!(
            "    --exclude PATTERN / -e    Exclude files/directories matching PATTERN (can use multiple times)"
        );
        eprintln!("    --no-content / -n         Exclude file contents from the output");
        eprintln!(
            "  timber decode <file> [outdir]                         Decode JSON/tree-text to directory"
        );
        std::process::exit(1);
    }

    let cmd = &args[1];
    match cmd.as_str() {
        "encode" => {
            if let Err(e) = cmd_encode(&args[2..]) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        "decode" => {
            if let Err(e) = cmd_decode(&args[2..]) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Unknown command: {cmd}");
            eprintln!("Available commands: encode, decode");
            std::process::exit(1);
        }
    }
}
