// --- HTML/URL helpers ---

pub fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

pub fn extract_between<'a>(html: &'a str, open_tag: &str, close_tag: &str) -> Option<&'a str> {
    let start = html.to_lowercase().find(&open_tag.to_lowercase())?;
    let rest = &html[start + open_tag.len()..];
    let end = rest.to_lowercase().find(&close_tag.to_lowercase())?;
    Some(&rest[..end])
}

pub fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let lower = html.to_lowercase();
    let mut result = String::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find(&open) {
        result.push_str(&html[pos..pos + start]);
        let after = pos + start;
        if let Some(end) = lower[after..].find(&close) {
            pos = after + end + close.len();
        } else {
            pos = html.len();
            break;
        }
    }
    result.push_str(&html[pos..]);
    result
}

pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
            result.push(' ');
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Decode common HTML entities
    result.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- urlencoding tests ---

    #[test]
    fn urlencoding_alphanumeric_passthrough() {
        assert_eq!(urlencoding("abc123"), "abc123");
        assert_eq!(urlencoding("AZaz09"), "AZaz09");
    }

    #[test]
    fn urlencoding_unreserved_chars() {
        assert_eq!(urlencoding("-_.~"), "-_.~");
    }

    #[test]
    fn urlencoding_spaces_become_plus() {
        assert_eq!(urlencoding("hello world"), "hello+world");
        assert_eq!(urlencoding("a b c"), "a+b+c");
    }

    #[test]
    fn urlencoding_special_chars() {
        assert_eq!(urlencoding("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlencoding("foo@bar"), "foo%40bar");
        assert_eq!(urlencoding("100%"), "100%25");
    }

    #[test]
    fn urlencoding_cjk_characters() {
        // CJK chars are multi-byte UTF-8, each byte gets percent-encoded
        let encoded = urlencoding("test");
        assert_eq!(encoded, "test");

        let encoded_zh = urlencoding("hello world");
        // Should not contain raw CJK chars
        assert!(!encoded_zh.contains("hello") || encoded_zh == "hello+world");

        // Specific test: Chinese char is 3 bytes in UTF-8
        let result = urlencoding("\u{4F60}"); // character ni3
        assert!(result.starts_with('%'));
        assert_eq!(result.matches('%').count(), 3); // 3 bytes = 3 percent-encoded sequences
    }

    #[test]
    fn urlencoding_empty_string() {
        assert_eq!(urlencoding(""), "");
    }

    // --- extract_between tests ---

    #[test]
    fn extract_between_basic() {
        let html = "<title>Hello World</title>";
        assert_eq!(extract_between(html, "<title>", "</title>"), Some("Hello World"));
    }

    #[test]
    fn extract_between_case_insensitive() {
        let html = "<TITLE>Test</Title>";
        assert_eq!(extract_between(html, "<title>", "</title>"), Some("Test"));
    }

    #[test]
    fn extract_between_no_match() {
        let html = "<p>no title here</p>";
        assert_eq!(extract_between(html, "<title>", "</title>"), None);
    }

    #[test]
    fn extract_between_no_close_tag() {
        let html = "<title>unclosed";
        assert_eq!(extract_between(html, "<title>", "</title>"), None);
    }

    // --- strip_tag_blocks tests ---

    #[test]
    fn strip_tag_blocks_removes_script() {
        let html = "before<script>alert(1)</script>after";
        assert_eq!(strip_tag_blocks(html, "script"), "beforeafter");
    }

    #[test]
    fn strip_tag_blocks_multiple() {
        let html = "a<style>css</style>b<style>more</style>c";
        assert_eq!(strip_tag_blocks(html, "style"), "abc");
    }

    #[test]
    fn strip_tag_blocks_no_match() {
        let html = "<p>hello</p>";
        assert_eq!(strip_tag_blocks(html, "script"), "<p>hello</p>");
    }

    #[test]
    fn strip_tag_blocks_unclosed() {
        let html = "before<script>no close tag";
        assert_eq!(strip_tag_blocks(html, "script"), "before");
    }

    // --- strip_html_tags tests ---

    #[test]
    fn strip_html_tags_basic() {
        let result = strip_html_tags("<p>Hello</p>");
        assert!(result.contains("Hello"));
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
    }

    #[test]
    fn strip_html_tags_preserves_text() {
        let result = strip_html_tags("plain text");
        assert_eq!(result, "plain text");
    }

    #[test]
    fn strip_html_tags_nested() {
        let result = strip_html_tags("<div><p><b>bold</b> text</p></div>");
        assert!(result.contains("bold"));
        assert!(result.contains("text"));
        assert!(!result.contains('<'));
    }

    #[test]
    fn strip_html_tags_empty_input() {
        let result = strip_html_tags("");
        assert_eq!(result, "");
    }

    #[test]
    fn strip_html_tags_no_tags() {
        let result = strip_html_tags("just plain text here");
        assert_eq!(result, "just plain text here");
    }

    #[test]
    fn strip_html_tags_decodes_entities() {
        let result = strip_html_tags("a &amp; b &lt; c &gt; d &quot;e&quot; f&#39;g");
        assert!(result.contains("a & b < c > d \"e\" f'g"));
    }

    #[test]
    fn strip_html_tags_nbsp() {
        let result = strip_html_tags("hello&nbsp;world");
        assert!(result.contains("hello world"));
    }

    #[test]
    fn strip_html_tags_self_closing() {
        let result = strip_html_tags("line1<br/>line2<hr/>end");
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        assert!(result.contains("end"));
    }
}
