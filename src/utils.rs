use crate::types::CodeAnalysis;

/// Parse an OpenAI response into a CodeAnalysis structure
///
/// Attempts to extract and parse JSON from the response. Falls back to plain text parsing if JSON is invalid.
pub fn parse_analysis_response(response: &str) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    // Try to extract JSON from the response
    let json_str = extract_json_from_response(response);

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(json) => {
            let is_good = json["is_good"].as_bool().unwrap_or(false);
            let description = json["description"]
                .as_str()
                .unwrap_or("No description provided")
                .to_string();
            let suggestions = json["suggestions"].as_str().map(|s| s.to_string());
            let confidence = json["confidence"].as_f64().unwrap_or(0.5) as f32;

            Ok(CodeAnalysis {
                is_good,
                description,
                suggestions,
                confidence,
            })
        }
        Err(_) => {
            // Fallback: create analysis from plain text response
            Ok(CodeAnalysis {
                is_good: !response.to_lowercase().contains("error")
                    && !response.to_lowercase().contains("issue")
                    && !response.to_lowercase().contains("problem"),
                description: response.to_string(),
                suggestions: None,
                confidence: 0.3, // Low confidence for non-structured response
            })
        }
    }
}

/// Extract JSON from an OpenAI response
///
/// Looks for JSON block between braces. Returns the original response if no JSON is found.
pub fn extract_json_from_response(response: &str) -> String {
    // Look for JSON block between ```json and ``` or just find { ... }
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                return response[start..=end].to_string();
            }
        }
    }

    // If no JSON found, return the original response
    response.to_string()
}

/// Combine multiple code analysis results into a single result
///
/// Used when analyzing large code blocks that were split into multiple parts.
pub fn combine_multiple_analyses(
    analyses: &[String],
) -> Result<CodeAnalysis, Box<dyn std::error::Error>> {
    let parsed_analyses: Vec<CodeAnalysis> = analyses
        .iter()
        .map(|a| parse_analysis_response(a))
        .collect::<Result<Vec<_>, _>>()?;

    let overall_good = parsed_analyses.iter().all(|a| a.is_good);
    let avg_confidence =
        parsed_analyses.iter().map(|a| a.confidence).sum::<f32>() / parsed_analyses.len() as f32;

    let combined_description = parsed_analyses
        .iter()
        .enumerate()
        .map(|(i, a)| format!("Block {}: {}", i + 1, a.description))
        .collect::<Vec<_>>()
        .join("\n\n");

    let combined_suggestions = parsed_analyses
        .iter()
        .filter_map(|a| a.suggestions.as_ref())
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    Ok(CodeAnalysis {
        is_good: overall_good,
        description: combined_description,
        suggestions: if combined_suggestions.is_empty() {
            None
        } else {
            Some(combined_suggestions.join("\n"))
        },
        confidence: avg_confidence,
    })
}
