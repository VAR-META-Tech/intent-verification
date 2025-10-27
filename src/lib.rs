use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
    },
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Internal async OpenAI function
pub async fn ask_openai_internal(
    prompt: &str,
    api_key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = OpenAIConfig::new()
        .with_api_key(api_key);

    let client = Client::with_config(config);

    let messages = vec![ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
            name: None,
        },
    )];

    let request = CreateChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages,
        ..Default::default()
    };

    let response = client.chat().create(request).await?;
    let reply = response
        .choices
        .get(0)
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "No response.".to_string());

    Ok(reply)
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
