use std::env;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TreeNode {
    name: String,
    #[serde(rename = "type")]
    node_type: NodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<TreeNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum NodeType {
    Dir,
    File,
}

fn encode_dir(path: &Path) -> io::Result<TreeNode> {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    if path.is_dir() {
        let mut children = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let entry_path = entry.path();
            if entry_path.extension().map_or(false, |ext| ext == "json")
                && name == "."
            {
                continue;
            }
            children.push(encode_dir(&entry_path)?);
        }

        Ok(TreeNode {
            name,
            node_type: NodeType::Dir,
            content: None,
            children: Some(children),
        })
    } else {
        let content = fs::read_to_string(path).ok();
        Ok(TreeNode {
            name,
            node_type: NodeType::File,
            content,
            children: None,
        })
    }
}

fn decode_tree(node: &TreeNode, base_path: &Path) -> io::Result<()> {
    let full_path = base_path.join(&node.name);

    match node.node_type {
        NodeType::Dir => {
            fs::create_dir_all(&full_path)?;
            if let Some(children) = &node.children {
                for child in children {
                    decode_tree(child, &full_path)?;
                }
            }
        }
        NodeType::File => {
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Some(content) = &node.content {
                fs::write(&full_path, content)?;
            } else {
                fs::write(&full_path, "")?;
            }
        }
    }

    Ok(())
}

/// Strips tree-drawing prefixes from a line and returns (indent_depth, name).
/// Each indent level is 4 display-char wide: "│   ", "├── ", "└── ", or "    ".
fn strip_tree_prefix(line: &str) -> (usize, String) {
    let mut indent = 0;
    let mut pos = 0;

    while pos < line.len() {
        let remaining: Vec<char> = line[pos..].chars().collect();
        if remaining.len() < 4 {
            break;
        }

        let prefix: String = remaining[..4].iter().collect();

        if prefix.starts_with("│   ")
            || prefix.starts_with("├── ")
            || prefix.starts_with("└── ")
        {
            indent += 1;
            // Advance past the 4-char prefix by byte count
            let byte_advance: usize = remaining[..4].iter().map(|c| c.len_utf8()).sum();
            pos += byte_advance;
        } else if prefix == "    " {
            indent += 1;
            pos += 4;
        } else {
            break;
        }
    }

    let name = line[pos..].trim_end().to_string();
    let name = name.trim_end_matches('/').to_string();
    (indent, name)
}

fn parse_tree_text(input: &str) -> TreeNode {
    #[derive(Debug)]
    struct StackEntry {
        indent: usize,
        node: TreeNode,
    }

    let mut root = TreeNode {
        name: ".".to_string(),
        node_type: NodeType::Dir,
        content: None,
        children: Some(Vec::new()),
    };

    let mut stack: Vec<StackEntry> = Vec::new();

    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let (indent, name) = strip_tree_prefix(line);

        if name.is_empty() {
            continue;
        }

        let is_dir = name == "." || (!name.contains('.') || name.ends_with('.'));
        let name = if name == "." {
            ".".to_string()
        } else {
            name.trim_end_matches('/').to_string()
        };

        let node = TreeNode {
            name: name.clone(),
            node_type: if is_dir {
                NodeType::Dir
            } else {
                NodeType::File
            },
            content: None,
            children: if is_dir {
                Some(Vec::new())
            } else {
                None
            },
        };

        // Pop stack entries until we find a parent (lower indent level)
        while stack.last().map_or(false, |e| e.indent >= indent) {
            if let Some(popped) = stack.pop() {
                if let Some(parent) = stack.last_mut() {
                    parent
                        .node
                        .children
                        .get_or_insert_with(Vec::new)
                        .push(popped.node);
                } else {
                    root.children.get_or_insert_with(Vec::new).push(popped.node);
                }
            }
        }

        stack.push(StackEntry { indent, node });
    }

    // Flush remaining stack entries
    while let Some(entry) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent
                .node
                .children
                .get_or_insert_with(Vec::new)
                .push(entry.node);
        } else {
            root.children.get_or_insert_with(Vec::new).push(entry.node);
        }
    }

    root
}

fn cmd_encode(args: &[String]) -> io::Result<()> {
    let path = args.first().map_or(".", String::as_str);
    let path = Path::new(path);

    if !path.exists() {
        eprintln!("Error: path does not exist: {}", path.display());
        std::process::exit(1);
    }

    let node = encode_dir(path)?;
    let json = serde_json::to_string_pretty(&node)?;
    println!("{json}");

    Ok(())
}

fn cmd_decode(args: &[String]) -> io::Result<()> {
    let input_file = args.first().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing input file argument")
    })?;

    let content = fs::read_to_string(input_file)?;

    // Try parsing as JSON first
    if let Ok(node) = serde_json::from_str::<TreeNode>(&content) {
        let output_dir = args.get(1).cloned().unwrap_or_else(|| "output".to_string());
        decode_tree(&node, Path::new(&output_dir))?;
        println!("Decoded tree to: {}", output_dir);
        return Ok(());
    }

    // Fall back to parsing as plain tree text
    let node = parse_tree_text(&content);
    let output_dir = args.get(1).cloned().unwrap_or_else(|| "output".to_string());
    decode_tree(&node, Path::new(&output_dir))?;
    println!("Decoded tree to: {}", output_dir);

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  tree-rs encode [path]          Encode directory tree to JSON");
        eprintln!("  tree-rs decode <file> [outdir]  Decode JSON/tree-text to directory");
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