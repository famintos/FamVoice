use crate::settings;

const MAX_TRANSCRIPTION_PROMPT_CHARS: usize = 800;
const BUILT_IN_TRANSCRIPTION_TERMS: &[&str] = &[
    "FamVoice",
    "FamSpace",
    "FamDesign",
    "FamBrand",
    "OpenAI",
    "Groq",
    "Tauri",
    "React",
    "Rust",
    "TypeScript",
    "JavaScript",
    "Node.js",
    "Vite",
    "Tailwind CSS",
    "PowerShell",
    "GitHub",
    "Git",
    "Whisper Large V3",
    "clipboard",
    "hotkey",
    "frontend",
    "backend",
    "worker",
    "coordinator",
    "Backend Reviewer",
    "npm run build",
    "npm test",
    "npm install",
    "cargo test",
    "cargo check",
    "JSON",
    "API",
    "package.json",
    "tauri.conf.json",
    "src-tauri",
    "src-tauri/src/lib.rs",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GlossaryRule {
    target: String,
    replacement: String,
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '_'
}

fn is_single_word_target(target: &str) -> bool {
    !target.is_empty() && target.chars().all(is_word_char)
}

fn sorted_glossary_rules(
    replacements: &[settings::Replacement],
) -> (Vec<GlossaryRule>, Vec<GlossaryRule>) {
    let mut phrase_rules = Vec::new();
    let mut single_word_rules = Vec::new();

    for replacement in replacements {
        let target = replacement.target.trim();
        if target.is_empty() {
            continue;
        }

        let rule = GlossaryRule {
            target: target.to_string(),
            replacement: replacement.replacement.clone(),
        };

        if is_single_word_target(target) {
            single_word_rules.push(rule);
        } else {
            phrase_rules.push(rule);
        }
    }

    let sort_rules = |rules: &mut Vec<GlossaryRule>| {
        rules.sort_by(|left, right| {
            right
                .target
                .len()
                .cmp(&left.target.len())
                .then_with(|| left.target.cmp(&right.target))
        });
    };

    sort_rules(&mut phrase_rules);
    sort_rules(&mut single_word_rules);
    (phrase_rules, single_word_rules)
}

fn transcription_instruction(language: &str) -> Option<&'static str> {
    match language.trim() {
        "pt" => Some(
            "Transcreve literalmente em português europeu. Não traduzas nem reformules. Mantém palavras em inglês, nomes, marcas, comandos e termos técnicos exatamente como forem ditos.",
        ),
        _ => None,
    }
}

fn normalized_prompt_term(term: &str) -> Option<String> {
    let normalized = term.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() || normalized.chars().count() > 64 {
        None
    } else {
        Some(normalized)
    }
}

fn push_unique_prompt_term(terms: &mut Vec<String>, term: &str) {
    let Some(normalized) = normalized_prompt_term(term) else {
        return;
    };
    if terms
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
    {
        return;
    }
    terms.push(normalized);
}

fn transcription_prompt_terms(replacements: &[settings::Replacement]) -> Vec<String> {
    let mut terms = Vec::new();

    for replacement in replacements {
        let preferred = replacement.replacement.trim();
        let fallback = replacement.target.trim();
        push_unique_prompt_term(
            &mut terms,
            if preferred.is_empty() {
                fallback
            } else {
                preferred
            },
        );
        if terms.len() >= 20 {
            break;
        }
    }

    for term in BUILT_IN_TRANSCRIPTION_TERMS {
        push_unique_prompt_term(&mut terms, term);
    }

    terms
}

pub(crate) fn transcription_prompt(
    language: &str,
    replacements: &[settings::Replacement],
) -> Option<String> {
    let instruction = transcription_instruction(language)?;
    let mut prompt = instruction.to_string();
    let terms = transcription_prompt_terms(replacements);

    if terms.is_empty() {
        return Some(prompt);
    }

    let prefix = " Vocabulário esperado: ";
    let mut selected_terms = Vec::new();
    let base_len = prompt.chars().count() + prefix.chars().count() + 1;
    let mut projected_len = base_len;

    for term in terms {
        let separator_len = if selected_terms.is_empty() { 0 } else { 2 };
        let next_len = projected_len + separator_len + term.chars().count();
        if next_len > MAX_TRANSCRIPTION_PROMPT_CHARS {
            break;
        }
        projected_len = next_len;
        selected_terms.push(term);
    }

    if !selected_terms.is_empty() {
        prompt.push_str(prefix);
        prompt.push_str(&selected_terms.join(", "));
        prompt.push('.');
    }

    Some(prompt)
}

fn replace_literal_phrase_case_insensitive(text: &str, target: &str, replacement: &str) -> String {
    let target_lower = target.to_lowercase();
    let target_char_count = target_lower.chars().count();

    if target_char_count == 0 {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len());
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check if a case-insensitive match starts at position i
        let remaining_chars = chars.len() - i;
        if remaining_chars >= target_char_count {
            let candidate: String = chars[i..i + target_char_count]
                .iter()
                .map(|(_, ch)| *ch)
                .collect();
            if candidate.to_lowercase() == target_lower {
                output.push_str(replacement);
                i += target_char_count;
                continue;
            }
        }
        output.push(chars[i].1);
        i += 1;
    }

    output
}

fn match_phrase_with_flexible_spacing_at(
    chars: &[char],
    start: usize,
    segment_lowers: &[String],
    starts_with_word_char: bool,
    ends_with_word_char: bool,
) -> Option<usize> {
    if starts_with_word_char && start > 0 && is_word_char(chars[start - 1]) {
        return None;
    }

    let mut cursor = start;

    for (index, segment_lower) in segment_lowers.iter().enumerate() {
        let segment_len = segment_lower.chars().count();
        if cursor + segment_len > chars.len() {
            return None;
        }

        let candidate: String = chars[cursor..cursor + segment_len].iter().collect();
        if candidate.to_lowercase() != *segment_lower {
            return None;
        }

        cursor += segment_len;

        if index + 1 < segment_lowers.len() {
            while cursor < chars.len() && chars[cursor].is_whitespace() {
                cursor += 1;
            }
        }
    }

    if ends_with_word_char && cursor < chars.len() && is_word_char(chars[cursor]) {
        return None;
    }

    Some(cursor)
}

fn replace_phrase_with_flexible_spacing_case_insensitive(
    text: &str,
    target: &str,
    replacement: &str,
) -> String {
    let segments: Vec<&str> = target.split_whitespace().collect();
    if segments.len() < 2 {
        return replace_literal_phrase_case_insensitive(text, target, replacement);
    }

    let segment_lowers: Vec<String> = segments
        .iter()
        .map(|segment| segment.to_lowercase())
        .collect();
    let starts_with_word_char = segments
        .first()
        .and_then(|segment| segment.chars().next())
        .map(is_word_char)
        .unwrap_or(false);
    let ends_with_word_char = segments
        .last()
        .and_then(|segment| segment.chars().last())
        .map(is_word_char)
        .unwrap_or(false);
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        if let Some(end) = match_phrase_with_flexible_spacing_at(
            &chars,
            i,
            &segment_lowers,
            starts_with_word_char,
            ends_with_word_char,
        ) {
            output.push_str(replacement);
            i = end;
            continue;
        }

        output.push(chars[i]);
        i += 1;
    }

    output
}

fn replace_phrase_case_insensitive(text: &str, target: &str, replacement: &str) -> String {
    if target.split_whitespace().count() >= 2 {
        return replace_phrase_with_flexible_spacing_case_insensitive(text, target, replacement);
    }

    replace_literal_phrase_case_insensitive(text, target, replacement)
}

fn replace_whole_word_case_insensitive(text: &str, target: &str, replacement: &str) -> String {
    let target_lower = target.to_lowercase();
    let mut output = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.peek().copied() {
        if !is_word_char(ch) {
            output.push(ch);
            chars.next();
            continue;
        }

        let mut token = String::new();
        while let Some((_, word_char)) = chars.peek().copied() {
            if !is_word_char(word_char) {
                break;
            }
            token.push(word_char);
            chars.next();
        }

        if token.to_lowercase() == target_lower {
            output.push_str(replacement);
        } else {
            output.push_str(&token);
        }
    }

    output
}

pub(crate) fn finalize_transcript(
    mut text: String,
    replacements: &[settings::Replacement],
) -> String {
    let (phrase_rules, single_word_rules) = sorted_glossary_rules(replacements);

    for rule in phrase_rules {
        text = replace_phrase_case_insensitive(&text, &rule.target, &rule.replacement);
    }

    for rule in single_word_rules {
        text = replace_whole_word_case_insensitive(&text, &rule.target, &rule.replacement);
    }

    if text.ends_with("...") {
        text.truncate(text.len() - 3);
    } else if text.ends_with('\u{2026}') || text.ends_with('.') {
        text.pop();
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Replacement;

    #[test]
    fn test_finalize_transcript_applies_replacements_and_trims_trailing_period() {
        let transcript = finalize_transcript(
            "omg hello.".to_string(),
            &[Replacement {
                target: "omg".to_string(),
                replacement: "Oh my gosh".to_string(),
            }],
        );

        assert_eq!(transcript, "Oh my gosh hello");
    }

    #[test]
    fn test_finalize_transcript_skips_blank_replacement_targets() {
        let transcript = finalize_transcript(
            "hello".to_string(),
            &[Replacement {
                target: "   ".to_string(),
                replacement: "ignored".to_string(),
            }],
        );

        assert_eq!(transcript, "hello");
    }

    #[test]
    fn test_finalize_transcript_replaces_single_words_without_touching_substrings() {
        let transcript = finalize_transcript(
            "partial art party".to_string(),
            &[Replacement {
                target: "art".to_string(),
                replacement: "design".to_string(),
            }],
        );

        assert_eq!(transcript, "partial design party");
    }

    #[test]
    fn test_finalize_transcript_replaces_single_words_case_insensitively() {
        let transcript = finalize_transcript(
            "OMG hello".to_string(),
            &[Replacement {
                target: "omg".to_string(),
                replacement: "Oh my gosh".to_string(),
            }],
        );

        assert_eq!(transcript, "Oh my gosh hello");
    }

    #[test]
    fn test_finalize_transcript_trims_trailing_ellipsis() {
        let transcript = finalize_transcript("hello world...".to_string(), &[]);
        assert_eq!(transcript, "hello world");
    }

    #[test]
    fn test_finalize_transcript_trims_trailing_unicode_ellipsis() {
        let transcript = finalize_transcript("hello world\u{2026}".to_string(), &[]);
        assert_eq!(transcript, "hello world");
    }

    #[test]
    fn test_finalize_transcript_prefers_longer_phrase_rules_before_single_words() {
        let transcript = finalize_transcript(
            "new york is new".to_string(),
            &[
                Replacement {
                    target: "new".to_string(),
                    replacement: "fresh".to_string(),
                },
                Replacement {
                    target: "new york".to_string(),
                    replacement: "NYC".to_string(),
                },
            ],
        );

        assert_eq!(transcript, "NYC is fresh");
    }

    #[test]
    fn test_finalize_transcript_replaces_multi_word_targets_when_dictated_without_spaces() {
        let transcript = finalize_transcript(
            "FemDesign rocks".to_string(),
            &[Replacement {
                target: "FEM Design".to_string(),
                replacement: "FamDesign".to_string(),
            }],
        );

        assert_eq!(transcript, "FamDesign rocks");
    }

    #[test]
    fn test_finalize_transcript_replaces_multi_word_targets_with_different_case() {
        let transcript = finalize_transcript(
            "fem design rocks".to_string(),
            &[Replacement {
                target: "FEM Design".to_string(),
                replacement: "FamDesign".to_string(),
            }],
        );

        assert_eq!(transcript, "FamDesign rocks");
    }

    #[test]
    fn test_finalize_transcript_replaces_multi_word_targets_when_dictated_all_caps_without_spaces()
    {
        let transcript = finalize_transcript(
            "FEMDESIGN rocks".to_string(),
            &[Replacement {
                target: "FEM Design".to_string(),
                replacement: "FamDesign".to_string(),
            }],
        );

        assert_eq!(transcript, "FamDesign rocks");
    }

    #[test]
    fn test_finalize_transcript_does_not_replace_multi_word_target_inside_larger_word() {
        let transcript = finalize_transcript(
            "SuperFemDesignTool".to_string(),
            &[Replacement {
                target: "FEM Design".to_string(),
                replacement: "FamDesign".to_string(),
            }],
        );

        assert_eq!(transcript, "SuperFemDesignTool");
    }

    #[test]
    fn test_finalize_transcript_does_not_cross_punctuation_when_matching_space_insensitive_phrase()
    {
        let transcript = finalize_transcript(
            "Fem-Design rocks".to_string(),
            &[Replacement {
                target: "FEM Design".to_string(),
                replacement: "FamDesign".to_string(),
            }],
        );

        assert_eq!(transcript, "Fem-Design rocks");
    }

    #[test]
    fn test_finalize_transcript_prefers_longer_multi_word_phrase_with_space_insensitive_matching() {
        let transcript = finalize_transcript(
            "FemDesign System".to_string(),
            &[
                Replacement {
                    target: "FEM Design".to_string(),
                    replacement: "FamDesign".to_string(),
                },
                Replacement {
                    target: "FEM Design System".to_string(),
                    replacement: "FamDesignSystem".to_string(),
                },
            ],
        );

        assert_eq!(transcript, "FamDesignSystem");
    }

    #[test]
    fn test_transcription_prompt_is_absent_without_language_specific_instruction() {
        assert_eq!(transcription_prompt("auto", &[]), None);
    }

    #[test]
    fn test_transcription_prompt_in_portuguese_discourages_translation() {
        let prompt = transcription_prompt("pt", &[]).expect("expected prompt");

        assert!(prompt.contains("Não traduzas nem reformules"));
        assert!(prompt.contains("Mantém palavras em inglês"));
        assert!(prompt.contains("português europeu"));
    }

    #[test]
    fn test_transcription_prompt_in_portuguese_preserves_named_entities_and_commands() {
        let prompt = transcription_prompt("pt", &[]).expect("expected prompt");

        assert!(prompt.contains("nomes"));
        assert!(prompt.contains("comandos"));
        assert!(prompt.contains("termos técnicos"));
    }

    #[test]
    fn test_transcription_prompt_includes_built_in_and_glossary_vocabulary() {
        let prompt = transcription_prompt(
            "pt",
            &[
                Replacement {
                    target: "FemVoice".to_string(),
                    replacement: "FamVoice".to_string(),
                },
                Replacement {
                    target: "back end reviewer".to_string(),
                    replacement: "Backend Reviewer".to_string(),
                },
            ],
        )
        .expect("expected prompt");

        assert!(prompt.contains("Vocabulário esperado"));
        assert!(prompt.contains("Groq"));
        assert!(prompt.contains("npm run build"));
        assert!(prompt.contains("src-tauri/src/lib.rs"));
        assert!(prompt.contains("Backend Reviewer"));
        assert_eq!(prompt.matches("FamVoice").count(), 1);
    }
}
