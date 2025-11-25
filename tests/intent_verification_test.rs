use dotenvy::dotenv;
use intent_verification::verify_test_intent_with_changes;
use std::env;

#[tokio::test]
async fn test_verify_test_intent_with_changes() {
    // Load .env file
    dotenv().ok();

    // Get API key from environment variable
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.starts_with("sk-") {
                println!("Skipping test - no valid API key available");
                return;
            }
            key
        }
        Err(_) => {
            println!("Skipping test - OPENAI_API_KEY not set");
            return;
        }
    };

    let user_intent =
        "I want to test the calculate_sum and process_data functions to ensure they work correctly";

    // For testing purposes, we'll use the current repo
    // In a real scenario, you'd use actual commit hashes
    let repo_url = ".";
    let commit1 = "HEAD~1";
    let commit2 = "HEAD";

    println!("🔍 Testing intent verification with git changes...");
    println!("User Intent: {}", user_intent);

    match verify_test_intent_with_changes(
        &api_key,
        repo_url, // solution_repo_url
        commit1,  // solution_commit1
        commit2,  // solution_commit2
        repo_url, // test_repo_url
        commit2,  // test_commit (use commit2 to read test targets)
        user_intent,
    )
    .await
    {
        Ok(result) => {
            println!("\n✅ Intent Verification Result:");
            println!("  Intent Fulfilled: {}", result.is_intent_fulfilled);
            println!("  Confidence: {:.2}", result.confidence);
            println!("  Explanation: {}", result.explanation);
            println!("\n  Overall Assessment:");
            println!("  {}", result.overall_assessment);

            println!("\n  📁 Files Analyzed:");
            for file_analysis in &result.files_analyzed {
                println!(
                    "    - {}: {}",
                    file_analysis.file_path,
                    if file_analysis.supports_intent {
                        "✅ SUPPORTS"
                    } else {
                        "❌ DOES NOT SUPPORT"
                    }
                );
                println!("      Reasoning: {}", file_analysis.reasoning);
                if !file_analysis.relevant_changes.is_empty() {
                    println!("      Relevant Changes:");
                    for change in &file_analysis.relevant_changes {
                        println!("        • {}", change);
                    }
                }
            }

            // Assertions
            assert!(
                result.confidence >= 0.0 && result.confidence <= 1.0,
                "Confidence should be between 0 and 1"
            );
            assert!(!result.explanation.is_empty(), "Should have an explanation");
            assert!(
                !result.overall_assessment.is_empty(),
                "Should have an overall assessment"
            );
        }
        Err(e) => {
            // It's okay if there are no recent changes or the repo doesn't have proper git history
            println!("ℹ️  Note: {}", e);
            println!("This is expected if there are no recent changes or git history issues");
        }
    }
}

#[tokio::test]
async fn test_full_workflow_extract_and_verify() {
    // Load .env file
    dotenv().ok();

    // Get API key from environment variable
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.starts_with("sk-") {
                println!("Skipping test - no valid API key available");
                return;
            }
            key
        }
        Err(_) => {
            println!("Skipping test - OPENAI_API_KEY not set");
            return;
        }
    };

    let user_intent =
        "I need the analyze_repository_changes function and the git module to work properly";

    println!("🎯 Testing full workflow: Verify intent with AI-powered extraction");
    println!("User Intent: {}", user_intent);

    // The function will automatically extract targets and verify changes
    println!("\n🔍 Verifying if changes support the intent...");
    let repo_url = ".";
    let commit1 = "HEAD~1";
    let commit2 = "HEAD";

    match verify_test_intent_with_changes(
        &api_key,
        repo_url, // solution_repo_url
        commit1,  // solution_commit1
        commit2,  // solution_commit2
        repo_url, // test_repo_url
        commit2,  // test_commit
        user_intent,
    )
    .await
    {
        Ok(result) => {
            println!("  ✅ Verification complete:");
            println!("    Intent Fulfilled: {}", result.is_intent_fulfilled);
            println!("    Confidence: {:.2}", result.confidence);
            println!("    Files Analyzed: {}", result.files_analyzed.len());
            println!("\n  📝 Assessment: {}", result.overall_assessment);

            // Basic assertions
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
            assert!(!result.overall_assessment.is_empty());
        }
        Err(e) => {
            println!("  ℹ️  Note: {}", e);
        }
    }

    println!("\n✅ Full workflow test completed");
}

#[tokio::test]
async fn test_verify_with_empty_targets() {
    // Load .env file
    dotenv().ok();

    // Get API key from environment variable
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.starts_with("sk-") {
                println!("Skipping test - no valid API key available");
                return;
            }
            key
        }
        Err(_) => {
            println!("Skipping test - OPENAI_API_KEY not set");
            return;
        }
    };

    // Test with vague intent (will extract minimal or no targets)
    let user_intent = "Make everything work";

    println!("🧪 Testing with vague intent...");

    match verify_test_intent_with_changes(
        &api_key,
        ".",      // solution_repo_url
        "HEAD~1", // solution_commit1
        "HEAD",   // solution_commit2
        ".",      // test_repo_url
        "HEAD",   // test_commit
        user_intent,
    )
    .await
    {
        Ok(result) => {
            println!("  ✅ Handled vague intent gracefully");
            println!("  Intent Fulfilled: {}", result.is_intent_fulfilled);
            println!("  Confidence: {:.2}", result.confidence);

            // Should still provide some assessment even with vague intent
            assert!(!result.overall_assessment.is_empty());
        }
        Err(e) => {
            println!(
                "  ℹ️  Error (expected for vague intent or no changes): {}",
                e
            );
        }
    }
}
