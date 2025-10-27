use dotenvy::dotenv;
use intent_verification::ask_openai_internal;
use std::env;

#[tokio::main]
async fn main() {
    // Load `.env` file
    dotenv().ok();

    println!("Testing Intent Verification Library");
    println!("===================================");

    // Get API key from environment variable or use a placeholder
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        println!("Warning: OPENAI_API_KEY environment variable not set.");
        println!("Using placeholder API key for testing (this will fail actual API calls).");
        "sk-placeholder-api-key-for-testing".to_string()
    });

    println!(
        "API Key: {} (first 10 chars)",
        &api_key[..std::cmp::min(10, api_key.len())]
    );

    // Simple test with one prompt
    let prompt = "Hello, how are you?";
    println!("\nTesting with prompt: {}", prompt);

    // Call the internal async function directly
    match ask_openai_internal(prompt, &api_key).await {
        Ok(result) => {
            println!("Result: {}", result);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }

    println!("\nTesting completed!");
}
