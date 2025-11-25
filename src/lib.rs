// Git-related functionality
mod git;
pub use git::{ChangeType, FileChange, get_git_changed_files, read_test_targets_code};

// Type definitions
mod types;
pub use types::{
    CodeAnalysis, FileAnalysisResult, FileContent, FileIntentAnalysis, FunctionContent,
    IntentVerificationResult, RepositoryAnalysisResult, TestTargets, TestTargetsWithCode,
};

// Utility functions
mod utils;
pub use utils::{combine_multiple_analyses, extract_json_from_response, parse_analysis_response};

// Code parsing utilities
mod code_parser;
pub use code_parser::{extract_function_from_content_with_name, is_source_file_by_name};

// OpenAI-related functionality
mod openai;
pub use openai::{
    analyze_file_change_with_ai, analyze_repository_changes, ask_openai_internal,
    extract_test_targets_with_ai, verify_test_intent_with_changes,
};

// FFI-related functionality
mod ffi;
pub use ffi::{
    CRepositoryAnalysisResult, analyze_repository_changes_ffi, ask_openai, free_analysis_result,
    free_str,
};
