use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
    },
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

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

/// C-compatible structure for FFI results
#[repr(C)]
#[derive(Debug)]
pub struct CRepositoryAnalysisResult {
    pub is_good: bool,
    pub total_files: i32,
    pub analyzed_files: i32,
    pub good_files: i32,
    pub files_with_issues: i32,
    pub files_json: *mut c_char, // JSON string with file details
}

impl CRepositoryAnalysisResult {
    pub fn new() -> Self {
        Self {
            is_good: false,
            total_files: 0,
            analyzed_files: 0,
            good_files: 0,
            files_with_issues: 0,
            files_json: std::ptr::null_mut(),
        }
    }
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

/// FFI: Call OpenAI from C/FFI
#[unsafe(no_mangle)]
pub extern "C" fn ask_openai(prompt: *const c_char, api_key: *const c_char) -> *mut c_char {
    let prompt_c_str = unsafe {
        if prompt.is_null() {
            return std::ptr::null_mut();
        }
        CStr::from_ptr(prompt)
    };

    let api_key_c_str = unsafe {
        if api_key.is_null() {
            return std::ptr::null_mut();
        }
        CStr::from_ptr(api_key)
    };

    let prompt_str = match prompt_c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let api_key_str = match api_key_c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(ask_openai_internal(prompt_str, api_key_str));

    match result {
        Ok(output) => CString::new(output).unwrap().into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// FFI: Free string allocated by ask_openai
#[unsafe(no_mangle)]
pub extern "C" fn free_str(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

/// FFI: Analyze repository changes between two commits
/// Returns detailed analysis result as C-compatible structure
#[unsafe(no_mangle)]
pub extern "C" fn analyze_repository_changes_ffi(
    api_key: *const c_char,
    repo_url: *const c_char,
    commit1: *const c_char,
    commit2: *const c_char,
) -> *mut CRepositoryAnalysisResult {
    // Validate inputs
    if api_key.is_null() || repo_url.is_null() || commit1.is_null() || commit2.is_null() {
        return std::ptr::null_mut();
    }

    let api_key_str = unsafe {
        match CStr::from_ptr(api_key).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let repo_url_str = unsafe {
        match CStr::from_ptr(repo_url).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let commit1_str = unsafe {
        match CStr::from_ptr(commit1).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let commit2_str = unsafe {
        match CStr::from_ptr(commit2).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    // Run the async function in a blocking context
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(analyze_repository_changes(
            api_key_str,
            repo_url_str,
            commit1_str,
            commit2_str,
        ));

    match result {
        Ok(analysis_result) => {
            // Convert files to JSON for detailed information
            let files_json = match serde_json::to_string(&analysis_result.files) {
                Ok(json) => match CString::new(json) {
                    Ok(cstring) => cstring.into_raw(),
                    Err(_) => std::ptr::null_mut(),
                },
                Err(_) => std::ptr::null_mut(),
            };

            let c_result = Box::new(CRepositoryAnalysisResult {
                is_good: analysis_result.is_good,
                total_files: analysis_result.total_files,
                analyzed_files: analysis_result.analyzed_files,
                good_files: analysis_result.good_files,
                files_with_issues: analysis_result.files_with_issues,
                files_json,
            });

            Box::into_raw(c_result)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// FFI: Free CRepositoryAnalysisResult allocated by analyze_repository_changes_ffi
#[unsafe(no_mangle)]
pub extern "C" fn free_analysis_result(ptr: *mut CRepositoryAnalysisResult) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        let result = Box::from_raw(ptr);

        // Free the JSON string if it was allocated
        if !result.files_json.is_null() {
            drop(CString::from_raw(result.files_json));
        }

        // Box will be automatically dropped here
    }
}

pub async fn analyze_file_change_with_ai(
    file_change: &FileChange,
    api_key: &str,
) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    let content = match &file_change.content {
        Some(c) => c,
        None => {
            return Ok(CodeAnalysis {
                is_good: true, // Not bad, just no content
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
        if let Some(end) = response.rfind('}') {
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
