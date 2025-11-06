use dotenvy::dotenv;
use intent_verification::{analyze_repository_changes_ffi, free_analysis_result};
use std::env;
use std::ffi::CString;

#[test]
fn test_analyze_repository_changes_ffi_invalid_params() {
    // Test with null pointers - should return null pointer
    let result = analyze_repository_changes_ffi(
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null(),
    );
    assert!(
        result.is_null(),
        "Should return null pointer for null parameters"
    );
}

#[test]
fn test_analyze_repository_changes_ffi_with_valid_api_key() {
    // Load .env file
    dotenv().ok();

    // Test with valid API key from environment
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            if !key.starts_with("sk-") {
                println!("Skipping FFI test - no valid API key available");
                return;
            }
            key
        }
        Err(_) => {
            println!("Skipping FFI test - OPENAI_API_KEY not set");
            return;
        }
    };

    let api_key_cstring = CString::new(api_key).unwrap();
    let repo_url = CString::new("https://github.com/arkhai-io/alkahest-rs").unwrap();
    let commit1 = CString::new("0879d7bc336977136c6aa1674ee52601286ff9b1").unwrap();
    let commit2 = CString::new("04d80bfe66a3ac62f2d33cdcfcca859c92808e10").unwrap();

    let result_ptr = analyze_repository_changes_ffi(
        api_key_cstring.as_ptr(),
        repo_url.as_ptr(),
        commit1.as_ptr(),
        commit2.as_ptr(),
    );

    assert!(
        !result_ptr.is_null(),
        "Should return valid pointer for successful analysis"
    );

    unsafe {
        let result = &*result_ptr;

        if result.is_good {
            println!("✅ FFI: All files look good!");
        } else {
            println!("⚠️ FFI: Some files need attention");
        }

        println!("📊 FFI Analysis Summary:");
        println!("  Total files: {}", result.total_files);
        println!("  Analyzed files: {}", result.analyzed_files);
        println!("  Good files: {}", result.good_files);
        println!("  Files with issues: {}", result.files_with_issues);

        // Convert C string to Rust string for printing
        if !result.files_json.is_null() {
            let json_cstr = std::ffi::CStr::from_ptr(result.files_json);
            if let Ok(json_str) = json_cstr.to_str() {
                println!("  Files JSON: {}", json_str);
            } else {
                println!("  Files JSON: [Invalid UTF-8]");
            }
        } else {
            println!("  Files JSON: [null]");
        }

        // Cleanup
        free_analysis_result(result_ptr);
    }
}
