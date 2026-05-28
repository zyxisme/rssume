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
        Some(lang) => normalize_code(&lang) != normalize_code(target_lang),
        None => false,
    }
}

fn normalize_code(code: &str) -> &str {
    let base = code.split('-').next().unwrap_or(code);
    match base {
        "cmn" => "zho",
        other => other,
    }
}

pub fn lang_name(code: &str) -> String {
    let code = code.split('-').next().unwrap_or(code);
    match code {
        "zho" | "zh" | "cmn" => "Chinese",
        "eng" | "en" => "English",
        "jpn" | "ja" => "Japanese",
        "kor" | "ko" => "Korean",
        "fra" | "fr" => "French",
        "deu" | "de" => "German",
        "spa" | "es" => "Spanish",
        "rus" | "ru" => "Russian",
        "ara" | "ar" => "Arabic",
        "por" | "pt" => "Portuguese",
        "ita" | "it" => "Italian",
        _ => code,
    }
    .to_string()
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
