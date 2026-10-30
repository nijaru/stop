//! Lossless wrapping of escaped detail values. JSON remains the copyable argv form.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::safe;

fn append_value(out: &mut String, prefix: &str, value: &str, width: usize) {
    let prefix_width = prefix.width();
    let indent = " ".repeat(prefix_width);
    let available = width.saturating_sub(prefix_width).max(1);
    let printable_ascii = value.bytes().all(|byte| (b' '..=b'~').contains(&byte));
    out.push_str(prefix);
    let mut remaining = value;
    while !remaining.is_empty() {
        let mut end = 0;
        let mut boundary = 0;
        if printable_ascii {
            end = remaining.len().min(available);
            if end < remaining.len() {
                boundary = remaining.as_bytes()[..end]
                    .iter()
                    .rposition(|&byte| byte == b'/' || byte == b' ')
                    .map_or(0, |index| index + 1);
            }
        } else {
            // Width can depend on context across graphemes (e.g. Lam-Alef).
            for (index, grapheme) in remaining.grapheme_indices(true) {
                let next = index + grapheme.len();
                // unicode-width's rules bound display width by UTF-8 length.
                // Only measure a contextual prefix when it could exceed the line.
                if index > 0 && next > available && remaining[..next].width() > available {
                    break;
                }
                end = next;
                if grapheme == "/" || grapheme.chars().all(char::is_whitespace) {
                    boundary = end;
                }
            }
        }
        if end < remaining.len() && boundary > 0 {
            end = boundary;
        }
        out.push_str(&remaining[..end]);
        remaining = &remaining[end..];
        if !remaining.is_empty() {
            out.push('\n');
            out.push_str(&indent);
        }
    }
    out.push('\n');
}

pub(super) fn field(out: &mut String, label: &str, value: &str, width: usize) {
    append_value(out, &format!("{label:<12} "), value, width);
}

pub(super) fn arguments(out: &mut String, argv: &[String], width: usize) {
    let quoted: Vec<_> = argv.iter().map(|arg| safe(&format!("{arg:?}"))).collect();
    // Quoted arguments are separated by ASCII spaces, so their widths add
    // without carrying Unicode shaping context across argument boundaries.
    let inline_width =
        quoted.iter().map(|arg| arg.width()).sum::<usize>() + quoted.len().saturating_sub(1);
    if 13 + inline_width <= width {
        field(out, "Arguments", &quoted.join(" "), width);
    } else {
        out.push_str("Arguments\n");
        for (index, arg) in quoted.iter().enumerate() {
            append_value(out, &format!("  [{index}] "), arg, width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Original contextual-width algorithm, kept as a differential oracle.
    fn reference_append(out: &mut String, prefix: &str, value: &str, width: usize) {
        let indent = " ".repeat(prefix.width());
        let available = width.saturating_sub(prefix.width()).max(1);
        out.push_str(prefix);
        let mut remaining = value;
        while !remaining.is_empty() {
            let mut end = 0;
            let mut boundary = 0;
            for (index, grapheme) in remaining.grapheme_indices(true) {
                let next = index + grapheme.len();
                if index > 0 && remaining[..next].width() > available {
                    break;
                }
                end = next;
                if grapheme == "/" || grapheme.chars().all(char::is_whitespace) {
                    boundary = end;
                }
            }
            if end < remaining.len() && boundary > 0 {
                end = boundary;
            }
            out.push_str(&remaining[..end]);
            remaining = &remaining[end..];
            if !remaining.is_empty() {
                out.push('\n');
                out.push_str(&indent);
            }
        }
        out.push('\n');
    }

    fn check_wrapping(value: &str) {
        for prefix in ["", "  [0] ", "  [100] ", "日本 ", "e\u{301} "] {
            for width in (0..=24).chain([100, value.len() + 13, usize::MAX]) {
                let mut expected = String::from("before\n");
                reference_append(&mut expected, prefix, value, width);
                let mut actual = String::from("before\n");
                append_value(&mut actual, prefix, value, width);
                assert_eq!(actual, expected, "{prefix:?}, {value:?}, width {width}");
            }
        }
    }

    #[test]
    fn ascii_wrapping_matches_original_at_boundaries() {
        // Exhaust short combinations to cover competing slash/space breaks,
        // leading/trailing whitespace, empty values and one-cell lines.
        for len in 0..=6 {
            for mut encoded in 0..3usize.pow(len) {
                let value: String = (0..len)
                    .map(|_| {
                        let byte = b"a/ "[encoded % 3];
                        encoded /= 3;
                        char::from(byte)
                    })
                    .collect();
                check_wrapping(&value);
            }
        }
        let printable: String = (b' '..=b'~').map(char::from).collect();
        check_wrapping(&printable.repeat(3));
    }

    #[test]
    fn unicode_table_preserves_the_byte_width_bound() {
        // Protect the fast path against Unicode-table changes, including
        // unusually wide characters such as the three-cell Khmer BEYYAL.
        let mut bytes = [0; 4];
        for codepoint in 0..=0x10ffff {
            if let Some(ch) = char::from_u32(codepoint) {
                let value = ch.encode_utf8(&mut bytes);
                assert!(value.width() <= value.len(), "U+{codepoint:04X}");
            }
        }
    }

    #[test]
    fn unicode_and_control_fallback_preserves_contextual_width() {
        for value in [
            "لاx",
            "لالا / لا x",
            "日本語/👩‍💻/❤️/  padded  ",
            "e\u{301}/\u{301} a\u{a0}b\u{2003}c",
            "a\t b\r\nc\x1b\x7f",
            "\u{17d8}\u{17d8}\u{17d8}/\u{16d67}\u{16d67}\u{16d68}",
        ] {
            check_wrapping(value);
        }
        let mut out = String::new();
        append_value(&mut out, "", "لاx", 1);
        assert_eq!(out, "لا\nx\n");
    }

    #[test]
    fn inline_decision_matches_joined_width_and_indexed_output() {
        for argv in [
            vec![],
            vec![String::new()],
            vec!["node".into(), "two words".into(), "".into()],
            vec!["لا".into(), "👩‍💻/❤️/e\u{301}".into(), "日本".into()],
            vec!["\t\n\x1b\u{202e}\\\" / ".repeat(8)],
            vec![String::new(); 101],
        ] {
            let quoted: Vec<_> = argv.iter().map(|arg| safe(&format!("{arg:?}"))).collect();
            let inline = quoted.join(" ");
            for width in (0..=100).chain([13 + inline.width(), 14 + inline.width()]) {
                let mut expected = String::new();
                if 13 + inline.width() <= width {
                    reference_append(&mut expected, "Arguments    ", &inline, width);
                } else {
                    expected.push_str("Arguments\n");
                    for (index, arg) in quoted.iter().enumerate() {
                        reference_append(&mut expected, &format!("  [{index}] "), arg, width);
                    }
                }
                let mut actual = String::new();
                arguments(&mut actual, &argv, width);
                assert_eq!(actual, expected, "{argv:?}, width {width}");
            }
        }
    }

    #[test]
    fn wrapped_values_preserve_whitespace_and_graphemes() {
        let value = "long path/日本語/👩‍💻/❤️/  padded  ".repeat(3);
        let mut out = String::new();
        field(&mut out, "Directory", &value, 32);
        assert!(out.lines().all(|line| line.width() <= 32), "{out}");
        let reconstructed: String = out.lines().map(|line| &line[13..]).collect();
        assert_eq!(reconstructed, value);
        assert!(!out.contains("👩\n") && !out.contains("❤\n"));
    }

    #[test]
    fn wrapping_prefers_word_boundaries_without_dropping_spaces() {
        let mut out = String::new();
        field(&mut out, "Directory", "alpha beta gamma delta", 21);
        let values: Vec<_> = out.lines().map(|line| &line[13..]).collect();
        assert_eq!(values, ["alpha ", "beta ", "gamma ", "delta"]);
    }

    #[test]
    fn short_arguments_stay_inline_and_long_ones_have_explicit_boundaries() {
        let short = ["node".into(), "two words".into(), "".into()];
        let mut out = String::new();
        arguments(&mut out, &short, 80);
        assert_eq!(out, "Arguments    \"node\" \"two words\" \"\"\n");

        let argv = [
            "node".into(),
            "".into(),
            " \t padded \n ".repeat(8),
            "λ".into(),
        ];
        out.clear();
        arguments(&mut out, &argv, 32);
        assert!(out.lines().all(|line| line.width() <= 32), "{out}");
        assert!(out.starts_with("Arguments\n  [0] \"node\"\n  [1] \"\"\n  [2] "));
        let mut reconstructed = Vec::new();
        for line in out.lines().skip(1) {
            if line.starts_with("  [") {
                reconstructed.push(line[6..].to_owned());
            } else {
                reconstructed.last_mut().unwrap().push_str(&line[6..]);
            }
        }
        assert_eq!(
            reconstructed,
            argv.iter()
                .map(|arg| safe(&format!("{arg:?}")))
                .collect::<Vec<_>>()
        );
        assert!(!out.contains('\t') && !out.contains('\x1b'));
    }
}
