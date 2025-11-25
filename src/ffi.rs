use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::openai::{analyze_repository_changes, ask_openai_internal};

/// C-compatible structure for FFI results
#[repr(C)]
#[derive(Debug)]
pub struct CRepositoryAnalysisResult {
    pub is_good: bool,
    pub total_files: i32,
    pub analyzed_files: i32,
    pub good_files: i32,
    pub files_with_issues: i32,
    pub files_json: *mut c_char, // JSON string with file details
}

impl CRepositoryAnalysisResult {
    pub fn new() -> Self {
        Self {
            is_good: false,
            total_files: 0,
            analyzed_files: 0,
            good_files: 0,
            files_with_issues: 0,
            files_json: std::ptr::null_mut(),
        }
    }
}

/// FFI: Call OpenAI from C/FFI
#[unsafe(no_mangle)]
pub extern "C" fn ask_openai(prompt: *const c_char, api_key: *const c_char) -> *mut c_char {
    let prompt_c_str = unsafe {
        if prompt.is_null() {
            return std::ptr::null_mut();
        }
        CStr::from_ptr(prompt)
    };

    let api_key_c_str = unsafe {
        if api_key.is_null() {
            return std::ptr::null_mut();
        }
        CStr::from_ptr(api_key)
    };

    let prompt_str = match prompt_c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let api_key_str = match api_key_c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(ask_openai_internal(prompt_str, api_key_str));

    match result {
        Ok(output) => CString::new(output).unwrap().into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// FFI: Free string allocated by ask_openai
#[unsafe(no_mangle)]
pub extern "C" fn free_str(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

/// FFI: Analyze repository changes between two commits
/// Returns detailed analysis result as C-compatible structure
#[unsafe(no_mangle)]
pub extern "C" fn analyze_repository_changes_ffi(
    api_key: *const c_char,
    repo_url: *const c_char,
    commit1: *const c_char,
    commit2: *const c_char,
) -> *mut CRepositoryAnalysisResult {
    // Validate inputs
    if api_key.is_null() || repo_url.is_null() || commit1.is_null() || commit2.is_null() {
        return std::ptr::null_mut();
    }

    let api_key_str = unsafe {
        match CStr::from_ptr(api_key).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let repo_url_str = unsafe {
        match CStr::from_ptr(repo_url).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let commit1_str = unsafe {
        match CStr::from_ptr(commit1).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let commit2_str = unsafe {
        match CStr::from_ptr(commit2).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    // Run the async function in a blocking context
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(analyze_repository_changes(
            api_key_str,
            repo_url_str,
            commit1_str,
            commit2_str,
        ));

    match result {
        Ok(analysis_result) => {
            // Convert files to JSON for detailed information
            let files_json = match serde_json::to_string(&analysis_result.files) {
                Ok(json) => match CString::new(json) {
                    Ok(cstring) => cstring.into_raw(),
                    Err(_) => std::ptr::null_mut(),
                },
                Err(_) => std::ptr::null_mut(),
            };

            let c_result = Box::new(CRepositoryAnalysisResult {
                is_good: analysis_result.is_good,
                total_files: analysis_result.total_files,
                analyzed_files: analysis_result.analyzed_files,
                good_files: analysis_result.good_files,
                files_with_issues: analysis_result.files_with_issues,
                files_json,
            });

            Box::into_raw(c_result)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// FFI: Free CRepositoryAnalysisResult allocated by analyze_repository_changes_ffi
#[unsafe(no_mangle)]
pub extern "C" fn free_analysis_result(ptr: *mut CRepositoryAnalysisResult) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        let result = Box::from_raw(ptr);

        // Free the JSON string if it was allocated
        if !result.files_json.is_null() {
            drop(CString::from_raw(result.files_json));
        }

        // Box will be automatically dropped here
    }
}
