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
        text = text.replace(&rule.target, &rule.replacement);
    }

    for rule in single_word_rules {
        text = replace_whole_word_case_insensitive(&text, &rule.target, &rule.replacement);
    }

    if text.ends_with('.') {
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
}
