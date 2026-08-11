use regex::Regex;
use std::sync::OnceLock;

/// Perform deterministic OCR cleaning and repair on raw extracted text prior to semantic chunking.
/// Corrects common neural OCR artifacts, character substitution errors, broken line wraps, and control characters.
pub fn repair_ocr_text(raw_text: &str) -> String {
    if raw_text.trim().is_empty() {
        return String::new();
    }

    // 1. Remove replacement characters and null bytes
    let mut cleaned = raw_text.replace(['\u{fffd}', '\0'], "");

    // 2. Fix hyphenated line breaks (e.g. "disclo-\nsure" -> "disclosure")
    static RE_HYPHEN: OnceLock<Regex> = OnceLock::new();
    let re_hyphen = RE_HYPHEN.get_or_init(|| {
        Regex::new(r"(?i)([a-z]{2,})-\s*[\r\n]+\s*([a-z]{2,})").expect("valid regex")
    });
    cleaned = re_hyphen.replace_all(&cleaned, "$1$2").to_string();

    // 3. Fix common OCR character substitution errors in English words (e.g., "1nformation" -> "information", "0fficial" -> "official")
    static RE_NUM_SUB_START: OnceLock<Regex> = OnceLock::new();
    let re_num_sub_start = RE_NUM_SUB_START.get_or_init(|| {
        Regex::new(r"\b1([a-z]{3,})\b").expect("valid regex")
    });
    cleaned = re_num_sub_start.replace_all(&cleaned, "i$1").to_string();

    static RE_ZERO_SUB_START: OnceLock<Regex> = OnceLock::new();
    let re_zero_sub_start = RE_ZERO_SUB_START.get_or_init(|| {
        Regex::new(r"\b0([a-z]{3,})\b").expect("valid regex")
    });
    cleaned = re_zero_sub_start.replace_all(&cleaned, "o$1").to_string();

    // 4. Normalize excessive newlines and spaces
    static RE_SPACES: OnceLock<Regex> = OnceLock::new();
    let re_spaces = RE_SPACES.get_or_init(|| Regex::new(r"[ \t]+").expect("valid regex"));
    cleaned = re_spaces.replace_all(&cleaned, " ").to_string();

    static RE_NEWLINES: OnceLock<Regex> = OnceLock::new();
    let re_newlines = RE_NEWLINES.get_or_init(|| Regex::new(r"\n{3,}").expect("valid regex"));
    cleaned = re_newlines.replace_all(&cleaned, "\n\n").to_string();

    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repair_ocr_text_hyphenation() {
        let raw = "This is a disclo-\nsure document for review.";
        let repaired = repair_ocr_text(raw);
        assert_eq!(repaired, "This is a disclosure document for review.");
    }

    #[test]
    fn test_repair_ocr_text_char_substitutions() {
        let raw = "1nformation 0fficial disclosure";
        let repaired = repair_ocr_text(raw);
        assert_eq!(repaired, "information official disclosure");
    }
}
