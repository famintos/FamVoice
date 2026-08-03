# Transcription models

Status reviewed: **2026-08-02**. Provider capabilities and list prices can change; check the linked official sources before a release decision.

FamVoice records a bounded audio clip and uploads the completed file to the selected provider's transcription endpoint. It is not a continuous live-captioning client.

## Supported choices

| Provider | Model | FamVoice role | Language hint | Prompt / vocabulary guidance | File-response streaming | Timestamps, subtitles, translation | Official list price |
| --- | --- | --- | --- | --- | --- | --- | --- |
| OpenAI | `gpt-transcribe` | **Recommended default** for new completed-file dictation | `languages[]` | Context-only `prompt`; literal glossary targets in `keywords[]` | Yes | Use a specialized model when these outputs are required | **$0.0045/min** |
| OpenAI | `whisper-1` | Specialized fallback | `language` | `prompt` (limited compared with the recommended path) | No | Word/segment timestamps, SRT/VTT subtitles, and translation to English | **$0.006/min** |
| Groq | `whisper-large-v3-turbo` | Speed / value option | `language` | `prompt` | Not used by FamVoice | Groq transcription fields support word/segment timestamps; Turbo does not support the translation endpoint | **$0.04/hour** |
| Groq | `whisper-large-v3` | Accuracy-first option | `language` | `prompt` | Not used by FamVoice | Groq supports word/segment timestamps and translation | **$0.111/hour** |

The OpenAI `stream` field streams the response generated from an uploaded, completed file; it does not turn FamVoice into a realtime microphone session. `whisper-1` ignores that field, so FamVoice uses its normal non-streaming response path for that model. The Settings labels describe these roles rather than implying that the most expensive model is always the best choice.

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
