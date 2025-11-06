use git2::{Delta, Repository};
use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeType,
    pub content: Option<String>,
}

/// Get list of files that were added or changed between two commits
/// This function clones the repository from the given URL and compares the commits
pub fn get_git_changed_files(
    repo_url: &str,
    commit_hash_1: &str,
    commit_hash_2: &str,
) -> Result<Vec<FileChange>, Box<dyn std::error::Error>> {
    // Create a temporary directory for cloning
    let temp_dir = format!("/tmp/git_analysis_{}", std::process::id());

    // Clean up any existing temp directory
    if Path::new(&temp_dir).exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    // Clone the repository
    let repo = Repository::clone(repo_url, &temp_dir)?;

    let commit1 = repo.find_commit(repo.revparse_single(commit_hash_1)?.id())?;
    let commit2 = repo.find_commit(repo.revparse_single(commit_hash_2)?.id())?;

    let tree1 = commit1.tree()?;
    let tree2 = commit2.tree()?;

    let diff = repo.diff_tree_to_tree(Some(&tree1), Some(&tree2), None)?;

    let mut file_changes = Vec::new();

    diff.foreach(
        &mut |delta, _| {
            let (path, change_type) = match delta.status() {
                Delta::Added => {
                    if let Some(path) = delta.new_file().path() {
                        (path.to_string_lossy().to_string(), ChangeType::Added)
                    } else {
                        return true; // Skip if no path
                    }
                }
                Delta::Modified => {
                    if let Some(path) = delta.new_file().path() {
                        (path.to_string_lossy().to_string(), ChangeType::Modified)
                    } else {
                        return true; // Skip if no path
                    }
                }
                Delta::Deleted => {
                    if let Some(path) = delta.old_file().path() {
                        (path.to_string_lossy().to_string(), ChangeType::Deleted)
                    } else {
                        return true; // Skip if no path
                    }
                }
                _ => return true, // Skip other types
            };

            // Get file content for added and modified files
            let content = match change_type {
                ChangeType::Added | ChangeType::Modified => {
                    // Get the file content from the second commit (newer version)
                    match tree2.get_path(Path::new(&path)) {
                        Ok(entry) => {
                            if let Ok(blob) =
                                entry.to_object(&repo).and_then(|obj| obj.peel_to_blob())
                            {
                                // Try to convert to UTF-8 string, skip binary files
                                if blob.is_binary() {
                                    Some("[Binary file]".to_string())
                                } else {
                                    std::str::from_utf8(blob.content())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|_| "[Non-UTF8 content]".to_string())
                                        .into()
                                }
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                }
                ChangeType::Deleted => None, // No content for deleted files
            };

            file_changes.push(FileChange {
                path,
                status: change_type,
                content,
            });

            true
        },
        None,
        None,
        None,
    )?;

    // Clean up the temporary directory
    std::fs::remove_dir_all(&temp_dir).ok();

    Ok(file_changes)
}

pub fn split_by_function(content: &str) -> Vec<String> {
    let mut blocks = vec![];

    let re = Regex::new(r#"(?m)^(pub\s+)?(async\s+)?(fn\s+\w+|function\s+\w+|const\s+\w+\s*=\s*\(|let\s+\w+\s*=\s*\(|export\s+(async\s+)?function\s+\w+)"#).unwrap();
    let mut last_index = 0;

    for mat in re.find_iter(content) {
        let start = mat.start();
        if start > last_index {
            let chunk = &content[last_index..start];
            if !chunk.trim().is_empty() {
                blocks.push(chunk.to_string());
            }
        }
        last_index = start;
    }

    if last_index < content.len() {
        blocks.push(content[last_index..].to_string());
    }

    blocks
}
