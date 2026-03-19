use reqwest::multipart;
use std::time::{Duration, Instant};

fn model_supports_streaming(model: &str) -> bool {
    model != "whisper-1"
}

pub async fn transcribe_audio(
    client: &reqwest::Client,
    wav_bytes: Vec<u8>,
    api_key: &str,
    model: &str,
    language: Option<&str>,
) -> Result<String, String> {
    eprintln!("[FamVoice] Sending {} bytes to API", wav_bytes.len());

    let use_streaming = model_supports_streaming(model);

    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
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

    let t_request = Instant::now();
    let mut res = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(30))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("API Error: {}", err_text));
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
