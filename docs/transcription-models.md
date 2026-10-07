# Transcription models

Soniox integration reviewed: **2026-09-23**. Original OpenAI/Groq matrix reviewed: **2026-08-02**. Provider capabilities and list prices can change; check the linked official sources before a release decision.

FamVoice records bounded dictations. OpenAI/Groq send completed clips by file upload; Soniox streams PCM during capture over a bounded WebSocket session. It is not a continuous live-captioning client.

## OpenRouter: MAI Transcribe 2 and Scribe v2

Choose **OpenRouter** in Settings, then select **MAI Transcribe 2**
(`microsoft/mai-transcribe-2`) or **Scribe v2** (`elevenlabs/scribe-v2`). Both
reuse the same saved OpenRouter API key. MAI remains the default; a saved Scribe
selection survives restarting FamVoice. Try **Auto Detect** for Portuguese mixed
with English, or **Portuguese** to provide a language hint.

Both models send the completed FLAC/WAV clip to OpenRouter's
`/api/v1/audio/transcriptions` endpoint and deliver the final transcript through
the existing replacement and paste flow. Prompt optimization stays disabled for
OpenRouter. Scribe requests verbatim speech without speaker labels, timestamps,
audio-event tags, or transcript editing. Valid glossary targets use ElevenLabs
`keyterms`; replacement values and prompt instructions are never sent. Keyterms
are omitted when there are no valid glossary targets, and can incur an additional
provider charge when used. Scribe clips are limited to 25 MiB.

Pricing checked **2026-10-07**: MAI is **$0.10/hour**; Scribe's OpenRouter
endpoint lists **$0.11/hour** with a **50% promotional discount**. The promotion
can change, so the picker does not hardcode a discounted price.

Official sources: [MAI](https://openrouter.ai/microsoft/mai-transcribe-2),
[Scribe](https://openrouter.ai/elevenlabs/scribe-v2),
[Scribe endpoint catalog](https://openrouter.ai/api/v1/models/elevenlabs/scribe-v2/endpoints),
[OpenRouter STT contract](https://openrouter.ai/docs/guides/overview/multimodal/stt),
[ElevenLabs options](https://elevenlabs.io/docs/api-reference/speech-to-text/convert).
Automated transport and settings tests do not establish recognition quality or
microphone-to-paste latency for the user's voice.

## Supported choices

| Provider | Model | FamVoice role | Language hint | Prompt / vocabulary guidance | File-response streaming | Timestamps, subtitles, translation | Official list price |
| --- | --- | --- | --- | --- | --- | --- | --- |
| OpenAI | `gpt-transcribe` | **Recommended default** for new completed-file dictation | `languages[]` | Context-only `prompt`; literal glossary targets in `keywords[]` | Yes | Use a specialized model when these outputs are required | **$0.0045/min** |
| OpenAI | `whisper-1` | Specialized fallback | `language` | `prompt` (limited compared with the recommended path) | No | Word/segment timestamps, SRT/VTT subtitles, and translation to English | **$0.006/min** |
| Groq | `whisper-large-v3-turbo` | Speed / value option | `language` | `prompt` | Not used by FamVoice | Groq transcription fields support word/segment timestamps; Turbo does not support the translation endpoint | **$0.04/hour** |
| Groq | `whisper-large-v3` | Accuracy-first option | `language` | `prompt` | Not used by FamVoice | Groq supports word/segment timestamps and translation | **$0.111/hour** |

The OpenAI `stream` field streams the response generated from an uploaded, completed file; it does not turn FamVoice into a realtime microphone session. `whisper-1` ignores that field, so FamVoice uses its normal non-streaming response path for that model. The Settings labels describe these roles rather than implying that the most expensive model is always the best choice.

## Soniox v5

`stt-rt-v5` is an additional provider choice; existing selections and defaults are preserved.

- Uses `wss://stt-rt.soniox.com/transcribe-websocket`, with authentication in the initial JSON message, then 16 kHz mono PCM (`pcm_s16le`) during capture. Manual retry uses binary FLAC/WAV frames (`audio_format: auto`). An empty **text** frame ends audio; only confirmed tokens are accumulated and delivery waits for `finished: true`. Provisional tokens, translations and control markers are excluded. A close before completion fails rather than delivering partial text.
- Portuguese sends `language_hints: ["pt", "en"]` plus the existing European Portuguese context. Other selected languages send their own hint; Auto Detect omits hints. Literal glossary targets go to `context.terms`; replacement outputs are not sent as vocabulary.
- The app opens the connection when recording starts and sends blocks of up to 200 ms from the existing bounded capture buffer. Settings and glossary are captured at recording start. Auto-gain runs per block; optional noise suppression retains its state and incomplete frames and flushes its delay once at the end. The capture pre-roll and release tail are retained. Silence is still rejected before delivery. No captions or provisional text are delivered. The session is bounded to 330 seconds, finalization to 30 seconds, with bounded incoming messages. Dropping the session cancels its task/socket; session IDs prevent cross-recording reads. No automatic retry replays billable audio. The existing single-use RAM retry path remains available after a streaming failure.
- The key follows the existing Windows Credential Manager / DPAPI recovery rule. Soniox is included in the content-free provider connectivity test (`GET https://api.soniox.com/v1/models`). No new frontend network permissions are needed.
- No remote file or transcription job is created by this WebSocket path. Do not infer the provider's contractual retention policy from this implementation; consult [Soniox privacy documentation](https://soniox.com/docs/security-and-privacy).
- Estimated usage price: **$0.12/hour**, token-based, before taxes. See [pricing](https://soniox.com/pricing).
- Automated tests cover wire framing, finalization, mixed-language hints, sanitized errors, disconnect, timeout/cancellation, secret persistence and provider switching. A live synthetic-tone comparison on 2026-09-23 reproduced 408 for both FLAC and WAV with binary EOF (~21 seconds); changing only EOF to empty text completed both streams (~1.8 seconds, no speech). Despite the reference allowing either frame type, keep text EOF to avoid the observed timeout. This verifies live transport/finalization, not speech accuracy, native dictation or the billed amount.

Official contract: [WebSocket API](https://soniox.com/docs/api-reference/stt/websocket-api), [models](https://soniox.com/docs/stt/models), [language hints](https://soniox.com/docs/stt/concepts/language-hints), [models/auth test](https://soniox.com/docs/api-reference/stt/get_models).

### Observed latency of completed-clip upload (2026-09-23)

A bounded live comparison used identical locally synthesized English speech, encoded
as 16 kHz mono FLAC entirely in memory, with saved provider credentials. Only durations
and success/failure were recorded; audio and returned text were not persisted or logged.

| Audio duration | Soniox `stt-rt-v5` | Groq `whisper-large-v3-turbo` |
| --- | --- | --- |
| 4.14 s | 2.16 s | 0.22 s |
| 16.54 s | 13.17 s | 0.23 s |

These are single observations per provider and clip, measured from the transcription
call through its completed result, excluding microphone capture, prompt optimization
and paste. They are not pt-PT accuracy results or latency guarantees. This was the
original Soniox path, which delayed all processing until recording ended; it remains
available for manual retry. Groq was faster for that completed-clip workflow.

### Capture-time streaming comparison (2026-09-23)

The same synthesized speech was subsequently fed at microphone speed through the
production streaming function, including incremental gain and PCM framing. Waiting
from simulated release to final result was **146 ms** for 4.14 s of speech and
**192 ms** for 16.54 s. Both recognized all six checked English keywords. These are
single live observations, with noise suppression disabled, not guarantees or a pt-PT
accuracy evaluation. No audio or returned transcript was logged or saved. The temporary
authenticated benchmark was removed after measurement. Native hotkey/capture/paste and
the user's Portuguese/English dictation still need interactive validation.

## Request-field matrix

| Field | `gpt-transcribe` | OpenAI `whisper-1` | Groq Whisper models | FamVoice policy |
| --- | --- | --- | --- | --- |
| `file` | Required | Required | Required unless Groq `url` is used | FamVoice sends the recorded file |
| `model` | Required | Required | Required | Must match the selected provider |
| `language` | Do not send | Optional | Optional | Used only by Whisper-compatible paths; omitted for Auto Detect |
| `languages[]` | Optional | Do not send | Do not send | `gpt-transcribe` receives the selected ISO-639-1 language; never sent together with `language` |
| `prompt` | Optional | Optional | Optional | `gpt-transcribe` receives bounded language/context instructions only; Whisper/Groq keep the established vocabulary prompt |
| `keywords[]` | Supported | Not supported | Not part of the Groq-compatible contract used here | Only literal, filtered glossary targets are sent to `gpt-transcribe`; replacement values are excluded |
| `response_format` | Supported formats depend on the model | `json`, `text`, `srt`, `verbose_json`, `vtt` | `json`, `verbose_json`, `text` | Omitted on the streaming `gpt-transcribe` path; Whisper/Groq request plain text |
| `stream` | Supported for completed-file response events | Not supported; ignored | Not used by FamVoice | Enabled only on the compatible OpenAI path |
| `timestamp_granularities` | Not the specialized path | `word` and/or `segment` with `verbose_json` | `word` and/or `segment` | Not requested by normal dictation |

## Versioned migration policy

The persisted JSON field `transcription_model_settings_version` is currently `1`.

- New settings and provider switches to OpenAI select `gpt-transcribe` explicitly. The implementation uses named provider defaults; it does not infer a default from array order.
- An older settings file with no version (treated as version `0`) and `transcription_provider: "openai"` migrates once to `gpt-transcribe`, including old `whisper-1`, missing models, unsupported values, and legacy `gpt-4o-*-transcribe` values. The rewritten file records version `1`, and Settings shows a sanitized migration notice for that app session.
- After version `1` is recorded, an explicit OpenAI `whisper-1` choice is preserved across load and save. It is not silently changed on the next launch.
- Both valid Groq choices are preserved during the version migration. An invalid Groq model normalizes to the named Groq default, `whisper-large-v3-turbo`.
- Changing providers selects that provider's named default. Saving validates the provider/model pair before it reaches the transcription client.

## pt-PT evaluation protocol

Evaluation status on **2026-08-02**: **not run**. No provider credentials and no approved representative audio/reference-transcript set were available in this work session. The accuracy/latency gate has therefore **not passed**; the recommendation above follows current provider guidance and must still be validated on FamVoice audio before production traffic is deliberately moved.

The executable privacy boundary, private-manifest format, safe preflight, paid-run command, aggregation rules, and dated rates live in [Phase 5 pt-PT transcription evaluation](transcription/phase5-pt-pt-evaluation.md). The checklist below is the release-level interpretation of that harness.

Run the comparison on the same Windows machine, network, microphone path, and lossless input files:

1. Prepare consented pt-PT clips with exact human reference transcripts, covering quiet speech, background noise, short commands, long dictation, numbers, punctuation, names, English code terms, acronyms, and glossary terms.
2. Submit every clip through the four harness variants: Groq `whisper-large-v3`, OpenAI `whisper-1`, `gpt-transcribe` without context, and `gpt-transcribe` with pt/keywords/prompt. Use at least five runs when producing the release decision, and randomize request order to reduce network/time bias.
3. Record word error rate, character error rate, exact glossary-term recall, false insertion of unspoken keyword hints, number/date fidelity, punctuation review, empty/error rate, total latency, audio duration, and estimated provider cost. Do not log API keys or raw transcript content outside the approved private working session.
4. Report median and p95 latency separately from accuracy. Review substitutions manually with a native pt-PT speaker; aggregate WER alone can hide harmful proper-name or negation errors.
5. Keep `gpt-transcribe` as the default only if the representative pt-PT set confirms acceptable accuracy and reliability for dictation. Retain `whisper-1` for its specialized output capabilities regardless of the general dictation ranking.

## Official sources

- OpenAI: [Transcription model guidance](https://developers.openai.com/api/docs/guides/transcription#choose-a-specialized-capability)
- OpenAI: [File transcription and reliability](https://developers.openai.com/api/docs/guides/speech-to-text)
- OpenAI: [Create transcription API reference](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)
- OpenAI: [Transcription pricing](https://developers.openai.com/api/docs/pricing#transcription-and-speech)
- OpenAI: [Migration from Whisper to GPT-Transcribe](https://developers.openai.com/cookbook/examples/migrating_from_whisper_to_gpt_transcribe)
- Groq: [Speech-to-text models, fields, capabilities, and pricing](https://console.groq.com/docs/speech-to-text)
- Groq: [`whisper-large-v3` model page](https://console.groq.com/docs/model/whisper-large-v3)
- Groq: [`whisper-large-v3-turbo` model page](https://console.groq.com/docs/model/whisper-large-v3-turbo)
