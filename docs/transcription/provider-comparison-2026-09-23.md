# Provider comparison for Portuguese dictation

Research snapshot: 23 September 2026. Scope: European Portuguese dictation containing English technical terms. This is a literature and documentation review, not an audio evaluation or a change to the application.

## What the evidence supports

There is no demonstrated winner for this exact speaker, accent, microphone and mixed-language vocabulary in the evidence reviewed. General multilingual scores and vendor benchmarks are useful for choosing candidates, not for declaring a winner.

- [CAMÕES, August 2025](https://arxiv.org/html/2508.19721v1) evaluates 46.2 hours of European Portuguese across 14 test corpora and five domains. Its table III reports average WER of 19.2% for zero-shot Whisper Large v3 with WhisperX, versus 12.5% after European Portuguese fine-tuning. This supports accent-specific evaluation and attention to preprocessing. It does **not** compare current hosted transcription APIs or establish performance for Portuguese-English code-switching. Its averages are domain-balanced, not a direct forecast for desktop dictation.
- [Commercial code-switching benchmark, May 2026, v3](https://arxiv.org/pdf/2605.19069) evaluates Arabic-English (two varieties), Persian-English and German-English, 300 samples each. Scribe v2 leads the reported overall results (13.2% WER). **Portuguese is absent.** Speakers recorded supplied transcripts conversationally in quiet indoor environments; this is not an unrestricted spontaneous dictation sample. Systems include Scribe v2, GPT-4o-transcribe, Chirp 3, Azure and Nova-3; Deepgram is excluded from Arabic/Persian aggregates because those pairs fall outside its documented support. Soniox and Voxtral are absent. The study therefore supports including Scribe in a trial, not declaring it the best provider for FamVoice.

## Price shortlist

Public list prices reviewed during this research session. USD before taxes; arithmetic assumes 300 minutes/month and excludes optional features unless stated. Account eligibility, billing minimums, credits and actual token usage may change the invoice. Pricing is not evidence of transcription accuracy.

| Candidate | Published usage rate | Approximate five-hour usage cost | Qualification |
| --- | --- | --- | --- |
| Groq Whisper Large v3 | $0.111/hour paid | $0.56, or $0 within eligible free-plan limits | Strong free baseline; minimum ten seconds per paid request. |
| Groq Whisper Large v3 Turbo | $0.04/hour paid | $0.20, or $0 within eligible free-plan limits | Speed/cost baseline; same request-duration caveat. |
| Soniox v5 | About $0.10/hour asynchronous; $0.12/hour realtime | About $0.50 / $0.60 | Hourly figures are token-based estimates, not a universal fixed audio rate. Do not promise recurring free usage. |
| Mistral Voxtral Mini Transcribe V2 | $0.003/minute | $0.90 | Free usage eligibility/limits must be checked on the account. |
| OpenAI GPT-Transcribe | $0.0045/minute | $1.35 | Candidate already represented in the repository evaluation workflow. |
| ElevenLabs Scribe v2 | $0.22/hour | $1.10 | Pay-as-you-go available without subscription commitment; keyterm prompting adds $0.05/hour ($0.25 at five hours). |
| AssemblyAI Universal-3.5 | $0.21/hour asynchronous | $1.05 | Keyterms and prompting each add $0.05/hour. Synchronous service is $0.45/hour with keyterms included; do not equate async and synchronous prices. |
| Speechmatics Melia 1 | $0.129/hour | $0.65 | Advertised $100 credit is one-time, not recurring free service. |

Official sources: [Groq speech-to-text](https://console.groq.com/docs/speech-to-text), [Groq rate limits](https://console.groq.com/docs/rate-limits), [Soniox pricing](https://soniox.com/pricing), [Soniox signup-credit change](https://soniox.com/blog/2025-10-27-free-credits-update-for-soniox-api), [Mistral pricing](https://docs.mistral.ai/inference/pricing), [Voxtral V2 announcement](https://mistral.ai/news/voxtral-transcribe-2/), [OpenAI pricing](https://developers.openai.com/api/docs/pricing), [ElevenLabs API pricing](https://elevenlabs.io/pricing/api), [AssemblyAI pricing](https://www.assemblyai.com/pricing/), [Speechmatics pricing](https://www.speechmatics.com/pricing).

## Practical conclusion

Keep Groq Large v3 as the free reference. Compare Soniox, Voxtral V2 and Scribe v2 against the existing OpenAI option before selecting a paid default. Soniox is an attractive inexpensive candidate; that is a product judgment from its price and multilingual positioning, not a measured Portuguese accuracy result. API integration differences, final-result latency and handling of custom terminology matter alongside word error rate.

Use the existing [pt-PT evaluation workflow](phase5-pt-pt-evaluation.md) as the starting point, adding providers only when implementation work is authorized. Evaluate the same private samples with identical reference text and terminology; report aggregate word errors, English-term errors, unwanted translation, hallucinations, final-result latency and billed cost. No paid requests, private audio uploads, native smoke run, settings changes, commits or releases were performed for this note.
