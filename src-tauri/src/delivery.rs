pub const MAX_DELIVERED_TEXT_CHARS: usize = 10_000;

pub fn validate_text_length(text: &str) -> Result<(), String> {
    let char_count = text.chars().count();
    if char_count > MAX_DELIVERED_TEXT_CHARS {
        return Err(format!(
            "Transcript is too long for automatic delivery ({char_count} characters; limit {MAX_DELIVERED_TEXT_CHARS}). It remains available in History."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_limit_counts_unicode_characters_instead_of_bytes() {
        let unicode = "🦀".repeat(MAX_DELIVERED_TEXT_CHARS);
        assert!(unicode.len() > MAX_DELIVERED_TEXT_CHARS);
        assert!(validate_text_length(&unicode).is_ok());
        assert!(validate_text_length(&(unicode + "é")).is_err());
    }

    #[test]
    fn delivery_limit_accepts_multiline_text_at_boundary() {
        let text = format!("{}\n{}", "a".repeat(4_999), "b".repeat(5_000));
        assert_eq!(text.chars().count(), MAX_DELIVERED_TEXT_CHARS);
        assert!(validate_text_length(&text).is_ok());
    }
}
