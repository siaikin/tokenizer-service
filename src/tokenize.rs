/// CJK + latin 分词引擎：按 locale 选择分词器 → 小写去重 → 空格拼接。
use std::collections::{BTreeSet, HashMap};

use jieba_rs::Jieba;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use once_cell::sync::Lazy;
use regex::Regex;

static LATIN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{L}\p{N}]+").expect("latin regex"));

pub struct TokenizerEngine {
    ja: Tokenizer,
    ko: Tokenizer,
    zh: Jieba,
}

impl TokenizerEngine {
    pub fn new() -> Result<Self, String> {
        tracing::info!("正在加载嵌入式日文词典（IPADIC）…");
        let ja_dict = load_dictionary("embedded://ipadic")
            .map_err(|e| format!("加载日文词典失败：{e}"))?;
        let ja = Tokenizer::new(Segmenter::new(Mode::Normal, ja_dict, None));

        tracing::info!("正在加载嵌入式韩文词典（ko-dic）…");
        let ko_dict = load_dictionary("embedded://ko-dic")
            .map_err(|e| format!("加载韩文词典失败：{e}"))?;
        let ko = Tokenizer::new(Segmenter::new(Mode::Normal, ko_dict, None));

        tracing::info!("正在初始化中文分词器（jieba）…");
        let zh = Jieba::new();

        tracing::info!("分词引擎已就绪");
        Ok(Self { ja, ko, zh })
    }

    /// 对单个 locale→text map 分词：各 locale 结果小写去重后空格拼接。
    pub fn tokens_from_map(&self, texts: &HashMap<String, String>) -> String {
        let mut seen = BTreeSet::new();
        let mut parts: Vec<String> = Vec::new();

        for (locale, raw) in texts {
            let text = raw.trim();
            if text.is_empty() {
                continue;
            }
            for token in self.tokenize_by_locale(text, locale) {
                let key = token.to_lowercase();
                if seen.insert(key) {
                    parts.push(token);
                }
            }
        }

        parts.join(" ")
    }

    fn tokenize_by_locale(&self, text: &str, locale: &str) -> Vec<String> {
        match locale_kind(locale) {
            Lang::Ja => self.tokenize_ja(text),
            Lang::Ko => self.tokenize_ko(text),
            Lang::Zh => self.tokenize_zh(text),
            Lang::Latin => tokenize_latin(text),
        }
    }

    fn tokenize_ja(&self, text: &str) -> Vec<String> {
        match self.ja.tokenize(text) {
            Ok(tokens) => tokens
                .into_iter()
                .map(|t| t.surface.to_string())
                .filter(|t| !t.trim().is_empty())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "日文分词失败，回退 latin");
                tokenize_latin(text)
            }
        }
    }

    fn tokenize_ko(&self, text: &str) -> Vec<String> {
        match self.ko.tokenize(text) {
            Ok(tokens) => tokens
                .into_iter()
                .map(|t| t.surface.to_string())
                .filter(|t| !t.trim().is_empty())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "韩文分词失败，回退 latin");
                tokenize_latin(text)
            }
        }
    }

    fn tokenize_zh(&self, text: &str) -> Vec<String> {
        self.zh
            .cut(text, false)
            .into_iter()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect()
    }
}

enum Lang {
    Ja,
    Ko,
    Zh,
    Latin,
}

fn locale_kind(locale: &str) -> Lang {
    let lc = locale.to_ascii_lowercase();
    if lc == "ja" || lc.starts_with("ja-") {
        Lang::Ja
    } else if lc == "ko" || lc.starts_with("ko-") {
        Lang::Ko
    } else if lc.starts_with("zh") || lc == "cmn" {
        Lang::Zh
    } else {
        Lang::Latin
    }
}

fn tokenize_latin(text: &str) -> Vec<String> {
    LATIN_RE
        .split(&text.to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn summarize_texts(texts: &[HashMap<String, String>]) -> (usize, String, i64) {
    let mut langs = BTreeSet::new();
    let mut chars: i64 = 0;
    let mut count = 0usize;

    for lt in texts {
        for (locale, raw) in lt {
            let text = raw.trim();
            if text.is_empty() {
                continue;
            }
            count += 1;
            chars += text.chars().count() as i64;
            langs.insert(match locale_kind(locale) {
                Lang::Ja => "ja",
                Lang::Ko => "ko",
                Lang::Zh => "zh",
                Lang::Latin => "latin",
            });
        }
    }

    (count, langs.into_iter().collect::<Vec<_>>().join(","), chars)
}
