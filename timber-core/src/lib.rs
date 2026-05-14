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
                children.push(encode_dir(&entry_path, next_depth, exclude)?);
            } else if is_dir {
                children.push(TreeNode {
                    name: entry_name,
                    node_type: NodeType::Dir,
                    content: None,
                    children: None,
                });
            } else {
                let content = fs::read_to_string(&entry_path).ok();
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
        let content = fs::read_to_string(path).ok();
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

    let mut root = TreeNode {
        name: ".".to_string(),
        node_type: NodeType::Dir,
        content: None,
        children: Some(Vec::new()),
    };

    let mut stack: Vec<StackEntry> = Vec::new();

    for line in input.lines() {
        let (indent, name) = match strip_tree_prefix(line) {
            Some(pair) => pair,
            None => continue,
        };

        let is_dir = name == "." || !name.contains('.');
        let name = name.trim_end_matches('/').to_string();

        let node = TreeNode {
            name: name.clone(),
            node_type: if is_dir {
                NodeType::Dir
            } else {
                NodeType::File
            },
            content: None,
            children: if is_dir { Some(Vec::new()) } else { None },
        };

        while stack.last().is_some_and(|e| e.indent >= indent) {
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

fn strip_tree_prefix(line: &str) -> Option<(usize, String)> {
    if !line.contains('│') && !line.contains('├') && !line.contains('└') {
        let trimmed = line.trim();
        if trimmed == "." {
            return Some((0, ".".to_string()));
        }
        return None;
    }

    let mut indent = 0;
    let mut chars = line.char_indices().peekable();

    while let Some(&(_, ch)) = chars.peek() {
        match ch {
            '│' => {
                let rest: String = chars.clone().map(|(_, c)| c).take(4).collect();
                if rest.starts_with("│   ") {
                    indent += 1;
                    for _ in 0..4 {
                        chars.next();
                    }
                } else {
                    break;
                }
            }
            '├' => {
                let rest: String = chars.clone().map(|(_, c)| c).take(4).collect();
                if rest.starts_with("├── ") {
                    indent += 1;
                    for _ in 0..4 {
                        chars.next();
                    }
                } else {
                    break;
                }
            }
            '└' => {
                let rest: String = chars.clone().map(|(_, c)| c).take(4).collect();
                if rest.starts_with("└── ") {
                    indent += 1;
                    for _ in 0..4 {
                        chars.next();
                    }
                } else {
                    break;
                }
            }
            ' ' => {
                let rest: String = chars.clone().map(|(_, c)| c).take(4).collect();
                if rest == "    " {
                    indent += 1;
                    for _ in 0..4 {
                        chars.next();
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    let remaining: String = chars.map(|(_, c)| c).collect();
    let name = remaining.trim_end().trim_end_matches('/').to_string();

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
