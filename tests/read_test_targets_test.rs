use intent_verification::{TestTargets, read_test_targets_code};

#[test]
fn test_read_test_targets_code() {
    // Create test targets with known functions and files from this project
    let targets = TestTargets {
        functions: vec![
            "analyze_repository_changes".to_string(),
            "extract_test_targets_with_ai".to_string(),
        ],
        files: vec!["src/lib.rs".to_string(), "Cargo.toml".to_string()],
    };

    // Read the code content
    let result = read_test_targets_code(&targets, ".");

    match result {
        Ok(targets_with_code) => {
            println!("✅ Successfully read test targets code");

            // Verify file contents
            println!("\n📁 File Contents:");
            for file_content in &targets_with_code.file_contents {
                if let Some(ref err) = file_content.error {
                    println!("  ❌ {}: {}", file_content.path, err);
                } else {
                    println!(
                        "  ✅ {}: {} bytes",
                        file_content.path,
                        file_content.content.len()
                    );
                    assert!(
                        !file_content.content.is_empty(),
                        "File content should not be empty"
                    );
                }
            }

            // Verify function contents
            println!("\n🔧 Function Contents:");
            for func_content in &targets_with_code.function_contents {
                if let Some(ref content) = func_content.content {
                    println!(
                        "  ✅ {}: found in {:?} ({} bytes)",
                        func_content.name,
                        func_content.file_path,
                        content.len()
                    );
                    println!(
                        "    Content preview: {}",
                        &content[..content.len().min(100)].replace('\n', "\\n")
                    );
                    assert!(!content.is_empty(), "Function content should not be empty");

                    // Verify the function name appears in the content
                    assert!(
                        content.contains(&func_content.name),
                        "Function content should contain the function name"
                    );
                } else if let Some(ref err) = func_content.error {
                    println!("  ⚠️  {}: {}", func_content.name, err);
                }
            }

            // Verify we found at least some content
            let files_found = targets_with_code
                .file_contents
                .iter()
                .filter(|f| f.error.is_none())
                .count();
            let functions_found = targets_with_code
                .function_contents
                .iter()
                .filter(|f| f.content.is_some())
                .count();

            println!("\n📊 Summary:");
            println!("  Files found: {}/{}", files_found, targets.files.len());
            println!(
                "  Functions found: {}/{}",
                functions_found,
                targets.functions.len()
            );

            assert!(files_found > 0, "Should find at least one file");
            assert!(functions_found > 0, "Should find at least one function");
        }
        Err(e) => {
            panic!("Failed to read test targets code: {}", e);
        }
    }
}

#[test]
fn test_read_test_targets_with_nested_function() {
    // Test with a function that has nested braces
    let targets = TestTargets {
        functions: vec!["get_git_changed_files".to_string()],
        files: vec![],
    };

    let result = read_test_targets_code(&targets, "src");

    match result {
        Ok(targets_with_code) => {
            println!("✅ Successfully read nested function");

            for func_content in &targets_with_code.function_contents {
                if let Some(ref content) = func_content.content {
                    println!(
                        "  ✅ Found {} in {:?}",
                        func_content.name, func_content.file_path
                    );
                    println!("  Content length: {} bytes", content.len());

                    // Verify it contains the function name
                    assert!(content.contains("get_git_changed_files"));

                    // Verify it looks like a complete function (has opening and closing braces)
                    let open_braces = content.matches('{').count();
                    let close_braces = content.matches('}').count();
                    assert_eq!(open_braces, close_braces, "Braces should be balanced");
                }
            }
        }
        Err(e) => {
            panic!("Failed to read nested function: {}", e);
        }
    }
}

#[test]
fn test_read_nonexistent_targets() {
    // Test with non-existent files and functions
    let targets = TestTargets {
        functions: vec!["nonexistent_function_xyz".to_string()],
        files: vec!["nonexistent_file.rs".to_string()],
    };

    let result = read_test_targets_code(&targets, ".");

    match result {
        Ok(targets_with_code) => {
            println!("✅ Handled non-existent targets gracefully");

            // Verify errors are reported
            for file_content in &targets_with_code.file_contents {
                assert!(
                    file_content.error.is_some(),
                    "Should have error for non-existent file"
                );
                println!("  ✅ File error: {:?}", file_content.error);
            }

            for func_content in &targets_with_code.function_contents {
                assert!(
                    func_content.content.is_none(),
                    "Should not find non-existent function"
                );
                assert!(
                    func_content.error.is_some(),
                    "Should have error for non-existent function"
                );
                println!("  ✅ Function error: {:?}", func_content.error);
            }
        }
        Err(e) => {
            panic!(
                "Should handle non-existent targets gracefully, not error: {}",
                e
            );
        }
    }
}
