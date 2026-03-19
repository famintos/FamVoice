# Prompt Optimizer for Coding Agents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorient the prompt optimizer so `Improve into prompt` produces English implementation-ready prompts for coding agents instead of generic cleaned-up prompts.

**Architecture:** Keep the existing optimizer plumbing, request building, timeout logic, and fallback behavior intact. Replace the metaprompt contract and examples so the Anthropic pass explicitly produces code-agent-oriented implementation prompts, then update the settings copy to match the new behavior.

**Tech Stack:** Rust, Tauri, reqwest, React, TypeScript

---

### Task 1: Tighten the metaprompt contract tests

**Files:**
- Modify: `src-tauri/src/prompt_optimizer/metaprompt.rs`
- Test: `src-tauri/src/prompt_optimizer/metaprompt.rs`

- [ ] **Step 1: Write the failing metaprompt assertions**

Replace the current `system_instruction_contains_few_shot_examples` test with assertions that match the approved spec:

```rust
#[test]
fn system_instruction_targets_coding_agent_implementation_prompts() {
    let instruction = SYSTEM_INSTRUCTION.to_lowercase();

    assert!(instruction.contains("coding agent") || instruction.contains("codex"));
    assert!(instruction.contains("english"));
    assert!(instruction.contains("inspect the codebase") || instruction.contains("inspect the existing codebase"));
    assert!(instruction.contains("acceptance criteria"));
    assert!(instruction.contains("testing and verification"));
    assert!(instruction.contains("assumptions"));
    assert!(instruction.contains("avoid unrelated refactors"));
}
```

- [ ] **Step 2: Run the targeted test to verify RED**

Run: `cargo test prompt_optimizer::metaprompt::tests::system_instruction_targets_coding_agent_implementation_prompts -- --nocapture`

Expected: FAIL because the current instruction still targets generic voice-to-prompt cleanup and still forbids the structured output this redesign wants.

- [ ] **Step 3: Add one more failing test for example orientation**

Add a second test that verifies the few-shot examples are implementation-oriented and no longer include generic writing/email examples:

```rust
#[test]
fn system_instruction_examples_focus_on_implementation_work() {
    let instruction = SYSTEM_INSTRUCTION.to_lowercase();

    assert!(instruction.contains("ui feature") || instruction.contains("bugfix") || instruction.contains("implementation"));
    assert!(!instruction.contains("write an email to my team"));
}
```

- [ ] **Step 4: Run the second targeted test to verify RED**

Run: `cargo test prompt_optimizer::metaprompt::tests::system_instruction_examples_focus_on_implementation_work -- --nocapture`

Expected: FAIL because the current examples still include a writing/email example.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/prompt_optimizer/metaprompt.rs
git commit -m "test: redefine prompt optimizer metaprompt contract"
```

### Task 2: Rewrite the metaprompt for coding agents

**Files:**
- Modify: `src-tauri/src/prompt_optimizer/metaprompt.rs`
- Test: `src-tauri/src/prompt_optimizer/metaprompt.rs`

- [ ] **Step 1: Replace the system instruction**

Rewrite `SYSTEM_INSTRUCTION` so it:
- frames the model as converting dictated implementation ideas into execution-ready prompts for Codex, Gemini, and Claude Code
- requires English output
- tells the model to instruct the agent to inspect the existing codebase and follow current patterns
- uses the approved hybrid ambiguity policy
- encourages semi-structured output with `Objective`, `Requested behavior`, `Implementation notes`, `Assumptions`, `Acceptance criteria`, and `Testing and verification` when useful
- removes the generic "no markdown formatting" rule and instead forbids extra meta commentary around the final prompt
- replaces generic writing examples with coding-agent-oriented examples for feature work, bugfixes, or behavior changes

- [ ] **Step 2: Run the metaprompt tests to verify GREEN**

Run: `cargo test prompt_optimizer::metaprompt::tests -- --nocapture`

Expected: PASS

- [ ] **Step 3: Run the backend optimizer tests that depend on the instruction**

Run: `cargo test prompt_optimizer -- --nocapture`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/prompt_optimizer/metaprompt.rs
git commit -m "feat: retarget prompt optimizer for coding agents"
```

### Task 3: Align the settings copy with the new optimizer behavior

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Update the prompt optimization description copy**

Change the section helper text and checkbox helper text so they explicitly describe:
- an Anthropic pass after transcription
- English implementation-ready prompts
- coding agents such as Codex, Gemini, and Claude Code

Keep the existing controls and layout unchanged.

- [ ] **Step 2: Run the frontend build**

Run: `npm run build`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "docs: clarify coding-agent prompt optimization copy"
```

### Task 4: Final verification

**Files:** None

- [ ] **Step 1: Run focused backend verification**

Run: `cargo test prompt_optimizer -- --nocapture`

Expected: PASS

- [ ] **Step 2: Run the prompt optimizer integration fallback tests**

Run: `cargo test test_resolve_final_output -- --nocapture`

Expected: PASS

- [ ] **Step 3: Run the frontend build again**

Run: `npm run build`

Expected: PASS

- [ ] **Step 4: Review branch status**

Run: `git status --short`

Expected: only the intended plan, metaprompt, and UI changes are present.
