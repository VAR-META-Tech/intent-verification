// Git-related functionality
mod git;
pub use git::{ChangeType, FileChange, get_git_changed_files};

// OpenAI-related functionality
mod openai;
pub use openai::{
    CRepositoryAnalysisResult, CodeAnalysis, FileAnalysisResult, RepositoryAnalysisResult,
    analyze_file_change_with_ai, analyze_repository_changes, analyze_repository_changes_ffi,
    ask_openai, ask_openai_internal, free_analysis_result, free_str,
};
