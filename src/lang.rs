pub fn detect(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    whatlang::detect_lang(text).map(|lang| lang.code().to_string())
}

pub fn needs_translation(text: &str, target_lang: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    match detect(text) {
        Some(lang) => normalize_code(&lang) != resolve_lang_code(target_lang),
        None => false,
    }
}

fn normalize_code(code: &str) -> &str {
    let base = code.split(['-', '_']).next().unwrap_or(code);
    match base {
        "cmn" | "zh" => "zho",
        "eng" | "en" => "eng",
        "jpn" | "ja" => "jpn",
        "kor" | "ko" => "kor",
        "fra" | "fr" => "fra",
        "deu" | "de" => "deu",
        "spa" | "es" => "spa",
        "rus" | "ru" => "rus",
        "ara" | "ar" => "ara",
        "por" | "pt" => "por",
        "ita" | "it" => "ita",
        other => other,
    }
}

/// Map free-form language names to ISO 639-3 codes for comparison.
fn resolve_lang_code(code: &str) -> &str {
    let normalized = normalize_code(code);
    if normalized != code {
        return normalized;
    }
    match code {
        "简体中文" | "繁體中文" | "中文" | "Chinese" | "Mandarin" => "zho",
        "English" | "英文" => "eng",
        "日本語" | "Japanese" | "日文" => "jpn",
        "한국어" | "Korean" | "韩文" => "kor",
        "Français" | "French" | "法文" => "fra",
        "Deutsch" | "German" | "德文" => "deu",
        "Español" | "Spanish" | "西班牙文" => "spa",
        "Русский" | "Russian" | "俄文" => "rus",
        "العربية" | "Arabic" | "阿拉伯文" => "ara",
        "Português" | "Portuguese" | "葡萄牙文" => "por",
        "Italiano" | "Italian" | "意大利文" => "ita",
        other => normalize_code(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chinese() {
        let lang = detect("这是一段中文文本，用于测试语言检测功能");
        assert!(lang.is_some());
        let code = lang.unwrap().split('-').next().unwrap().to_string();
        assert!(code == "cmn" || code == "zho");
    }

    #[test]
    fn test_detect_english() {
        let lang = detect("This is an English sentence for testing language detection");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().split('-').next().unwrap(), "eng");
    }

    #[test]
    fn test_needs_translation_chinese_to_english() {
        let cn_text = "这是中文";
        assert!(needs_translation(cn_text, "eng"));
    }

    #[test]
    fn test_no_translation_needed() {
        let en_text = "This is English text that should not need translation";
        assert!(!needs_translation(en_text, "eng"));
    }
}
