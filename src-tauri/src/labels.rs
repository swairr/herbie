//! #tag 解析,Rust 实现对齐 `src/shared/labels.ts`。
//! - 写路径(入库时由 todos 仓储调用)用本模块;
//! - 读/单元测试用 TS(`tests/labels.test.ts`);
//! - 两侧共享 `tests/fixtures/labels.json` 夹具对拍防漂移(计划第5/68行)。

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

static URL_RE: OnceLock<Regex> = OnceLock::new();
static LABEL_RE: OnceLock<Regex> = OnceLock::new();

fn url_re() -> &'static Regex {
    // 不用 `\b`(Rust regex 的 `\b` 是 Unicode 词边界,JS 是 ASCII,会漂移;且 crate
    // 不支持 lookbehind)。等价 ASCII 边界的判定放在 parse_labels 里做:URL 前一字节
    // 若为 [A-Za-z0-9_] 则视为非独立 URL(如 "ahttps://x")。
    URL_RE.get_or_init(|| Regex::new(r"https?://[^\s]+").expect("invalid url regex"))
}

fn label_re() -> &'static Regex {
    LABEL_RE.get_or_init(|| {
        Regex::new(r"#([\p{L}\p{N}_-]{1,60})").expect("invalid label regex")
    })
}

/// 解析 detail 文本中的 #tag,逐行翻译自 `src/shared/labels.ts` 的 `parseLabels`:
/// - 空串返空数组
/// - 先用 URL 正则找出所有 URL 区间 `[start,end)`
/// - 再用 label 正则逐个匹配;若整段匹配区间落任一 URL 区间内则跳过
/// - 取捕获组 1(去掉 `#`),按首次出现顺序去重后返回
pub fn parse_labels(detail: &str) -> Vec<String> {
    if detail.is_empty() {
        return Vec::new();
    }
    let ranges: Vec<(usize, usize)> = url_re()
        .find_iter(detail)
        .filter(|m| {
            // 等价 JS `\b`(ASCII 词边界):URL 前一字节若为 [A-Za-z0-9_] 则不是独立 URL
            //(JS 中 "ahttps://x" 的 `\b` 不成立);中文/空格/标点等前一字符则成立。
            let prev = m
                .start()
                .checked_sub(1)
                .and_then(|i| detail.as_bytes().get(i));
            !matches!(
                prev,
                Some(b) if b.is_ascii_alphanumeric() || *b == b'_'
            )
        })
        .map(|m| (m.start(), m.end()))
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for caps in label_re().captures_iter(detail) {
        let full = match caps.get(0) {
            Some(f) => f,
            None => continue,
        };
        let start = full.start();
        let end = full.end();
        if ranges.iter().any(|(s, e)| start >= *s && end <= *e) {
            continue;
        }
        if let Some(g) = caps.get(1) {
            let label = g.as_str().to_string();
            if seen.insert(label.clone()) {
                out.push(label);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unicode_labels_from_detail() {
        assert_eq!(
            parse_labels("今天 #工作 很忙 #项目-x"),
            vec!["工作".to_string(), "项目-x".to_string()]
        );
    }

    #[test]
    fn skips_hash_inside_urls() {
        assert_eq!(
            parse_labels("见 https://example.com/page#section 和 #工作"),
            vec!["工作".to_string()]
        );
    }

    #[test]
    fn skips_hash_that_is_part_of_an_http_url_token() {
        assert_eq!(
            parse_labels("https://git.io/abc#def #todo"),
            vec!["todo".to_string()]
        );
    }

    #[test]
    fn skips_url_hash_when_url_is_adjacent_to_cjk_text() {
        // JS `\b` 是 ASCII 词边界:紧贴中文的 URL 也应被识别,片段 #tag 不得入库。
        assert_eq!(parse_labels("查https://example.com/p#sec"), Vec::<String>::new());
        assert_eq!(
            parse_labels("中文https://a.io/x#y 和 #keep"),
            vec!["keep".to_string()]
        );
    }

    #[test]
    fn dedupes_same_named_labels() {
        assert_eq!(
            parse_labels("#a #a #b"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn is_case_sensitive() {
        assert_eq!(
            parse_labels("#Work #WORK"),
            vec!["Work".to_string(), "WORK".to_string()]
        );
    }

    #[test]
    fn returns_empty_for_empty_detail() {
        assert!(parse_labels("").is_empty());
    }

    #[test]
    fn only_matches_valid_label_chars_1_to_60() {
        assert!(parse_labels("#").is_empty());
        assert_eq!(parse_labels("#a"), vec!["a".to_string()]);
    }

    #[test]
    fn does_not_treat_plain_words_as_labels() {
        assert!(parse_labels("just some text no tags").is_empty());
    }

    #[test]
    fn handles_labels_adjacent_to_punctuation() {
        assert_eq!(
            parse_labels("do #work, then #home."),
            vec!["work".to_string(), "home".to_string()]
        );
    }

    #[test]
    fn ignores_hash_not_followed_by_valid_label_char() {
        assert!(parse_labels("#! and # work").is_empty());
    }

    #[test]
    fn parses_labels_across_multiple_lines() {
        assert_eq!(
            parse_labels("line1 #a\nline2 #b"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn skips_multiple_urls_and_labels_outside_them() {
        assert_eq!(
            parse_labels("https://a.io/x#y https://b.io/z#w #keep"),
            vec!["keep".to_string()]
        );
    }

    #[test]
    fn does_not_capture_hash_inside_a_url_with_trailing_label() {
        assert_eq!(parse_labels("https://a.io#x#y #ok"), vec!["ok".to_string()]);
    }

    #[test]
    fn preserves_unicode_and_underscore_dash_combos() {
        assert_eq!(
            parse_labels("#项目_1 #A-B #测试-2"),
            vec![
                "项目_1".to_string(),
                "A-B".to_string(),
                "测试-2".to_string()
            ]
        );
    }

    #[derive(serde::Deserialize)]
    struct FixtureCase {
        input: String,
        expected: Vec<String>,
    }

    /// 共享 JSON 夹具对拍:读取与 TS `tests/labels-fixture.test.ts` 同一份
    /// `tests/fixtures/labels.json`,逐条断言 `parse_labels(input)==expected`。
    /// 任一侧改动解析逻辑都必须同步该夹具,否则本测试与 TS 夹具测试互漂移报警。
    #[test]
    fn fixture_parity_with_ts() {
        let raw = include_str!("../../tests/fixtures/labels.json");
        let cases: Vec<FixtureCase> =
            serde_json::from_str(raw).expect("labels.json must parse as array of {input,expected}");
        for c in &cases {
            assert_eq!(parse_labels(&c.input), c.expected, "input: {:?}", c.input);
        }
    }
}