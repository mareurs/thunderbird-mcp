/// Sanitize a JSON response that may contain raw control characters inside string values.
///
/// Tracks whether we're inside a JSON string (between unescaped `"`).
/// - Inside strings: escape control chars (0x00-0x1F, 0x7F) as `\uXXXX`
/// - Outside strings: strip control chars except \t, \n, \r (valid JSON whitespace)
pub fn sanitize_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut prev_backslash = false;

    for c in s.chars() {
        if in_string {
            if c == '"' && !prev_backslash {
                in_string = false;
                out.push(c);
            } else {
                let n = c as u32;
                if n <= 0x1F || n == 0x7F {
                    // Escape control chars as \uXXXX inside JSON strings
                    out.push_str(&format!("\\u{:04x}", n));
                } else {
                    out.push(c);
                }
            }
            prev_backslash = !prev_backslash && c == '\\';
        } else if c == '"' {
            in_string = true;
            out.push(c);
            prev_backslash = false;
        } else {
            let n = c as u32;
            // Outside strings: keep valid JSON whitespace and printable chars
            if matches!(n, 0x09 | 0x0a | 0x0d) || (n >= 0x20 && n != 0x7f) {
                out.push(c);
            }
            // else: drop control chars outside strings
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Outside strings: control chars are stripped ---

    #[test]
    fn strips_control_chars_outside_strings() {
        let input = "hello\x00world\x07foo\x7fbar";
        assert_eq!(sanitize_str(input), "helloworldfoobar");
    }

    #[test]
    fn preserves_whitespace_outside_strings() {
        let input = "{\n\t\"key\": \"val\"\r\n}";
        assert_eq!(sanitize_str(input), "{\n\t\"key\": \"val\"\r\n}");
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(sanitize_str(""), "");
    }

    #[test]
    fn preserves_unicode() {
        let input = "Héllo wörld 日本語";
        assert_eq!(sanitize_str(input), input);
    }

    #[test]
    fn handles_only_control_chars() {
        let input = "\x00\x01\x02\x03";
        assert_eq!(sanitize_str(input), "");
    }

    #[test]
    fn strips_vt_and_ff_outside_strings() {
        assert_eq!(sanitize_str("foo\x0bbar"), "foobar");
        assert_eq!(sanitize_str("foo\x0cbar"), "foobar");
    }

    // --- Inside strings: control chars are escaped as \uXXXX ---

    #[test]
    fn escapes_newline_inside_json_string() {
        // A literal newline inside a JSON string must become \u000a
        let input = "{\"body\": \"line1\nline2\"}";
        let result = sanitize_str(input);
        assert_eq!(result, "{\"body\": \"line1\\u000aline2\"}");
        // The result must be valid JSON
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn escapes_tab_inside_json_string() {
        let input = "{\"body\": \"col1\tcol2\"}";
        let result = sanitize_str(input);
        assert_eq!(result, "{\"body\": \"col1\\u0009col2\"}");
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn escapes_null_inside_json_string() {
        let input = "{\"body\": \"hel\x00lo\"}";
        let result = sanitize_str(input);
        assert_eq!(result, "{\"body\": \"hel\\u0000lo\"}");
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn escapes_cr_inside_json_string() {
        let input = "{\"body\": \"foo\rbar\"}";
        let result = sanitize_str(input);
        assert_eq!(result, "{\"body\": \"foo\\u000dbar\"}");
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn handles_escaped_quote_inside_string() {
        // \" inside a JSON string should not end the string
        let input = r#"{"body": "say \"hello\" world"}"#;
        let result = sanitize_str(input);
        assert_eq!(result, input);
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn handles_backslash_before_quote() {
        // \\" means escaped backslash then real closing quote
        let input = r#"{"path": "C:\\"}"#;
        let result = sanitize_str(input);
        assert_eq!(result, input);
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn real_world_email_body_with_mixed_control_chars() {
        let input = "{\"subject\": \"Re: Hello\", \"body\": \"Hi,\r\nThanks!\t- Sent from\x00phone\"}";
        let result = sanitize_str(input);
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["subject"].as_str().unwrap(), "Re: Hello");
    }

    #[test]
    fn boundary_0x1f_escaped_inside_string_0x20_kept() {
        let input = "{\"x\": \"\x1f \"}";
        let result = sanitize_str(input);
        assert_eq!(result, "{\"x\": \"\\u001f \"}");
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }
}
