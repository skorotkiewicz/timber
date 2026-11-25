use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Dir,
    File,
}

// tree -J format
#[derive(Debug, Clone, Deserialize)]
struct TreeJNode {
    name: String,
    #[serde(rename = "type")]
    node_type: TreeJNodeType,
    #[serde(default)]
    contents: Option<Vec<TreeJNode>>,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TreeJNodeType {
    Directory,
    File,
}

impl From<TreeJNode> for TreeNode {
    fn from(j: TreeJNode) -> Self {
        let node_type = match j.node_type {
            TreeJNodeType::Directory => NodeType::Dir,
            TreeJNodeType::File => NodeType::File,
        };
        TreeNode {
            name: j.name,
            node_type,
            content: j.content,
            children: j
                .contents
                .map(|cs| cs.into_iter().map(|c| c.into()).collect()),
        }
    }
}

pub fn encode_dir(
    path: &Path,
    depth_limit: Option<usize>,
    exclude: &[String],
    no_content: bool,
) -> io::Result<TreeNode> {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    if path.is_dir() {
        let mut children = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.path());

        let can_descend = depth_limit.is_none_or(|d| d > 0);
        let next_depth = depth_limit.and_then(|d| if d > 0 { Some(d - 1) } else { None });

        for entry in entries {
            let entry_path = entry.path();

            let entry_name = entry_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();

            if exclude.iter().any(|ex| &entry_name == ex) {
                continue;
            }

            let is_dir = entry_path.is_dir();

            if is_dir && can_descend {
                children.push(encode_dir(&entry_path, next_depth, exclude, no_content)?);
            } else if is_dir {
                children.push(TreeNode {
                    name: entry_name,
                    node_type: NodeType::Dir,
                    content: None,
                    children: None,
                });
            } else {
                let content = if no_content {
                    None
                } else {
                    fs::read_to_string(&entry_path).ok()
                };
                children.push(TreeNode {
                    name: entry_name,
                    node_type: NodeType::File,
                    content,
                    children: None,
                });
            }
        }

        Ok(TreeNode {
            name,
            node_type: NodeType::Dir,
            content: None,
            children: Some(children),
        })
    } else {
        let content = if no_content {
            None
        } else {
            fs::read_to_string(path).ok()
        };
        Ok(TreeNode {
            name,
            node_type: NodeType::File,
            content,
            children: None,
        })
    }
}

pub fn decode_tree(node: &TreeNode, base_path: &Path) -> io::Result<()> {
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

pub fn parse_tree_text(input: &str) -> TreeNode {
    #[derive(Debug)]
    struct StackEntry {
        indent: usize,
        node: TreeNode,
    }

    let mut root_name = ".".to_string();
    let mut saw_root = false;
    let mut entries: Vec<(usize, String)> = Vec::new();

    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((indent, name)) = strip_tree_prefix(line) else {
            continue;
        };
        if indent > 0 {
            entries.push((indent, name));
        } else if !saw_root && !is_summary_line(line.trim()) {
            // First line of a tree dump is the root directory name.
            root_name = name.trim_end_matches('/').to_string();
            saw_root = true;
        }
    }

    // A trailing '/' on any entry means the dump came from `tree -F`,
    // which marks directories explicitly. Otherwise use the dot heuristic.
    let f_mode = entries.iter().any(|(_, n)| n.ends_with('/'));

    let mut root = TreeNode {
        name: root_name,
        node_type: NodeType::Dir,
        content: None,
        children: Some(Vec::new()),
    };

    let mut stack: Vec<StackEntry> = Vec::new();

    for i in 0..entries.len() {
        let (indent, raw) = &entries[i];
        let has_children = entries
            .get(i + 1)
            .is_some_and(|(next_indent, _)| next_indent > indent);

        // Symlinks are printed as "name -> target".
        let name = raw.split(" -> ").next().unwrap_or(raw.as_str());
        let trailing_slash = name.ends_with('/');
        let mut name = name.trim_end_matches('/').to_string();
        if f_mode
            && !trailing_slash
            && let Some(stripped) = name.strip_suffix(['*', '=', '@', '|', '>'])
        {
            name = stripped.to_string();
        }

        let is_dir =
            has_children || trailing_slash || (!f_mode && (name == "." || !name.contains('.')));

        let node = TreeNode {
            name,
            node_type: if is_dir {
                NodeType::Dir
            } else {
                NodeType::File
            },
            content: None,
            children: if is_dir { Some(Vec::new()) } else { None },
        };

        while stack.last().is_some_and(|e| e.indent >= *indent) {
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
        stack.push(StackEntry {
            indent: *indent,
            node,
        });
    }

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

fn is_summary_line(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|t| t.parse::<usize>().is_ok())
        && line.contains("director")
}

fn strip_tree_prefix(line: &str) -> Option<(usize, String)> {
    // tree draws vertical continuations as '│' + two U+00A0 (NBSP) + one space.
    // Normalize NBSP to plain spaces so prefix matching works at any depth.
    let normalized: String = line
        .chars()
        .map(|c| if c == '\u{a0}' { ' ' } else { c })
        .collect();

    // UTF-8 groups first, then the ASCII charset (LANG=C tree) equivalents.
    const GROUPS: [&str; 7] = ["│   ", "├── ", "└── ", "    ", "|   ", "|-- ", "`-- "];

    let mut indent = 0usize;
    let mut rest = normalized.as_str();
    'outer: loop {
        for group in GROUPS {
            if let Some(r) = rest.strip_prefix(group) {
                indent += 1;
                rest = r;
                continue 'outer;
            }
        }
        break;
    }

    let name = rest.trim_end().to_string();
    if name.is_empty() {
        None
    } else {
        Some((indent, name))
    }
}

pub fn try_parse_timber_json(content: &str) -> Option<TreeNode> {
    serde_json::from_str::<TreeNode>(content).ok()
}

pub fn try_parse_tree_j(content: &str) -> Option<TreeNode> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(content).ok()?;
    let root_val = arr.first()?;
    let root: TreeJNode = serde_json::from_value(root_val.clone()).ok()?;
    Some(root.into())
}
