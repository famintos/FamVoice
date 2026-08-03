# Phase 5 pt-PT transcription evaluation

This is an opt-in, paid evaluation harness for choosing the Phase 5 transcription default. It runs the same private pt-PT samples through four variants:

1. Groq `whisper-large-v3` with `language=pt`.
2. OpenAI `whisper-1` with `language=pt`.
3. OpenAI `gpt-transcribe` without language, keywords, or prompt.
4. OpenAI `gpt-transcribe` with `languages[]=pt`, private keywords, and a private prompt.

The harness is a separate benchmark. It does not read or change FamVoice settings, history, glossary, or recordings.

## Privacy boundary

The manifest, audio, ground truth, prompt, keywords, and term list must stay outside the repository. The script rejects a manifest or audio file inside the FamVoice tree. Keep that private corpus out of Git, synced team folders, shell transcripts, and CI artifacts.

Audio is held in memory and uploaded directly to the selected provider only when `-ExecutePaidCalls` is present. API responses remain in memory only. The harness emits only corpus totals and aggregate measurements; it never prints or writes API keys, file paths, raw audio, reference text, prompts, keywords, terms, or provider transcripts. Redirect aggregate JSON only to an approved private location outside the repository if a retained result is needed.

The providers still receive the audio during a paid run. Confirm consent and organizational data controls before using real dictation. Review the current official policies before every run:

- OpenAI business/API data privacy: <https://openai.com/business-data/>
- OpenAI API data controls: <https://platform.openai.com/docs/models/default-usage-policies-by-endpoint>
- GroqCloud data handling and Zero Data Retention: <https://console.groq.com/docs/your-data>

## Private manifest

Create a JSON file outside the repository. Relative `audioPath` values resolve from the private manifest's directory. `durationSeconds` is required so preflight can estimate the same corpus pass without opening an audio decoder.

```json
{
  "version": 1,
  "language": "pt-PT",
  "prompt": "Ditado técnico em português europeu com nomes de produto em inglês.",
  "keywords": ["FamVoice", "PowerShell", "WebView2"],
  "samples": [
    {
      "audioPath": "audio/sample-01.wav",
      "durationSeconds": 8.42,
      "reference": "Texto de referência privado desta gravação.",
      "terms": ["FamVoice", "PowerShell"]
    }
  ]
}
```

Use recordings representative of real pt-PT dictation: short and long utterances, natural pauses, punctuation, technical English terms, product names, numerals, and realistic microphone/noise conditions. Every variant receives exactly the same `samples` array. `terms` is optional per sample; each entry is one expected exact-term checkpoint. Do not list a term unless it was actually spoken in that sample, and list every top-level keyword that was spoken. The harness treats each remaining top-level keyword as deliberately unspoken for that sample so it can detect false keyword occurrences without retaining transcripts.

Supported inputs are the intersection accepted by the evaluated endpoints: `m4a`, `mp3`, `mp4`, `mpeg`, `mpga`, `wav`, and `webm`, up to 25 MB per file. The script requires at least one keyword and a non-empty prompt so the contextual variant cannot silently collapse into the no-context variant.

## Safe preflight

Preflight validates the private manifest and audio metadata, confirms that every private file is outside the repository, calculates the planned cost, and marks all empirical fields `not measured`. It does not inspect environment keys, instantiate an HTTP client, or make any network request.

```powershell
pwsh -NoProfile -File .\scripts\transcription-evaluation.ps1 `
  -ManifestPath 'D:\private\famvoice-eval\manifest.json'
```

The JSON result must say `"Mode": "preflight only - no API calls made"`. Review `EstimatedCorpusPassCostUsd` before opting in.

## Paid execution

Set keys only in the current process environment, then add the explicit paid-call switch. The script reads the variables without printing them.

```powershell
$env:OPENAI_API_KEY = '<private OpenAI key>'
$env:GROQ_API_KEY = '<private Groq key>'

pwsh -NoProfile -File .\scripts\transcription-evaluation.ps1 `
  -ManifestPath 'D:\private\famvoice-eval\manifest.json' `
  -ExecutePaidCalls
```

By default, a transient connection failure, timeout, HTTP 408/429, or HTTP 5xx response gets at most one retry. `RetryCount` is the observed number of extra attempts; `FailureCount` is the number of samples without a final usable transcript. Override conservatively with `-MaxRetries 0..5` and `-RequestTimeoutSeconds 10..900`. Retries can increase real cost even when the estimate cannot know whether a failed provider attempt was billed.

## Metrics and interpretation

The report contains one aggregate row per variant:

- WER is total word-level Levenshtein edits divided by total normalized reference words.
- CER is total character-level Levenshtein edits divided by total normalized reference characters, including normalized single spaces.
- Exact-term accuracy is the share of declared per-sample terms found as the same normalized word sequence. It is `not measured` when no terms are declared.
- Unspoken-keyword occurrence is the share of top-level keyword/sample pairs not declared in that sample's `terms` that nevertheless appear in the transcript. Compare the contextual variant with the no-context baseline; the contextual hints must not increase this rate.
- Normalization lowercases, normalizes Unicode, removes punctuation other than apostrophes, and collapses whitespace. Diacritics remain significant for pt-PT.
- A failed or empty transcription is scored as an empty hypothesis, so its reference contributes deletions and its declared terms count as misses rather than disappearing from accuracy.
- Final latency p50/p95 is end-to-end wall time per sample outcome, including any retry backoff. It includes successes and final failures.
- Failure and retry counts are directly observed by this harness. Provider-internal retries are not observable.
- Cost is an estimate for one planned pass over the corpus per variant. OpenAI uses audio duration. Groq applies its documented 10-second minimum to each sample. The estimate excludes retries, taxes, credits, plan differences, and future price changes.

Use enough varied samples that p95 and error rates are meaningful. A result is evidence for a default only after a paid run reports `measured` for all four variants; preflight output is not transcription evidence. Inspect false-positive term insertions separately in the private working session before accepting a contextual improvement, but never paste those transcripts into the repo report.

## Dated official sources and rates

Rates and request fields were rechecked on **2026-08-02**:

- OpenAI `gpt-transcribe`: **$0.0045/minute**. The official file-transcription guide recommends it and documents `prompt`, `keywords[]`, and `languages[]`: <https://developers.openai.com/api/docs/models/gpt-transcribe> and <https://developers.openai.com/api/docs/guides/speech-to-text>.
- OpenAI `whisper-1`: **$0.006/minute**: <https://developers.openai.com/api/docs/models/whisper-1>.
- Groq `whisper-large-v3`: **$0.111/hour**, with a 10-second minimum billed length per request: <https://console.groq.com/docs/speech-to-text> and <https://groq.com/pricing>.

Recheck these official pages before any future paid run. Model availability, accepted fields, limits, retention controls, and prices can change.
