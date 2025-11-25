// Git-related functionality
mod git;
pub use git::{ChangeType, FileChange, get_git_changed_files};

// OpenAI-related functionality
mod openai;
pub use openai::{
    CodeAnalysis, FileAnalysisResult, FileContent, FunctionContent, RepositoryAnalysisResult,
    TestTargets, TestTargetsWithCode, analyze_file_change_with_ai, analyze_repository_changes,
    ask_openai_internal, extract_test_targets_with_ai, read_test_targets_code,
};

// FFI-related functionality
mod ffi;
pub use ffi::{
    CRepositoryAnalysisResult, analyze_repository_changes_ffi, ask_openai, free_analysis_result,
    free_str,
};
