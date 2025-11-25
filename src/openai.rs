use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequest,
    },
};

use crate::git::{read_test_targets_code, split_by_function};
use crate::types::{
    FileIntentAnalysis, IntentVerificationResult, TestTargets, TestTargetsWithCode,
};
use crate::utils::extract_json_from_response;
use crate::{ChangeType, FileChange};

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

/// Analyze git changes to verify if they fulfill the intended test requirements
///
/// # Arguments
/// * `api_key` - OpenAI API key
/// * `test_repo_url` - Git repository URL to read test targets from
/// * `test_commit` - Commit hash to read test target code from
/// * `solution_repo_url` - Git repository URL for the solution/changes
/// * `solution_commit1` - First commit hash (before changes)
/// * `solution_commit2` - Second commit hash (after changes)
/// * `user_intent` - Original user prompt describing what should work
///
/// # Returns
/// * `IntentVerificationResult` - Analysis of whether changes fulfill the intent
pub async fn verify_test_intent_with_changes(
    api_key: &str,
    test_repo_url: &str,
    test_commit: &str,
    solution_repo_url: &str,
    solution_commit1: &str,
    solution_commit2: &str,
    user_intent: &str,
) -> Result<IntentVerificationResult, Box<dyn std::error::Error>> {
    // First, extract test targets from the user intent using AI
    let test_targets = extract_test_targets_with_ai(user_intent, api_key).await?;

    // Then, read the actual code of the test targets from the repository at the specified commit
    let targets_with_code = read_test_targets_code(&test_targets, test_repo_url, test_commit)?;

    // Get changed files from git
    let file_changes =
        crate::git::get_git_changed_files(solution_repo_url, solution_commit1, solution_commit2)?;

    println!(
        "📝 Found {} changed files between commits {} and {}",
        file_changes.len(),
        solution_commit1,
        solution_commit2
    );
    for (i, fc) in file_changes.iter().enumerate() {
        println!("  {}. {} [{:?}]", i + 1, fc.path, fc.status);
    }

    let mut file_analyses = Vec::new();
    let mut total_supporting = 0;

    // Analyze each changed file in context of the test intent
    for file_change in &file_changes {
        if file_change.status == ChangeType::Deleted {
            // Deleted files generally don't support making tests pass
            file_analyses.push(FileIntentAnalysis {
                file_path: file_change.path.clone(),
                change_type: file_change.status.clone(),
                supports_intent: false,
                reasoning: "File was deleted, which typically doesn't help tests pass".to_string(),
                relevant_changes: vec![],
            });
            continue;
        }

        // Analyze if this file change supports the test intent
        match analyze_file_for_test_intent(file_change, &targets_with_code, user_intent, api_key)
            .await
        {
            Ok(analysis) => {
                if analysis.supports_intent {
                    total_supporting += 1;
                }
                file_analyses.push(analysis);
            }
            Err(e) => {
                file_analyses.push(FileIntentAnalysis {
                    file_path: file_change.path.clone(),
                    change_type: file_change.status.clone(),
                    supports_intent: false,
                    reasoning: format!("Error analyzing file: {}", e),
                    relevant_changes: vec![],
                });
            }
        }
    }

    // Generate overall assessment using AI
    let overall_assessment = generate_overall_intent_assessment(
        &file_analyses,
        &targets_with_code,
        user_intent,
        api_key,
    )
    .await?;

    // Calculate confidence based on number of supporting files and AI assessment
    let support_ratio = if !file_analyses.is_empty() {
        total_supporting as f32 / file_analyses.len() as f32
    } else {
        0.0
    };

    let is_intent_fulfilled = total_supporting > 0 && support_ratio >= 0.5;
    let confidence = (support_ratio * 0.7 + 0.3).min(1.0); // Base confidence on support ratio

    Ok(IntentVerificationResult {
        is_intent_fulfilled,
        confidence,
        explanation: format!(
            "{} out of {} changed files support the test intent",
            total_supporting,
            file_analyses.len()
        ),
        files_analyzed: file_analyses,
        overall_assessment,
    })
}

/// Analyze a single file change to determine if it supports the test intent
async fn analyze_file_for_test_intent(
    file_change: &FileChange,
    targets_with_code: &TestTargetsWithCode,
    user_intent: &str,
    api_key: &str,
) -> Result<FileIntentAnalysis, Box<dyn std::error::Error>> {
    let content = match &file_change.content {
        Some(c) => c,
        None => {
            return Ok(FileIntentAnalysis {
                file_path: file_change.path.clone(),
                change_type: file_change.status.clone(),
                supports_intent: false,
                reasoning: "No content available to analyze".to_string(),
                relevant_changes: vec![],
            });
        }
    };

    if content == "[Binary file]" || content == "[Non-UTF8 content]" {
        return Ok(FileIntentAnalysis {
            file_path: file_change.path.clone(),
            change_type: file_change.status.clone(),
            supports_intent: false,
            reasoning: "Binary or non-UTF8 file, cannot analyze for test intent".to_string(),
            relevant_changes: vec![],
        });
    }

    // Split content into blocks if too large
    let blocks = if content.len() > 12_000 {
        split_by_function(content)
    } else {
        vec![content.clone()]
    };

    println!(
        "\n📄 Analyzing file {} ({} blocks)",
        file_change.path,
        blocks.len()
    );

    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);

    let mut all_supports_intent = Vec::new();
    let mut all_reasoning = Vec::new();
    let mut all_relevant_changes = Vec::new();

    // Analyze each block
    for (i, block) in blocks.iter().enumerate() {
        let mut messages = vec![intent_verification_system_rules()];
        messages.extend(add_test_target_context(targets_with_code));
        messages.push(add_file_change_context_for_block(
            file_change,
            user_intent,
            block,
            i + 1,
            blocks.len(),
        ));

        let request = CreateChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages,
            ..Default::default()
        };

        let response = client.chat().create(request).await?;
        let response_text = response
            .choices
            .get(0)
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response.".to_string());

        println!("\n🤖 OPENAI RESPONSE for block {}:", i + 1);
        println!("{}", response_text);
        println!("---");

        let json_str = extract_json_from_response(&response_text);

        match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(json) => {
                let supports_intent = json["supports_intent"].as_bool().unwrap_or(false);
                let reasoning = json["reasoning"]
                    .as_str()
                    .unwrap_or("No reasoning provided")
                    .to_string();
                let relevant_changes: Vec<String> = json["relevant_changes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                all_supports_intent.push(supports_intent);
                all_reasoning.push(reasoning);
                all_relevant_changes.extend(relevant_changes);
            }
            Err(_) => {
                // Fallback parsing
                let supports_intent = response_text.to_lowercase().contains("true")
                    || response_text.to_lowercase().contains("yes")
                    || response_text.to_lowercase().contains("supports");

                all_supports_intent.push(supports_intent);
                all_reasoning.push(response_text);
            }
        }
    }

    // Combine results from all blocks
    let final_supports_intent = all_supports_intent.iter().any(|&x| x);
    let final_reasoning = if blocks.len() > 1 {
        format!(
            "Analysis of {} blocks:\n{}",
            blocks.len(),
            all_reasoning
                .iter()
                .enumerate()
                .map(|(i, r)| format!("Block {}: {}", i + 1, r))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        all_reasoning.first().cloned().unwrap_or_default()
    };

    Ok(FileIntentAnalysis {
        file_path: file_change.path.clone(),
        change_type: file_change.status.clone(),
        supports_intent: final_supports_intent,
        reasoning: final_reasoning,
        relevant_changes: all_relevant_changes,
    })
}

/// Generate an overall assessment of whether the changes fulfill the test intent
async fn generate_overall_intent_assessment(
    file_analyses: &[FileIntentAnalysis],
    targets_with_code: &TestTargetsWithCode,
    user_intent: &str,
    api_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Summarize file analyses
    let summary = file_analyses
        .iter()
        .map(|fa| {
            format!(
                "- {}: {} ({})",
                fa.file_path,
                if fa.supports_intent {
                    "SUPPORTS"
                } else {
                    "DOES NOT SUPPORT"
                },
                fa.reasoning
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Summarize what was found vs what was expected
    let found_functions = targets_with_code
        .function_contents
        .iter()
        .filter(|f| f.content.is_some())
        .count();
    let total_functions = targets_with_code.targets.functions.len();

    let found_files = targets_with_code
        .file_contents
        .iter()
        .filter(|f| f.error.is_none())
        .count();
    let total_files = targets_with_code.targets.files.len();

    let prompt = format!(
        r#"Provide a concise overall assessment of whether the code changes fulfill the test intent.

User Intent: "{}"
Target Functions: {} (found {}/{} in codebase)
Target Files: {} (found {}/{})

File Analysis Summary:
{}

Provide a 2-3 sentence assessment covering:
1. Whether the changes are likely to make the specified tests work
2. Key supporting or missing changes
3. Overall confidence in test success

Respond with just the assessment text (no JSON):"#,
        user_intent,
        targets_with_code.targets.functions.join(", "),
        found_functions,
        total_functions,
        targets_with_code.targets.files.join(", "),
        found_files,
        total_files,
        summary
    );

    let assessment = ask_openai_internal(&prompt, api_key).await?;
    Ok(assessment.trim().to_string())
}

/// Create system message for intent verification analysis
pub fn intent_verification_system_rules() -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage::System(
        "You are an AI specialized in code analysis for test intent verification.\n\
         - Analyze code changes in context of test requirements\n\
         - Determine if changes support making tests pass\n\
         - Identify relevant changes that fulfill user intent\n\
         - Return strict JSON with reasoning\n"
            .into(),
    )
}

/// Add test target context (functions and files that need to work)
pub fn add_test_target_context(
    targets_with_code: &TestTargetsWithCode,
) -> Vec<ChatCompletionRequestMessage> {
    let mut context = String::from("TEST TARGETS:\n\n");

    // Add function targets with their code
    if !targets_with_code.function_contents.is_empty() {
        context.push_str("Functions that need to work:\n");
        for func in &targets_with_code.function_contents {
            if let Some(ref code) = func.content {
                context.push_str(&format!(
                    "- Function '{}' in {}:\n```\n{}\n```\n\n",
                    func.name,
                    func.file_path.as_deref().unwrap_or("unknown"),
                    code
                ));
            } else {
                context.push_str(&format!(
                    "- Function '{}' (not found in codebase)\n",
                    func.name
                ));
            }
        }
    }

    // Add file targets
    if !targets_with_code.file_contents.is_empty() {
        context.push_str("\nFiles that need to work:\n");
        for file in &targets_with_code.file_contents {
            if file.error.is_none() {
                context.push_str(&format!(
                    "- File '{}': {} bytes\n",
                    file.path,
                    file.content.len()
                ));
            } else {
                context.push_str(&format!(
                    "- File '{}' (error: {})\n",
                    file.path,
                    file.error.as_deref().unwrap_or("unknown")
                ));
            }
        }
    }

    let messages = vec![
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(context.clone()),
            name: None,
        }),
        ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
            content: Some(
                "Acknowledged. I will analyze changes in context of these test targets.".into(),
            ),
            name: None,
            ..Default::default()
        }),
    ];

    println!("\n🎯 TEST TARGET CONTEXT:");
    println!("{}", context);
    println!("---");

    messages
}

/// Add file change context for a specific block (for large files split into multiple blocks)
pub fn add_file_change_context_for_block(
    file_change: &FileChange,
    user_intent: &str,
    block_content: &str,
    block_num: usize,
    total_blocks: usize,
) -> ChatCompletionRequestMessage {
    let block_info = if total_blocks > 1 {
        format!(" (Block {}/{})", block_num, total_blocks)
    } else {
        String::new()
    };

    let message_content = format!(
        "USER INTENT: \"{}\"\n\n\
         CHANGED FILE: {}{}\n\
         CHANGE TYPE: {:?}\n\n\
         CODE CHANGES:\n\
         ```\n{}\n```\n\n\
         Analyze if these changes support fulfilling the user intent and making the test targets work. \
         Respond in JSON format with: supports_intent (bool), reasoning (string), relevant_changes (array), confidence (float).",
        user_intent, file_change.path, block_info, file_change.status, block_content
    );

    println!(
        "\n📄 FILE CHANGE CONTEXT for {}{}:",
        file_change.path, block_info
    );
    if total_blocks > 1 {
        println!("  ⚠️  Large file split into {} blocks", total_blocks);
    }
    println!("---");

    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
        content: ChatCompletionRequestUserMessageContent::Text(message_content),
        name: None,
    })
}
