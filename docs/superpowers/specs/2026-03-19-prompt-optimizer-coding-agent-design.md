# Prompt Optimizer for Coding Agents

## Status

Approved during brainstorming on March 19, 2026. This spec supersedes the generic prompt-cleanup direction for the prompt optimizer and narrows the feature to implementation-ready prompts for coding agents.

## Problem

The current prompt optimizer is still behaving like a transcript cleanup pass:

- it removes speech noise and rewrites the sentence
- it does not consistently expand a feature idea into an implementation-ready request
- it does not reliably produce the kind of structure that helps coding agents execute well

That behavior is too weak for the real use case. The product goal is not "make the transcript read better." The goal is "turn a spoken implementation idea into a prompt that a coding agent can act on with minimal ambiguity."

The user's concrete failure case is representative:

- spoken intent: add a new app feature where the waves get larger as the user speaks
- current optimizer output: a lightly cleaned sentence saying to implement larger waves while the user is talking

That output preserves the topic, but it does not provide enough execution guidance for Codex, Gemini, or Claude Code.

## Goal

When `Improve into prompt` is enabled, FamVoice should output an English implementation-ready prompt for coding agents instead of a generic cleaned-up prompt.

The optimized output should help an agent:

- understand the requested behavior quickly
- inspect the existing codebase before changing behavior
- preserve the current implementation style and architecture
- avoid scope creep and unrelated refactors
- verify the change with appropriate testing or validation

## Non-goals

- Supporting generic writing prompts, email prompts, or broad non-coding prompt use cases in this mode
- Adding new UI modes such as "generic prompt" vs "coding prompt"
- Inventing product decisions, frameworks, APIs, or file paths that were not stated
- Replacing the existing transcription, cleanup, or fallback behavior
- Building a prompt editor, preview screen, or prompt template manager

## Product Decision

The optimizer should be intent-locked to coding-agent usage.

When `Improve into prompt` is enabled:

- the output is always in English
- the output is always oriented toward implementation work for coding agents
- the optimizer should convert rough speech into an execution-ready implementation request, not a polished paraphrase

This keeps the product simple and aligned with the actual use case instead of trying to satisfy incompatible prompt styles with one toggle.

## Behavior

### Input

The optimizer receives the finalized transcript after existing cleanup and glossary replacement.

This remains important because glossary corrections improve the optimizer input before the second model pass runs.

### Output

The optimizer returns a prompt meant to be pasted directly into a coding agent such as Codex, Gemini, or Claude Code.

The prompt should:

- preserve the feature or change request from the user
- restate the implementation goal clearly
- translate rough dictation into explicit engineering instructions
- include explicit low-risk assumptions where they help execution
- keep material ambiguities visible instead of guessing

### Language

The final optimized prompt should always be in English, even when the source transcript is dictated in Portuguese.

This is a deliberate product choice to maximize compatibility with the target coding agents and to keep the resulting prompts consistent.

## Output Shape

The optimizer should move from "clean sentence rewrite" to a semi-structured implementation prompt.

Recommended shape:

1. A short opening instruction telling the agent to inspect the existing codebase and implement the requested change
2. `Objective`
3. `Requested behavior`
4. `Implementation notes` when constraints or useful context exist
5. `Assumptions` only for low-risk defaults
6. `Acceptance criteria`
7. `Testing and verification`

This shape is semi-stable, not rigid. The optimizer should adapt to the complexity of the request:

- short feature idea -> compact prompt with only the sections that add value
- medium implementation request -> structured prompt with the main sections
- larger multi-part change -> fuller structure with explicit acceptance and verification guidance

For feature requests, `Acceptance criteria` and `Testing and verification` should appear most of the time because they materially improve execution quality for coding agents.

## Guardrails

The optimizer should be aggressive about clarity and conservative about invention.

### Must Do

- clean up speech artifacts, filler words, false starts, and repetition
- preserve the core user request
- translate spoken intent into implementation-oriented instructions
- tell the agent to inspect the current codebase and follow existing patterns
- make low-risk implementation defaults explicit when they are broadly useful

### Must Not Do

- invent frameworks, libraries, architecture decisions, or file names the user did not mention
- silently convert major ambiguity into a concrete product decision
- broaden scope into unrelated refactors or improvements
- produce generic meta commentary like "here is the improved prompt"

### Ambiguity Policy

Use a hybrid policy:

- make low-risk assumptions explicit when they help the agent start well
- keep product-level or architecture-level ambiguities explicit
- when something important is unclear, instruct the agent to inspect the codebase and surface the ambiguity before broadening scope

Examples of acceptable low-risk assumptions:

- preserve existing architecture and conventions
- avoid unrelated refactors
- add or update tests when the project already has coverage in that area
- maintain current styling direction and responsive behavior where applicable

Examples of ambiguities that should stay explicit:

- exact interaction design when the user only described the high-level effect
- whether a new dependency should be introduced
- whether the requested behavior belongs in one component, several components, or a backend flow

## Integration

The app integration should stay simple.

### Existing Flow to Keep

- record audio
- transcribe audio
- run current transcript finalization and glossary replacement
- if prompt optimization is disabled, use the finalized transcript as-is

### Optimized Flow

If `Improve into prompt` is enabled and an Anthropic API key is configured:

1. send the finalized transcript to the optimizer
2. receive an English implementation-ready prompt for coding agents
3. paste and store that optimized prompt

If the optimizer fails, times out, or returns invalid output:

- fall back to the finalized transcript
- preserve the current silent fallback behavior
- do not break the primary dictation flow

## Metaprompt Design

The system instruction should be rewritten around the new job:

- role: convert dictated implementation ideas into execution-ready prompts for coding agents
- target agents: Codex, Gemini, Claude Code
- output language: English
- output type: implementation-ready request, not generic paraphrase

The metaprompt should explicitly instruct the model to:

- preserve the requested feature or change
- structure the result for implementation work
- include acceptance and verification expectations when useful
- prefer existing codebase conventions over speculative redesigns
- use explicit low-risk assumptions instead of hidden guesses

The few-shot examples should be replaced with coding-agent-oriented examples, such as:

- UI feature request
- behavior change or bugfix request
- small implementation request inside an existing app
- multi-step feature request with acceptance and testing expectations

The examples should show the difference between:

- a weak cleaned-up sentence
- a strong code-agent prompt with structure and execution guidance

## Example Outcome

For a spoken request like:

> I want to implement a new feature in the app where the waves get bigger as I speak

The desired output should look closer to:

Inspect the current voice UI implementation and add reactive waveform scaling tied to live microphone intensity while the user is actively recording. Preserve the existing visual style and interaction model, avoid unrelated UI refactors, and follow the current component and animation patterns in the codebase.

Objective
- Make the waveform feel more responsive by increasing its visual amplitude as live speech intensity rises during an active recording session.

Requested behavior
- Keep the waveform visible during recording.
- Increase wave height or amplitude in response to live mic input level.
- Reduce the waveform back toward its idle or lower-energy state when speech intensity drops.
- Keep the interaction smooth and avoid jittery or distracting motion.

Assumptions
- Preserve the current widget and main-window behavior unless the existing implementation clearly requires a different integration point.
- Reuse the current visual system instead of introducing a new style direction.

Acceptance criteria
- The waveform visibly responds to live speaking intensity during recording.
- Louder speech produces larger waves than quieter speech.
- The animation remains stable and consistent with the existing UI.

Testing and verification
- Add or update tests if this area already has automated coverage.
- Otherwise verify the behavior manually and confirm there are no regressions in the existing recording UI.

The exact wording does not need to match this example, but the level of execution guidance should.

## Testing

The redesign should be covered with tests that validate the metaprompt contract rather than brittle exact-string output.

At minimum:

- assert the system instruction states that the output is for coding agents
- assert the system instruction requires English output
- assert the system instruction includes a codebase inspection requirement
- assert the system instruction encodes the hybrid ambiguity policy
- assert the system instruction includes acceptance and testing guidance
- assert few-shot examples are implementation-oriented rather than generic writing examples

Existing request construction and fallback tests should stay in place.

## Acceptance Criteria

- With prompt optimization enabled, the optimizer is clearly oriented toward coding-agent implementation prompts
- The optimized output is always in English
- The output is richer than a cleaned-up paraphrase and usually includes implementation-relevant structure
- The optimizer uses explicit low-risk assumptions but does not invent major product decisions
- The app keeps the current fallback behavior when optimization fails
- No extra UI mode is introduced for this redesign

## Recommended Next Step

Implementation planning should focus on:

1. replacing the current metaprompt with a coding-agent-specific system instruction
2. updating few-shot examples to match implementation work
3. strengthening tests around the new metaprompt contract
4. leaving the existing optimizer plumbing and UI mostly unchanged unless implementation exposes a concrete gap
