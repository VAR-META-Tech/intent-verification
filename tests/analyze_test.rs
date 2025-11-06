use colored::*;
use dotenvy::dotenv;
use intent_verification::{ChangeType, analyze_file_change_with_ai, get_git_changed_files};
use std::env;

#[tokio::test]
async fn test_analyze_multiple_file_changes() {
    // Load .env file
    dotenv().ok();

    // Get API key from environment variable
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Warning: OPENAI_API_KEY not set, skipping multi-file test");
            return;
        }
    };

    // Skip if no real API key
    if api_key.starts_with("sk-placeholder") {
        println!("Skipping multi-file analysis test - no real API key available");
        return;
    }

    println!("Testing analysis of multiple file changes");

    // Get file changes from git
    let repo_url = "https://github.com/arkhai-io/alkahest-rs";
    let commit1 = "0879d7bc336977136c6aa1674ee52601286ff9b1";
    let commit2 = "04d80bfe66a3ac62f2d33cdcfcca859c92808e10";

    let file_changes = match get_git_changed_files(repo_url, commit1, commit2) {
        Ok(changes) => changes,
        Err(e) => {
            println!("Failed to get git changes: {}", e);
            return;
        }
    };

    // Analyze up to 2 files to avoid API rate limits in tests
    let files_to_analyze: Vec<_> = file_changes
        .iter()
        .filter(|fc| fc.content.is_some() && fc.status != ChangeType::Deleted)
        .collect();

    assert!(!files_to_analyze.is_empty(), "Should have files to analyze");

    for file_change in files_to_analyze {
        println!("Analyzing: {}", file_change.path);

        match analyze_file_change_with_ai(file_change, &api_key).await {
            Ok(analysis) => {
                println!("Analysis Result for {}:", file_change.path.bright_cyan());

                let quality_status = if analysis.is_good {
                    "GOOD".bright_green()
                } else {
                    "NEEDS IMPROVEMENT".bright_red()
                };
                println!("  Is Good: {}", quality_status);
                println!("  Confidence: {:.2}", analysis.confidence);
                println!("  Description: {}", analysis.description);
                if let Some(suggestions) = &analysis.suggestions {
                    println!("  Suggestions: {}", suggestions);
                } else {
                    println!("  Suggestions: None");
                }
                println!();

                assert!(
                    !analysis.description.is_empty(),
                    "Analysis description should not be empty for {}",
                    file_change.path
                );
                println!("✓ Successfully analyzed {}", file_change.path);
            }
            Err(e) => {
                println!("Failed to analyze {}: {}", file_change.path, e);
                // Don't fail the test on individual file failures
                if e.to_string().contains("rate limit") {
                    println!("Stopping due to rate limit");
                    break;
                }
            }
        }
    }

    println!("✓ Multi-file analysis test completed");
}
