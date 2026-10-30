//! Lossless wrapping of escaped detail values. JSON remains the copyable argv form.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::safe;

fn append_value(out: &mut String, prefix: &str, value: &str, width: usize) {
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

pub(super) fn field(out: &mut String, label: &str, value: &str, width: usize) {
    append_value(out, &format!("{label:<12} "), value, width);
}

pub(super) fn arguments(out: &mut String, argv: &[String], width: usize) {
    let quoted: Vec<_> = argv.iter().map(|arg| safe(&format!("{arg:?}"))).collect();
    let inline = quoted.join(" ");
    if 13 + inline.width() <= width {
        field(out, "Arguments", &inline, width);
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
