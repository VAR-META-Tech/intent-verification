use colored::*;
use dotenvy::dotenv;
use intent_verification::analyze_repository_changes;
use std::env;

#[tokio::test]
async fn test_analyze_repository_changes() {
    // Load .env file
    dotenv().ok();

    // Get API key from environment variable
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Warning: OPENAI_API_KEY not set, skipping repository analysis test");
            return;
        }
    };

    // Skip if no real API key
    if api_key.starts_with("sk-placeholder") {
        println!("Skipping repository analysis test - no real API key available");
        return;
    }

    println!("Testing complete repository analysis");

    let repo_url = "https://github.com/arkhai-io/alkahest-rs";
    let commit1 = "0879d7bc336977136c6aa1674ee52601286ff9b1";
    let commit2 = "04d80bfe66a3ac62f2d33cdcfcca859c92808e10";

    match analyze_repository_changes(&api_key, repo_url, commit1, commit2).await {
        Ok(result) => {
            println!("\n📊 Repository Analysis Result:");

            let overall_status = if result.is_good {
                "✅ ALL FILES LOOK GOOD".bright_green()
            } else {
                "⚠️  SOME FILES NEED ATTENTION".bright_red()
            };
            println!("  Overall Status: {}", overall_status);
            println!("  Total files changed: {}", result.total_files);
            println!("  Files analyzed: {}", result.analyzed_files);
            println!(
                "  Files with good quality: {}",
                result.good_files.to_string().bright_green()
            );
            println!(
                "  Files needing improvement: {}",
                result.files_with_issues.to_string().bright_red()
            );

            println!("\n📁 Individual File Results:");
            for file_result in &result.files {
                match &file_result.analysis {
                    Some(analysis) => {
                        let status_color = if analysis.is_good {
                            "GOOD".bright_green()
                        } else {
                            "NEEDS IMPROVEMENT".bright_red()
                        };

                        println!(
                            "  {} - {} (confidence: {:.2})",
                            file_result.file_path.bright_cyan(),
                            status_color,
                            analysis.confidence
                        );

                        if !analysis.description.is_empty() {
                            // Print first line of description only for brevity
                            let first_line = analysis.description.lines().next().unwrap_or("");
                            println!("    💬 {}", first_line);
                        }
                    }
                    None => {
                        let reason = file_result
                            .error
                            .as_ref()
                            .map(|e| format!("Error: {}", e))
                            .unwrap_or_else(|| format!("Skipped ({:?})", file_result.change_type));

                        println!(
                            "  {} - {}",
                            file_result.file_path.bright_cyan(),
                            reason.dimmed()
                        );
                    }
                }
            }

            // Assertions
            assert!(result.files.len() > 0, "Should have found changed files");

            println!("\n✅ Repository analysis completed successfully!");
        }
        Err(e) => {
            panic!("Repository analysis failed: {}", e);
        }
    }
}
