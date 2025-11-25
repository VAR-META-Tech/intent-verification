use crate::ChangeType;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeAnalysis {
    pub is_good: bool,
    pub description: String,
    pub suggestions: Option<String>,
    pub confidence: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub change_type: ChangeType,
    pub analysis: Option<CodeAnalysis>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepositoryAnalysisResult {
    pub files: Vec<FileAnalysisResult>,
    pub is_good: bool,
    pub total_files: i32,
    pub analyzed_files: i32,
    pub good_files: i32,
    pub files_with_issues: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestTargets {
    pub functions: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestTargetsWithCode {
    pub targets: TestTargets,
    pub file_contents: Vec<FileContent>,
    pub function_contents: Vec<FunctionContent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionContent {
    pub name: String,
    pub file_path: Option<String>,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntentVerificationResult {
    pub is_intent_fulfilled: bool,
    pub confidence: f32,
    pub explanation: String,
    pub files_analyzed: Vec<FileIntentAnalysis>,
    pub overall_assessment: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileIntentAnalysis {
    pub file_path: String,
    pub change_type: ChangeType,
    pub supports_intent: bool,
    pub reasoning: String,
    pub relevant_changes: Vec<String>,
}
