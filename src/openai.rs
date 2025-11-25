use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
    },
};

use crate::{ChangeType, FileChange, git::split_by_function};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeAnalysis {
    pub is_good: bool,
    pub description: String,
    pub suggestions: Option<String>,
    pub confidence: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub change_type: ChangeType,
    pub analysis: Option<CodeAnalysis>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepositoryAnalysisResult {
    pub files: Vec<FileAnalysisResult>,
    pub is_good: bool,
    pub total_files: i32,
    pub analyzed_files: i32,
    pub good_files: i32,
    pub files_with_issues: i32,
}

/// Internal async OpenAI function
pub async fn ask_openai_internal(
    prompt: &str,
    api_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = OpenAIConfig::new().with_api_key(api_key);

    let client = Client::with_config(config);

    let messages = vec![ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
            name: None,
        },
    )];

    let request = CreateChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages,
        ..Default::default()
    };

    let response = client.chat().create(request).await?;
    let reply = response
        .choices
        .get(0)
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "No response.".to_string());

    Ok(reply)
}

pub async fn analyze_file_change_with_ai(
    file_change: &FileChange,
    api_key: &str,
) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    let content = match &file_change.content {
        Some(c) => c,
        None => {
            return Ok(CodeAnalysis {
                is_good: true,
                description: "[No content to analyze]".to_string(),
                suggestions: None,
                confidence: 1.0,
            });
        }
    };

    if content == "[Binary file]" || content == "[Non-UTF8 content]" {
        return Ok(CodeAnalysis {
            is_good: true, // Not bad, just not analyzable as code
            description: format!("Skipped binary or unreadable file: {}", file_change.path),
            suggestions: Some(
                "Consider if this binary file should be tracked in version control".to_string(),
            ),
            confidence: 1.0,
        });
    }

    let blocks = if content.len() > 12_000 {
        split_by_function(content)
    } else {
        vec![content.clone()]
    };

    let mut analyses = vec![];

    for (i, block) in blocks.iter().enumerate() {
        let prompt = format!(
            r#"Analyze the following code block (part {}/{} from file {}) and provide a JSON response with this exact structure:
{{
    "is_good": true/false,
    "description": "Brief description of what the code does and its quality",
    "suggestions": "Optional suggestions for improvement or null",
    "confidence": 0.85
}}

Code to analyze:
```
{}
```

Focus on:
1. Code quality and best practices
2. Potential bugs or issues
3. Readability and maintainability
4. Security concerns if any

Respond ONLY with valid JSON:"#,
            i + 1,
            blocks.len(),
            file_change.path,
            block
        );

        let response = ask_openai_internal(&prompt, api_key).await?;
        analyses.push(response);
    }

    // Parse the JSON response from OpenAI
    let combined_analysis = if analyses.len() == 1 {
        parse_analysis_response(&analyses[0])?
    } else {
        // For multiple blocks, combine the analyses
        combine_multiple_analyses(&analyses)?
    };

    Ok(combined_analysis)
}

fn parse_analysis_response(response: &str) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    // Try to extract JSON from the response
    let json_str = extract_json_from_response(response);

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(json) => {
            let is_good = json["is_good"].as_bool().unwrap_or(false);
            let description = json["description"]
                .as_str()
                .unwrap_or("No description provided")
                .to_string();
            let suggestions = json["suggestions"].as_str().map(|s| s.to_string());
            let confidence = json["confidence"].as_f64().unwrap_or(0.5) as f32;

            Ok(CodeAnalysis {
                is_good,
                description,
                suggestions,
                confidence,
            })
        }
        Err(_) => {
            // Fallback: create analysis from plain text response
            Ok(CodeAnalysis {
                is_good: !response.to_lowercase().contains("error")
                    && !response.to_lowercase().contains("issue")
                    && !response.to_lowercase().contains("problem"),
                description: response.to_string(),
                suggestions: None,
                confidence: 0.3, // Low confidence for non-structured response
            })
        }
    }
}

fn extract_json_from_response(response: &str) -> String {
    // Look for JSON block between ```json and ``` or just find { ... }
    if let Some(start) = response.find('{') {
        if let Some(end) = response.find('}') {
            if end > start {
                return response[start..=end].to_string();
            }
        }
    }

    // If no JSON found, return the original response
    response.to_string()
}

fn combine_multiple_analyses(
    analyses: &[String],
) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    let parsed_analyses: Vec<CodeAnalysis> = analyses
        .iter()
        .map(|a| parse_analysis_response(a))
        .collect::<Result<Vec<_>, _>>()?;

    let overall_good = parsed_analyses.iter().all(|a| a.is_good);
    let avg_confidence =
        parsed_analyses.iter().map(|a| a.confidence).sum::<f32>() / parsed_analyses.len() as f32;

    let combined_description = parsed_analyses
        .iter()
        .enumerate()
        .map(|(i, a)| format!("Block {}: {}", i + 1, a.description))
        .collect::<Vec<_>>()
        .join("\n\n");

    let combined_suggestions = parsed_analyses
        .iter()
        .filter_map(|a| a.suggestions.as_ref())
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    Ok(CodeAnalysis {
        is_good: overall_good,
        description: combined_description,
        suggestions: if combined_suggestions.is_empty() {
            None
        } else {
            Some(combined_suggestions.join("\n"))
        },
        confidence: avg_confidence,
    })
}

/// Analyze all changes between two commits in a git repository using AI
///
/// # Arguments
/// * `api_key` - OpenAI API key
/// * `repo_url` - Git repository URL
/// * `commit1` - First commit hash (older)
/// * `commit2` - Second commit hash (newer)
///
/// # Returns
/// * `RepositoryAnalysisResult` - Comprehensive analysis of all changed files
pub async fn analyze_repository_changes(
    api_key: &str,
    repo_url: &str,
    commit1: &str,
    commit2: &str,
) -> Result<RepositoryAnalysisResult, Box<dyn std::error::Error>> {
    // Get changed files from git
    let file_changes = crate::git::get_git_changed_files(repo_url, commit1, commit2)?;

    let mut results = Vec::new();
    let mut has_any_issues = false;
    let mut analyzed_count = 0;
    let mut good_count = 0;

    for file_change in &file_changes {
        match &file_change.status {
            ChangeType::Deleted => {
                // Skip deleted files - they don't affect the "is_good" status
                results.push(FileAnalysisResult {
                    file_path: file_change.path.clone(),
                    change_type: file_change.status.clone(),
                    analysis: None,
                    error: None,
                });
            }
            _ => {
                // Analyze the file
                match analyze_file_change_with_ai(file_change, api_key).await {
                    Ok(analysis) => {
                        analyzed_count += 1;

                        if analysis.is_good {
                            good_count += 1;
                        } else {
                            has_any_issues = true;
                        }

                        results.push(FileAnalysisResult {
                            file_path: file_change.path.clone(),
                            change_type: file_change.status.clone(),
                            analysis: Some(analysis),
                            error: None,
                        });
                    }
                    Err(e) => {
                        // Analysis errors count as issues
                        has_any_issues = true;

                        results.push(FileAnalysisResult {
                            file_path: file_change.path.clone(),
                            change_type: file_change.status.clone(),
                            analysis: None,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }
    }

    Ok(RepositoryAnalysisResult {
        files: results,
        is_good: !has_any_issues,
        total_files: file_changes.len() as i32,
        analyzed_files: analyzed_count,
        good_files: good_count,
        files_with_issues: analyzed_count - good_count,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestTargets {
    pub functions: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestTargetsWithCode {
    pub targets: TestTargets,
    pub file_contents: Vec<FileContent>,
    pub function_contents: Vec<FunctionContent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionContent {
    pub name: String,
    pub file_path: Option<String>,
    pub content: Option<String>,
    pub error: Option<String>,
}

pub async fn extract_test_targets_with_ai(
    prompt: &str,
    api_key: &str,
) -> Result<TestTargets, Box<dyn std::error::Error>> {
    let extraction_prompt = format!(
        r#"Extract from the following prompt the list of function names and file names that the user expects to work.

Respond ONLY in this strict JSON format:
{{
  "functions": ["..."],
  "files": ["..."]
}}

Prompt:
"{prompt}"
"#,
        prompt = prompt
    );

    let raw_response = ask_openai_internal(&extraction_prompt, api_key).await?;

    let parsed: TestTargets = serde_json::from_str(&raw_response)?;

    Ok(parsed)
}

/// Read the actual code content for the test targets
///
/// # Arguments
/// * `targets` - The TestTargets containing function and file names
/// * `src_path` - Path to the source code directory
///
/// # Returns
/// * `TestTargetsWithCode` - The targets with their actual code content
pub fn read_test_targets_code(
    targets: &TestTargets,
    src_path: &str,
) -> Result<TestTargetsWithCode, Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::Path;

    let src_dir = Path::new(src_path);

    // Read file contents
    let mut file_contents = Vec::new();
    for file_path in &targets.files {
        let full_path = src_dir.join(file_path);

        match fs::read_to_string(&full_path) {
            Ok(content) => {
                file_contents.push(FileContent {
                    path: file_path.clone(),
                    content,
                    error: None,
                });
            }
            Err(e) => {
                file_contents.push(FileContent {
                    path: file_path.clone(),
                    content: String::new(),
                    error: Some(format!("Failed to read file: {}", e)),
                });
            }
        }
    }

    // Extract function contents by searching through source files
    let mut function_contents = Vec::new();
    for function_name in &targets.functions {
        let (found_file, found_content) = find_function_in_directory(src_dir, function_name)?;

        function_contents.push(FunctionContent {
            name: function_name.clone(),
            file_path: found_file.clone(),
            content: found_content.clone(),
            error: if found_content.is_none() {
                Some(format!(
                    "Function '{}' not found in source directory",
                    function_name
                ))
            } else {
                None
            },
        });
    }

    Ok(TestTargetsWithCode {
        targets: targets.clone(),
        file_contents,
        function_contents,
    })
}

/// Search for a function definition in a directory recursively
fn find_function_in_directory(
    dir: &std::path::Path,
    function_name: &str,
) -> Result<(Option<String>, Option<String>), Box<dyn std::error::Error>> {
    use std::fs;

    if !dir.is_dir() {
        return Ok((None, None));
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip target and hidden directories
            if let Some(dir_name) = path.file_name() {
                let dir_name = dir_name.to_string_lossy();
                if dir_name == "target" || dir_name.starts_with('.') {
                    continue;
                }
            }

            // Recursively search subdirectories
            let (found_file, found_content) = find_function_in_directory(&path, function_name)?;
            if found_content.is_some() {
                return Ok((found_file, found_content));
            }
        } else if is_source_file(&path) {
            // Search in source code files
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(function_content) =
                    extract_function_from_content(&content, function_name, &path)
                {
                    let relative_path = path
                        .strip_prefix(dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    return Ok((Some(relative_path), Some(function_content)));
                }
            }
        }
    }

    Ok((None, None))
}

/// Check if a file is a source code file (TypeScript, Rust, Python)
fn is_source_file(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext, "rs" | "py" | "ts" | "tsx" | "js" | "jsx")
    } else {
        false
    }
}

/// Extract a function's content from source code (supports Rust, Python, TypeScript/JavaScript)
fn extract_function_from_content(
    content: &str,
    function_name: &str,
    file_path: &std::path::Path,
) -> Option<String> {
    let ext = file_path.extension()?.to_str()?;

    match ext {
        "rs" => extract_rust_function(content, function_name),
        "py" => extract_python_function(content, function_name),
        "js" | "ts" | "jsx" | "tsx" => extract_javascript_function(content, function_name),
        _ => None,
    }
}

/// Extract Rust function
fn extract_rust_function(content: &str, function_name: &str) -> Option<String> {
    // Look for function definitions: pub fn, async fn, fn
    let patterns = [
        format!(r"pub async fn {}(", function_name),
        format!(r"pub fn {}(", function_name),
        format!(r"async fn {}(", function_name),
        format!(r"fn {}(", function_name),
        format!(r"pub unsafe fn {}(", function_name),
        format!(r"unsafe fn {}(", function_name),
    ];

    for pattern in &patterns {
        if let Some(start_pos) = content.find(pattern) {
            // Find the start of the function (look backwards for any attributes or doc comments)
            let mut func_start = start_pos;
            let lines: Vec<&str> = content[..start_pos].lines().collect();

            // Look backwards for attributes and doc comments
            for line in lines.iter().rev() {
                let trimmed = line.trim();
                if trimmed.starts_with("#[")
                    || trimmed.starts_with("///")
                    || trimmed.starts_with("//!")
                    || trimmed.is_empty()
                {
                    if let Some(pos) = content[..func_start].rfind(trimmed) {
                        func_start = pos;
                    }
                } else {
                    break;
                }
            }

            // Find the end of the function by counting braces
            let remaining = &content[start_pos..];
            if let Some(first_brace) = remaining.find('{') {
                let mut brace_count = 0;
                let mut in_string = false;
                let mut in_char = false;
                let mut escape_next = false;
                let mut func_end = start_pos + first_brace;

                for (i, ch) in remaining[first_brace..].char_indices() {
                    if escape_next {
                        escape_next = false;
                        continue;
                    }

                    match ch {
                        '\\' => escape_next = true,
                        '"' if !in_char => in_string = !in_string,
                        '\'' if !in_string => in_char = !in_char,
                        '{' if !in_string && !in_char => brace_count += 1,
                        '}' if !in_string && !in_char => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                func_end = start_pos + first_brace + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                if brace_count == 0 {
                    return Some(content[func_start..func_end].to_string());
                }
            }
        }
    }

    None
}

/// Extract Python function (def or async def)
fn extract_python_function(content: &str, function_name: &str) -> Option<String> {
    let patterns = [
        format!("async def {}(", function_name),
        format!("def {}(", function_name),
    ];

    for pattern in &patterns {
        if let Some(start_pos) = content.find(pattern) {
            let mut func_start = start_pos;

            // Look backwards for decorators
            let lines: Vec<&str> = content[..start_pos].lines().collect();
            for line in lines.iter().rev() {
                let trimmed = line.trim();
                if trimmed.starts_with('@') || trimmed.starts_with('#') || trimmed.is_empty() {
                    if let Some(pos) = content[..func_start].rfind(trimmed) {
                        func_start = pos;
                    }
                } else {
                    break;
                }
            }

            // Find end by tracking indentation
            let lines_after: Vec<&str> = content[start_pos..].lines().collect();
            if let Some(first_line) = lines_after.first() {
                let base_indent = first_line.len() - first_line.trim_start().len();
                let mut func_end = start_pos;
                let mut found_body = false;

                for line in &lines_after[1..] {
                    if line.trim().is_empty() {
                        func_end += line.len() + 1;
                        continue;
                    }

                    let line_indent = line.len() - line.trim_start().len();
                    if found_body && line_indent <= base_indent && !line.trim().is_empty() {
                        break;
                    }

                    found_body = true;
                    func_end += line.len() + 1;
                }

                return Some(content[func_start..func_end].to_string());
            }
        }
    }

    None
}

/// Extract JavaScript/TypeScript function
fn extract_javascript_function(content: &str, function_name: &str) -> Option<String> {
    let patterns = [
        format!("async function {}(", function_name),
        format!("function {}(", function_name),
        format!("const {} = (", function_name),
        format!("let {} = (", function_name),
        format!("var {} = (", function_name),
        format!("const {} = async (", function_name),
        format!("export function {}(", function_name),
        format!("export async function {}(", function_name),
        format!("{}(", function_name), // method definition
    ];

    for pattern in &patterns {
        if let Some(start_pos) = content.find(pattern) {
            if let Some(brace_start) = content[start_pos..].find('{') {
                let func_end = find_matching_brace(content, start_pos + brace_start)?;
                return Some(content[start_pos..func_end].to_string());
            }
        }
    }

    None
}

/// Find the matching closing brace for an opening brace
fn find_matching_brace(content: &str, open_brace_pos: usize) -> Option<usize> {
    let mut brace_count = 0;
    let mut in_string = false;
    let in_char = false;
    let mut escape_next = false;
    let mut string_char = '"';

    for (i, ch) in content[open_brace_pos..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => escape_next = true,
            '"' | '\'' if !in_char && !in_string => {
                in_string = true;
                string_char = ch;
            }
            c if in_string && c == string_char => in_string = false,
            '{' if !in_string && !in_char => brace_count += 1,
            '}' if !in_string && !in_char => {
                brace_count -= 1;
                if brace_count == 0 {
                    return Some(open_brace_pos + i + 1);
                }
            }
            _ => {}
        }
    }

    None
}
