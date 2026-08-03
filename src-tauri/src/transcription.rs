use reqwest::multipart;
use reqwest::StatusCode;
use std::time::{Duration, Instant};

const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;
const EMPTY_TRANSCRIPTION_ERROR: &str = "The transcription returned no text. Try speaking again.";
const OPENAI_BASE_TIMEOUT_SECS: u64 = 30;
const GROQ_BASE_TIMEOUT_SECS: u64 = 20;
const MAX_REQUEST_TIMEOUT_SECS: u64 = 15 * 60;
const UPLOAD_BYTES_PER_TIMEOUT_SECOND: usize = 256 * 1024;

pub struct TranscriptionRequest<'a> {
    pub model: &'a str,
    pub language: Option<&'a str>,
    pub prompt: Option<&'a str>,
    pub keywords: &'a [String],
    pub provider: &'a str,
    pub mime_type: &'a str,
    pub file_name: &'a str,
    pub audio_duration: Duration,
}

pub fn validate_transcript_text(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Err(EMPTY_TRANSCRIPTION_ERROR.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LanguageField {
    Singular,
    Plural,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModelCapabilities {
    streaming: bool,
    language_field: LanguageField,
    keywords: bool,
    legacy_response_controls: bool,
}

const GPT_TRANSCRIBE_CAPABILITIES: ModelCapabilities = ModelCapabilities {
    streaming: true,
    language_field: LanguageField::Plural,
    keywords: true,
    legacy_response_controls: false,
};

const WHISPER_COMPATIBLE_CAPABILITIES: ModelCapabilities = ModelCapabilities {
    streaming: false,
    language_field: LanguageField::Singular,
    keywords: false,
    legacy_response_controls: true,
};

fn model_capabilities(provider: &str, model: &str) -> ModelCapabilities {
    match (provider, model) {
        // Groq's current Whisper models use the legacy-compatible multipart
        // fields and are intentionally non-streaming in FamVoice.
        ("groq", _) => WHISPER_COMPATIBLE_CAPABILITIES,
        ("openai", "gpt-transcribe") => GPT_TRANSCRIBE_CAPABILITIES,
        // whisper-1 and unknown/legacy OpenAI models take the conservative
        // compatibility path rather than receiving gpt-transcribe-only fields.
        _ => WHISPER_COMPATIBLE_CAPABILITIES,
    }
}

fn api_endpoint(provider: &str) -> &'static str {
    match provider {
        "groq" => "https://api.groq.com/openai/v1/audio/transcriptions",
        _ => "https://api.openai.com/v1/audio/transcriptions",
    }
}

fn scaled_request_timeout(
    provider: &str,
    audio_size_bytes: usize,
    audio_duration: Duration,
) -> Duration {
    let base_secs = match provider {
        "groq" => GROQ_BASE_TIMEOUT_SECS,
        _ => OPENAI_BASE_TIMEOUT_SECS,
    };
    let duration_budget_secs = (audio_duration.as_secs_f64() * 0.5).ceil() as u64;
    let upload_budget_secs = if audio_size_bytes == 0 {
        0
    } else {
        audio_size_bytes.saturating_add(UPLOAD_BYTES_PER_TIMEOUT_SECOND - 1)
            / UPLOAD_BYTES_PER_TIMEOUT_SECOND
    } as u64;

    Duration::from_secs(
        base_secs
            .saturating_add(duration_budget_secs)
            .saturating_add(upload_budget_secs)
            .min(MAX_REQUEST_TIMEOUT_SECS),
    )
}

struct RequestPolicy<'a> {
    endpoint: &'a str,
    request_timeout: Duration,
    retry_delay: Duration,
}

impl RequestPolicy<'_> {
    fn production(
        provider: &str,
        audio_size_bytes: usize,
        audio_duration: Duration,
    ) -> RequestPolicy<'static> {
        RequestPolicy {
            endpoint: api_endpoint(provider),
            request_timeout: scaled_request_timeout(provider, audio_size_bytes, audio_duration),
            retry_delay: Duration::from_millis(1500),
        }
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

/// Returns true if a reqwest error is a transient network-level failure worth retrying.
/// Does NOT consider HTTP status errors retryable (those are handled after a successful send).
fn is_transient_network_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

fn user_facing_network_error(err: &reqwest::Error, provider: &str) -> String {
    let label = provider_label(provider);
    if err.is_timeout() {
        format!("{label} transcription timed out. Check the connection and try again.")
    } else {
        format!("Could not reach {label}. Check the connection and try again.")
    }
}

fn ensure_response_size(current: usize, incoming: usize) -> Result<(), String> {
    if current.saturating_add(incoming) > MAX_API_RESPONSE_BYTES {
        Err("Transcription API response exceeded the size limit".to_string())
    } else {
        Ok(())
    }
}

async fn read_response_text_limited(mut response: reqwest::Response) -> Result<String, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_API_RESPONSE_BYTES as u64)
    {
        return Err("Transcription API response exceeded the size limit".to_string());
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        ensure_response_size(body.len(), chunk.len())?;
        body.extend_from_slice(&chunk);
    }

    String::from_utf8(body).map_err(|_| "Transcription API returned invalid UTF-8".to_string())
}

/// Build the multipart form for the transcription request. The form is consumed
/// by each send attempt, so we rebuild it for retries.
fn build_form(
    audio_bytes: &[u8],
    request: &TranscriptionRequest<'_>,
    capabilities: ModelCapabilities,
) -> Result<multipart::Form, String> {
    let file_part = multipart::Part::bytes(audio_bytes.to_vec())
        .file_name(request.file_name.to_string())
        .mime_str(request.mime_type)
        .map_err(|e| e.to_string())?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", request.model.to_string());

    if capabilities.legacy_response_controls {
        form = form
            .text("response_format", "text")
            .text("temperature", "0");
    }

    if capabilities.streaming {
        form = form.text("stream", "true");
    }

    if let Some(language) = request
        .language
        .map(str::trim)
        .filter(|language| !language.is_empty() && *language != "auto")
    {
        form = match capabilities.language_field {
            LanguageField::Singular => form.text("language", language.to_string()),
            LanguageField::Plural => form.text("languages[]", language.to_string()),
        };
    }

    if capabilities.keywords {
        for keyword in request
            .keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty())
        {
            form = form.text("keywords[]", keyword.to_string());
        }
    }

    if let Some(prompt) = request
        .prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
    {
        form = form.text("prompt", prompt.to_string());
    }

    Ok(form)
}

struct TranscriptSseParser {
    line_buffer: Vec<u8>,
    full_text: String,
    first_delta_logged: bool,
    request_started_at: Instant,
}

impl TranscriptSseParser {
    fn new(request_started_at: Instant) -> Self {
        Self {
            line_buffer: Vec::new(),
            full_text: String::new(),
            first_delta_logged: false,
            request_started_at,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Result<Option<String>, String> {
        self.line_buffer.extend_from_slice(bytes);

        while let Some(newline_pos) = self.line_buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.line_buffer.drain(..=newline_pos).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }

            if let Some(text) = self.process_line(&line)? {
                return Ok(Some(text));
            }
        }

        Ok(None)
    }

    fn finish(mut self) -> Result<String, String> {
        if !self.line_buffer.is_empty() {
            let mut line = std::mem::take(&mut self.line_buffer);
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if let Some(text) = self.process_line(&line)? {
                return Ok(text);
            }
        }

        validate_transcript_text(&self.full_text)
    }

    fn process_line(&mut self, line: &[u8]) -> Result<Option<String>, String> {
        let line = std::str::from_utf8(line)
            .map_err(|_| "Transcription API returned invalid UTF-8".to_string())?;
        let Some(data) = line.strip_prefix("data:").map(str::trim_start) else {
            return Ok(None);
        };

        if data == "[DONE]" {
            return validate_transcript_text(&self.full_text).map(Some);
        }

        let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
            return Ok(None);
        };

        match event.get("type").and_then(|event_type| event_type.as_str()) {
            Some("transcript.text.done") => {
                if let Some(text) = event.get("text").and_then(|text| text.as_str()) {
                    return validate_transcript_text(text).map(Some);
                }

                validate_transcript_text(&self.full_text).map(Some)
            }
            Some("transcript.text.delta") => {
                if let Some(delta) = event.get("delta").and_then(|delta| delta.as_str()) {
                    if !self.first_delta_logged {
                        eprintln!(
                            "[FamVoice] First streaming delta at {:.0}ms",
                            self.request_started_at.elapsed().as_secs_f64() * 1000.0
                        );
                        self.first_delta_logged = true;
                    }
                    self.full_text.push_str(delta);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

pub async fn transcribe_audio(
    client: &reqwest::Client,
    audio_bytes: Vec<u8>,
    api_key: &str,
    request: TranscriptionRequest<'_>,
) -> Result<String, String> {
    let policy =
        RequestPolicy::production(request.provider, audio_bytes.len(), request.audio_duration);
    transcribe_audio_with_policy(client, audio_bytes, api_key, request, &policy).await
}

async fn transcribe_audio_with_policy(
    client: &reqwest::Client,
    audio_bytes: Vec<u8>,
    api_key: &str,
    request: TranscriptionRequest<'_>,
    policy: &RequestPolicy<'_>,
) -> Result<String, String> {
    eprintln!(
        "[FamVoice] Sending {} bytes ({}) to {} API (timeout {}s)",
        audio_bytes.len(),
        request.mime_type,
        provider_label(request.provider),
        policy.request_timeout.as_secs()
    );

    let capabilities = model_capabilities(request.provider, request.model);
    let request_started_at = Instant::now();

    // Send with a single retry for transient network errors (connection failures, timeouts).
    // HTTP-level errors (4xx, 5xx) are NOT retried — only connection-level failures.
    let mut res = {
        let form = build_form(&audio_bytes, &request, capabilities)?;
        let result = client
            .post(policy.endpoint)
            .bearer_auth(api_key)
            .timeout(policy.request_timeout)
            .multipart(form)
            .send()
            .await;

        match result {
            Ok(response) => response,
            Err(err) if is_transient_network_error(&err) => {
                eprintln!(
                    "[FamVoice] Transient network error, retrying in {}ms: {}",
                    policy.retry_delay.as_millis(),
                    err
                );
                tokio::time::sleep(policy.retry_delay).await;

                let retry_form = build_form(&audio_bytes, &request, capabilities)?;
                client
                    .post(policy.endpoint)
                    .bearer_auth(api_key)
                    .timeout(policy.request_timeout)
                    .multipart(retry_form)
                    .send()
                    .await
                    .map_err(|error| user_facing_network_error(&error, request.provider))?
            }
            Err(err) => return Err(user_facing_network_error(&err, request.provider)),
        }
    };

    if !res.status().is_success() {
        let status = res.status();
        eprintln!(
            "[FamVoice] {} API error {}",
            provider_label(request.provider),
            status
        );
        return Err(user_facing_api_error(status, request.provider));
    }

    if !capabilities.streaming {
        let text = read_response_text_limited(res).await?;
        return validate_transcript_text(&text);
    }

    if res
        .content_length()
        .is_some_and(|length| length > MAX_API_RESPONSE_BYTES as u64)
    {
        return Err("Transcription API response exceeded the size limit".to_string());
    }

    let mut parser = TranscriptSseParser::new(request_started_at);
    let mut received_bytes = 0usize;

    while let Some(chunk) = res.chunk().await.map_err(|error| error.to_string())? {
        ensure_response_size(received_bytes, chunk.len())?;
        received_bytes += chunk.len();
        if let Some(text) = parser.push(&chunk)? {
            return Ok(text);
        }
    }

    parser.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;

    enum MockReply {
        Response {
            status: &'static str,
            content_type: &'static str,
            chunks: Vec<&'static [u8]>,
        },
        Disconnect,
        Stall(Duration),
    }

    struct MockHttpServer {
        endpoint: String,
        requests: Arc<Mutex<Vec<String>>>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl MockHttpServer {
        fn start(replies: Vec<MockReply>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let captured_requests = Arc::clone(&requests);
            let server_thread = thread::spawn(move || {
                for reply in replies {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_http_request(&mut stream);
                    captured_requests.lock().unwrap().push(request);

                    match reply {
                        MockReply::Response {
                            status,
                            content_type,
                            chunks,
                        } => write_http_response(&mut stream, status, content_type, &chunks),
                        MockReply::Disconnect => {}
                        MockReply::Stall(duration) => thread::sleep(duration),
                    }
                }
            });

            Self {
                endpoint: format!("http://{address}/audio/transcriptions"),
                requests,
                thread: Some(server_thread),
            }
        }

        fn finish(mut self) -> Vec<String> {
            self.thread.take().unwrap().join().unwrap();
            Arc::try_unwrap(self.requests)
                .unwrap()
                .into_inner()
                .unwrap()
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut expected_length = None;

        loop {
            let read = stream.read(&mut buffer).unwrap_or(0);
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);

            if expected_length.is_none() {
                if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let header_length = header_end + 4;
                    let headers = String::from_utf8_lossy(&bytes[..header_length]);
                    let content_length = headers.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    });
                    expected_length = Some(header_length + content_length.unwrap_or(0));
                }
            }

            if expected_length.is_some_and(|length| bytes.len() >= length) {
                break;
            }
        }

        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn write_http_response(
        stream: &mut TcpStream,
        status: &str,
        content_type: &str,
        chunks: &[&[u8]],
    ) {
        let content_length = chunks.iter().map(|chunk| chunk.len()).sum::<usize>();
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();

        for chunk in chunks {
            stream.write_all(chunk).unwrap();
            stream.flush().unwrap();
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn request<'a>(
        endpoint: &'a str,
        model: &'a str,
        provider: &'a str,
        keywords: &'a [String],
    ) -> (TranscriptionRequest<'a>, RequestPolicy<'a>) {
        (
            TranscriptionRequest {
                model,
                language: Some("pt"),
                prompt: Some("FamVoice"),
                keywords,
                provider,
                mime_type: "audio/wav",
                file_name: "test.wav",
                audio_duration: Duration::from_secs(12),
            },
            RequestPolicy {
                endpoint,
                request_timeout: Duration::from_secs(1),
                retry_delay: Duration::from_millis(1),
            },
        )
    }

    async fn transcribe_against(
        endpoint: &str,
        model: &str,
        provider: &str,
        keywords: &[String],
        request_timeout: Duration,
    ) -> Result<String, String> {
        let (request, mut policy) = request(endpoint, model, provider, keywords);
        policy.request_timeout = request_timeout;
        transcribe_audio_with_policy(
            &reqwest::Client::new(),
            vec![1, 2, 3, 4],
            "test-key",
            request,
            &policy,
        )
        .await
    }

    fn has_multipart_field(request: &str, name: &str) -> bool {
        request.contains(&format!("name=\"{name}\""))
    }

    fn has_multipart_value(request: &str, name: &str, value: &str) -> bool {
        request.contains(&format!("name=\"{name}\"\r\n\r\n{value}\r\n"))
    }

    #[test]
    fn response_size_limit_accepts_exact_boundary() {
        assert!(ensure_response_size(MAX_API_RESPONSE_BYTES - 1, 1).is_ok());
    }

    #[test]
    fn response_size_limit_rejects_oversized_or_overflowing_body() {
        assert!(ensure_response_size(MAX_API_RESPONSE_BYTES, 1).is_err());
        assert!(ensure_response_size(usize::MAX, 1).is_err());
    }

    #[test]
    fn timeout_scales_with_audio_duration_and_size_and_is_capped() {
        let short = scaled_request_timeout("openai", 64 * 1024, Duration::from_secs(5));
        let long = scaled_request_timeout("openai", 64 * 1024, Duration::from_secs(10 * 60));
        let large = scaled_request_timeout("openai", 10 * 1024 * 1024, Duration::from_secs(5));
        let extreme = scaled_request_timeout("openai", usize::MAX, Duration::MAX);

        assert!(long > short);
        assert!(large > short);
        assert_eq!(extreme, Duration::from_secs(MAX_REQUEST_TIMEOUT_SECS));
    }

    #[test]
    fn normal_response_rejects_empty_or_whitespace_only_text() {
        assert_eq!(
            validate_transcript_text("  \r\n\t  ").unwrap_err(),
            EMPTY_TRANSCRIPTION_ERROR
        );
        assert_eq!(validate_transcript_text("  hello  ").unwrap(), "hello");
    }

    #[test]
    fn fragmented_utf8_sse_parses_deltas_and_final_done_at_eof() {
        let mut parser = TranscriptSseParser::new(Instant::now());
        assert!(parser
            .push(b"data: {\"type\":\"transcript.text.delta\",\"delta\":\"Ol\xc3")
            .unwrap()
            .is_none());
        assert!(parser
            .push(b"\xa1 \"}\n\ndata: {\"type\":\"transcript.text.delta\",\"delta\":\"mundo\"}\n\n")
            .unwrap()
            .is_none());
        assert!(parser
            .push(b"data: {\"type\":\"transcript.text.done\",\"text\":\"Ol\xc3")
            .unwrap()
            .is_none());
        assert!(parser.push(b"\xa1 mundo\"}").unwrap().is_none());

        assert_eq!(parser.finish().unwrap(), "Olá mundo");
    }

    #[tokio::test]
    async fn openai_gpt_success_uses_modern_multipart_fields_only() {
        let server = MockHttpServer::start(vec![MockReply::Response {
            status: "200 OK",
            content_type: "text/event-stream",
            chunks: vec![
                b"data: {\"type\":\"transcript.text.delta\",\"delta\":\"Ola \"}\n\n",
                b"data: {\"type\":\"transcript.text.done\",\"text\":\"Ola mundo\"}",
            ],
        }]);
        let keywords = vec!["FamVoice".to_string(), "Lisboa".to_string()];

        let result = transcribe_against(
            &server.endpoint,
            "gpt-transcribe",
            "openai",
            &keywords,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(result, "Ola mundo");

        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.starts_with("POST /audio/transcriptions HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-key"));
        assert!(has_multipart_value(request, "model", "gpt-transcribe"));
        assert!(has_multipart_value(request, "languages[]", "pt"));
        assert!(!has_multipart_field(request, "language"));
        assert!(has_multipart_value(request, "keywords[]", "FamVoice"));
        assert!(has_multipart_value(request, "keywords[]", "Lisboa"));
        assert!(has_multipart_value(request, "prompt", "FamVoice"));
        assert!(has_multipart_value(request, "stream", "true"));
        assert!(!has_multipart_field(request, "response_format"));
        assert!(!has_multipart_field(request, "temperature"));
    }

    #[tokio::test]
    async fn openai_whisper_success_uses_legacy_nonstreaming_fields() {
        let server = MockHttpServer::start(vec![MockReply::Response {
            status: "200 OK",
            content_type: "text/plain",
            chunks: vec![b"OpenAI legacy"],
        }]);
        let keywords = vec!["not-sent".to_string()];

        let result = transcribe_against(
            &server.endpoint,
            "whisper-1",
            "openai",
            &keywords,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(result, "OpenAI legacy");

        let requests = server.finish();
        let request = &requests[0];
        assert!(has_multipart_value(request, "language", "pt"));
        assert!(!has_multipart_field(request, "languages[]"));
        assert!(!has_multipart_field(request, "keywords[]"));
        assert!(!has_multipart_field(request, "stream"));
        assert!(has_multipart_value(request, "response_format", "text"));
        assert!(has_multipart_value(request, "temperature", "0"));
    }

    #[tokio::test]
    async fn groq_success_uses_legacy_nonstreaming_fields() {
        let server = MockHttpServer::start(vec![MockReply::Response {
            status: "200 OK",
            content_type: "text/plain",
            chunks: vec![b"  Ola, mundo!  "],
        }]);
        let keywords = vec!["not-sent".to_string()];

        let result = transcribe_against(
            &server.endpoint,
            "whisper-large-v3",
            "groq",
            &keywords,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(result, "Ola, mundo!");

        let requests = server.finish();
        let request = &requests[0];
        assert!(has_multipart_value(request, "model", "whisper-large-v3"));
        assert!(has_multipart_value(request, "language", "pt"));
        assert!(!has_multipart_field(request, "languages[]"));
        assert!(!has_multipart_field(request, "keywords[]"));
        assert!(!has_multipart_field(request, "stream"));
        assert!(has_multipart_value(request, "prompt", "FamVoice"));
        assert!(has_multipart_value(request, "response_format", "text"));
    }

    #[tokio::test]
    async fn openai_and_groq_map_http_errors_without_retrying() {
        for (provider, model, expected) in [
            (
                "openai",
                "gpt-transcribe",
                "OpenAI authentication failed. Check the saved API key.",
            ),
            (
                "groq",
                "whisper-large-v3",
                "Groq authentication failed. Check the saved API key.",
            ),
        ] {
            let server = MockHttpServer::start(vec![MockReply::Response {
                status: "401 Unauthorized",
                content_type: "application/json",
                chunks: vec![b"{}"],
            }]);

            let error = transcribe_against(
                &server.endpoint,
                model,
                provider,
                &[],
                Duration::from_secs(1),
            )
            .await
            .unwrap_err();
            assert_eq!(error, expected);
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[tokio::test]
    async fn openai_and_groq_retry_one_transient_disconnect() {
        for (provider, model, content_type, body, expected) in [
            (
                "openai",
                "gpt-transcribe",
                "text/event-stream",
                b"data: {\"type\":\"transcript.text.done\",\"text\":\"OpenAI recovered\"}"
                    .as_slice(),
                "OpenAI recovered",
            ),
            (
                "groq",
                "whisper-large-v3",
                "text/plain",
                b"Groq recovered".as_slice(),
                "Groq recovered",
            ),
        ] {
            let server = MockHttpServer::start(vec![
                MockReply::Disconnect,
                MockReply::Response {
                    status: "200 OK",
                    content_type,
                    chunks: vec![body],
                },
            ]);

            let result = transcribe_against(
                &server.endpoint,
                model,
                provider,
                &[],
                Duration::from_secs(1),
            )
            .await
            .unwrap();
            assert_eq!(result, expected);
            assert_eq!(server.finish().len(), 2);
        }
    }

    #[tokio::test]
    async fn providers_reject_empty_success_payloads() {
        for (provider, model, content_type, body) in [
            (
                "openai",
                "gpt-transcribe",
                "text/event-stream",
                b"data: [DONE]".as_slice(),
            ),
            (
                "groq",
                "whisper-large-v3",
                "text/plain",
                b" \r\n\t ".as_slice(),
            ),
        ] {
            let server = MockHttpServer::start(vec![MockReply::Response {
                status: "200 OK",
                content_type,
                chunks: vec![body],
            }]);

            let error = transcribe_against(
                &server.endpoint,
                model,
                provider,
                &[],
                Duration::from_secs(1),
            )
            .await
            .unwrap_err();
            assert_eq!(error, EMPTY_TRANSCRIPTION_ERROR);
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[tokio::test]
    async fn http_mock_times_out_after_the_single_retry() {
        let server = MockHttpServer::start(vec![
            MockReply::Stall(Duration::from_millis(60)),
            MockReply::Stall(Duration::from_millis(60)),
        ]);

        let error = transcribe_against(
            &server.endpoint,
            "whisper-large-v3",
            "groq",
            &[],
            Duration::from_millis(15),
        )
        .await
        .unwrap_err();
        assert!(error.to_ascii_lowercase().contains("timed out"));
        assert_eq!(server.finish().len(), 2);
    }
}
