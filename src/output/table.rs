//! Display-cell layout shared by process, port, and candidate tables.

use owo_colors::OwoColorize;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn truncate(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.into();
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    for grapheme in value.graphemes(true) {
        let end = result.len() + grapheme.len();
        if value[..end].width() > width - 1 {
            break;
        }
        result.push_str(grapheme);
    }
    result.push('…');
    result
}

fn pad(value: &str, width: usize, right: bool) -> String {
    let space = " ".repeat(width.saturating_sub(value.width()));
    if right {
        format!("{space}{value}")
    } else {
        format!("{value}{space}")
    }
}

/// Flexible columns have (index, maximum width), in shrinking priority order.
/// Numeric columns and addresses stay intact even if the terminal is too small.
pub(super) fn render(
    headers: &[&str],
    rows: Vec<Vec<String>>,
    right: &[usize],
    flexible: &[(usize, usize)],
    width: usize,
    color: bool,
) -> String {
    let mut widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            rows.iter()
                .map(|row| row[i].width())
                .max()
                .unwrap_or(0)
                .max(header.width())
        })
        .collect();
    for &(i, max) in flexible {
        widths[i] = widths[i].min(max);
    }
    let mut excess = (widths.iter().sum::<usize>() + 2 * (headers.len() - 1)).saturating_sub(width);
    for &(i, _) in flexible {
        let reduction = excess.min(widths[i].saturating_sub(1));
        widths[i] -= reduction;
        excess -= reduction;
    }
    let render_row = |row: &[String]| {
        row.iter()
            .enumerate()
            .map(|(i, value)| pad(&truncate(value, widths[i]), widths[i], right.contains(&i)))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_owned()
    };
    let heading = render_row(&headers.iter().map(|s| (*s).into()).collect::<Vec<_>>());
    let mut out = if color {
        format!("{}\n", heading.bold())
    } else {
        format!("{heading}\n")
    };
    for row in rows {
        out.push_str(&render_row(&row));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_and_alignment_use_display_cells() {
        for (input, width, expected) in [
            ("日本語abcd", 6, "日本…"),
            ("❤️❤️abcd", 4, "❤️…"),
            ("👩‍💻abcd", 3, "👩‍💻…"),
            ("e\u{301}abcd", 2, "e\u{301}…"),
            ("abc", 0, ""),
        ] {
            let result = truncate(input, width);
            assert_eq!(result, expected);
            assert!(result.width() <= width);
        }
        assert_eq!(pad("日本", 6, false), "日本  ");
        assert_eq!(pad("日本", 6, true), "  日本");
    }

    #[test]
    fn narrow_tables_shrink_names_and_owners_before_numbers() {
        let out = render(
            &["PID", "USER", "NAME", "CPU%", "RSS"],
            vec![vec![
                "12345".into(),
                "long_username".into(),
                "❤️❤️abcd".into(),
                "123.4".into(),
                "123.4 MiB".into(),
            ]],
            &[0, 3, 4],
            &[(2, usize::MAX), (1, 16)],
            32,
            false,
        );
        assert!(out.lines().all(|line| line.width() <= 32), "{out}");
        assert!(out.contains("12345") && out.contains("123.4 MiB"));
    }
}
