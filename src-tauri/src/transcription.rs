use reqwest::multipart;
use reqwest::StatusCode;
use std::time::{Duration, Instant};

fn model_supports_streaming(model: &str, provider: &str) -> bool {
    if provider == "groq" {
        return false;
    }
    model != "whisper-1"
}

fn api_endpoint(provider: &str) -> &'static str {
    match provider {
        "groq" => "https://api.groq.com/openai/v1/audio/transcriptions",
        _ => "https://api.openai.com/v1/audio/transcriptions",
    }
}

pub fn warmup_endpoint(provider: &str) -> &'static str {
    match provider {
        "groq" => "https://api.groq.com/openai/v1/models",
        _ => "https://api.openai.com/v1/models",
    }
}

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "groq" => "Groq",
        _ => "OpenAI",
    }
}

fn user_facing_api_error(status: StatusCode, provider: &str) -> String {
    let label = provider_label(provider);
    match status {
        StatusCode::UNAUTHORIZED => {
            format!("{label} authentication failed. Check the saved API key.")
        }
        StatusCode::TOO_MANY_REQUESTS => {
            format!("{label} rejected the request due to rate limits or quota. Try again later.")
        }
        StatusCode::BAD_REQUEST => {
            format!("{label} rejected the audio request. Verify the selected model and try again.")
        }
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => {
            format!("{label} is temporarily unavailable. Try again in a moment.")
        }
        _ => format!("{label} request failed with status {}.", status.as_u16()),
    }
}

pub async fn transcribe_audio(
    client: &reqwest::Client,
    audio_bytes: Vec<u8>,
    api_key: &str,
    model: &str,
    language: Option<&str>,
    provider: &str,
    mime_type: &str,
    file_name: &str,
) -> Result<String, String> {
    eprintln!(
        "[FamVoice] Sending {} bytes ({}) to {} API",
        audio_bytes.len(),
        mime_type,
        provider_label(provider)
    );

    let use_streaming = model_supports_streaming(model, provider);

    let file_part = multipart::Part::bytes(audio_bytes)
        .file_name(file_name.to_string())
        .mime_str(mime_type)
        .map_err(|e| e.to_string())?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("response_format", "text");

    if use_streaming {
        form = form.text("stream", "true");
    }

    if let Some(lang) = language {
        if lang != "auto" {
            form = form.text("language", lang.to_string());
        }
    }

    let endpoint = api_endpoint(provider);
    let request_timeout = match provider {
        "groq" => Duration::from_secs(10),
        _ => Duration::from_secs(30),
    };
    let t_request = Instant::now();
    let mut res = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(request_timeout)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let status = res.status();
        let _err_text = res.text().await.unwrap_or_default();
        #[cfg(debug_assertions)]
        eprintln!(
            "[FamVoice] {} API error {}: {}",
            provider_label(provider),
            status,
            _err_text
        );
        return Err(user_facing_api_error(status, provider));
    }

    if !use_streaming {
        let text = res.text().await.map_err(|e| e.to_string())?;
        return Ok(text.trim().to_string());
    }

    // Parse SSE stream for streaming-capable models
    let mut first_delta_logged = false;
    let mut full_text = String::new();
    let mut buffer = String::new();

    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
            buffer.drain(..newline_pos + 1);

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                if !full_text.is_empty() {
                    return Ok(full_text.trim().to_string());
                }
                continue;
            }

            let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            match event.get("type").and_then(|t| t.as_str()) {
                Some("transcript.text.done") => {
                    if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                        return Ok(text.trim().to_string());
                    }
                }
                Some("transcript.text.delta") => {
                    if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                        if !first_delta_logged {
                            eprintln!(
                                "[FamVoice] First streaming delta at {:.0}ms",
                                t_request.elapsed().as_secs_f64() * 1000.0
                            );
                            first_delta_logged = true;
                        }
                        full_text.push_str(delta);
                    }
                }
                _ => {}
            }
        }
    }

    if full_text.is_empty() {
        Err("No transcription received from streaming API".to_string())
    } else {
        Ok(full_text.trim().to_string())
    }
}
