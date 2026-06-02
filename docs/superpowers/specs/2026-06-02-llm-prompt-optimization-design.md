# LLM Prompt Optimization Design

## Problem

The current LLM summary output is too long. Users want concise summaries: 1-2 sentences, under 50 words, factual summary only (no opinions or conclusions).

## Current State

- **SYSTEM_PROMPT**: "50-80 words, capturing the core insight" → too vague, LLM often exceeds 80 words
- **temperature**: 0.3 → too high for factual summary task
- **max_tokens**: 4096 → using model default instead

## Solution

### 1. Optimize SYSTEM_PROMPT

```rust
const SYSTEM_PROMPT: &str = r#"Translate the article to the target language and write a summary.

Output format — each tag on its own line, nothing else:
|||TITLE||| <translated title> |||END_TITLE|||
|||CONTENT||| <translated content> |||END_CONTENT|||
|||SUMMARY||| <1-2 sentences, factual summary only> |||END_SUMMARY|||

Rules:
- Preserve HTML/code/technical terms
- Summary: what the article is about, no opinions or conclusions
- Summary must be under 50 words"#;
```

**Changes:**
- Summary requirement: "50-80 words, capturing the core insight" → "1-2 sentences, factual summary only"
- Add explicit constraint: "no opinions or conclusions"
- Add hard word limit: "under 50 words"

### 2. Optimize temperature

- Change from 0.3 to 0.1
- Lower temperature = more focused, deterministic output → better for concise summaries

### 3. max_tokens

- Remove explicit max_tokens setting, use model default
- This allows the model to handle long articles without artificial truncation

## Files to Modify

- `src/llm/translate_summarize.rs` - SYSTEM_PROMPT
- `src/llm/mod.rs` - temperature, max_tokens

## Verification

- Run `cargo test` to ensure parsing logic still works
- Run `cargo clippy` to check for warnings
