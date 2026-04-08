use crate::settings;

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
            "Transcreve literalmente no idioma falado, em português europeu correto. Usa acentos e cedilhas quando forem apropriados. Não traduzas nem reformules. Mantém palavras em inglês apenas quando forem ditas em inglês. Mantém nomes próprios, marcas, comandos e termos técnicos exatamente como forem ditos.",
        ),
        _ => None,
    }
}

pub(crate) fn transcription_prompt(language: &str) -> Option<String> {
    transcription_instruction(language).map(str::to_string)
}

fn replace_phrase_case_insensitive(text: &str, target: &str, replacement: &str) -> String {
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
    } else if text.ends_with('\u{2026}') {
        text.pop();
    } else if text.ends_with('.') {
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
    fn test_transcription_prompt_is_absent_without_language_specific_instruction() {
        assert_eq!(transcription_prompt("auto"), None);
    }

    #[test]
    fn test_transcription_prompt_in_portuguese_discourages_translation() {
        let prompt = transcription_prompt("pt").expect("expected prompt");

        assert!(prompt.contains("Não traduzas nem reformules"));
        assert!(prompt.contains("Mantém palavras em inglês"));
        assert!(prompt.contains("Usa acentos e cedilhas"));
        assert!(prompt.contains("português europeu"));
    }

    #[test]
    fn test_transcription_prompt_in_portuguese_preserves_named_entities_and_commands() {
        let prompt = transcription_prompt("pt").expect("expected prompt");

        assert!(prompt.contains("Mantém nomes próprios"));
        assert!(prompt.contains("comandos"));
        assert!(prompt.contains("termos técnicos"));
    }
}
