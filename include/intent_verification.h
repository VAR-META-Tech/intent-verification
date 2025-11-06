#ifndef INTENT_VERIFICATION_H
#define INTENT_VERIFICATION_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C"
{
#endif

    /**
     * C-compatible structure for repository analysis results
     */
    typedef struct
    {
        bool is_good;          // true if all files are good
        int total_files;       // total number of files changed
        int analyzed_files;    // number of files actually analyzed
        int good_files;        // number of files with good quality
        int files_with_issues; // number of files that need attention
        char *files_json;      // JSON string with detailed file information
    } CRepositoryAnalysisResult;

    /**
     * Analyze repository changes between two commits using AI
     *
     * @param api_key - OpenAI API key (null-terminated string)
     * @param repo_url - Git repository URL (null-terminated string)
     * @param commit1 - First commit hash, older (null-terminated string)
     * @param commit2 - Second commit hash, newer (null-terminated string)
     *
     * @return Pointer to CRepositoryAnalysisResult (must be freed with free_analysis_result)
     *         Returns NULL on error
     */
    CRepositoryAnalysisResult *analyze_repository_changes_ffi(
        const char *api_key,
        const char *repo_url,
        const char *commit1,
        const char *commit2);

    /**
     * Free CRepositoryAnalysisResult allocated by analyze_repository_changes_ffi
     *
     * @param ptr - Pointer to result structure to free
     */
    void free_analysis_result(CRepositoryAnalysisResult *ptr);

    /**
     * Ask OpenAI a question
     *
     * @param prompt - The prompt to send to OpenAI
     * @param api_key - OpenAI API key
     * @return Allocated string with response (must be freed with free_str)
     */
    char *ask_openai(const char *prompt, const char *api_key);

    /**
     * Free string allocated by ask_openai
     *
     * @param ptr - Pointer to string to free
     */
    void free_str(char *ptr);

#ifdef __cplusplus
}
#endif

#endif // INTENT_VERIFICATION_H