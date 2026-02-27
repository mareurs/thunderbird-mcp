pub fn sanitize_str(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            let n = c as u32;
            // Allow: \t (0x09), \n (0x0a), \r (0x0d), and everything >= 0x20 except DEL (0x7f)
            matches!(n, 0x09 | 0x0a | 0x0d) || (n >= 0x20 && n != 0x7f)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_control_chars() {
        // 0x00-0x08, 0x0b, 0x0c, 0x0e-0x1f, 0x7f should be removed
        let input = "hello\x00world\x07foo\x7fbar";
        assert_eq!(sanitize_str(input), "helloworldfoobar");
    }

    #[test]
    fn preserves_tab_and_newline() {
        let input = "line1\nline2\ttabbed\r\n";
        // These are allowed — they appear in normal email text
        assert!(sanitize_str(input).contains('\n'));
        assert!(sanitize_str(input).contains('\t'));
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
}
