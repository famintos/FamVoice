use reqwest::multipart;
use std::time::Duration;

pub async fn transcribe_audio(
    client: &reqwest::Client,
    wav_bytes: Vec<u8>,
    api_key: &str,
    model: &str,
    language: Option<&str>,
) -> Result<String, String> {
    eprintln!("[FamVoice] Sending {} bytes to API", wav_bytes.len());

    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("response_format", "text");

    if let Some(lang) = language {
        if lang != "auto" {
            form = form.text("language", lang.to_string());
        }
    }

    let res = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(30))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let text = res.text().await.map_err(|e| e.to_string())?;
        Ok(text.trim().to_string())
    } else {
        let err_text = res.text().await.unwrap_or_default();
        Err(format!("API Error: {}", err_text))
    }
}
