// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

//! Utilities for validating that translation placeholders (`%1`, `%n`, `%s`, `%d`, ...)
//! are preserved when back-filling translations.

/// Extract the placeholders from a string.
///
/// Handles common placeholder styles:
/// - Qt positional: `%1`, `%n`, `%99`
/// - POSIX conversion specifiers: `%s`, `%d`, `%i`, `%f`, `%lld`, ... (with flags, width,
///   precision and length modifiers)
///
/// A literal `%%` is NOT treated as a placeholder.
pub fn extract_placeholders(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != '%' {
            i += 1;
            continue;
        }

        // `%%` is a literal percent sign.
        if i + 1 < bytes.len() && bytes[i + 1] == '%' {
            i += 2;
            continue;
        }

        // Try POSIX conversion: %[flags][width][.precision][length]specifier
        if i + 1 < bytes.len() {
            let mut j = i + 1;
            let mut tok = String::from("%");
            // flags
            while j < bytes.len() && "-+ #0".contains(bytes[j]) {
                tok.push(bytes[j]);
                j += 1;
            }
            // width / precision
            let mut has_num = false;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == '.') {
                has_num = true;
                tok.push(bytes[j]);
                j += 1;
            }
            // length modifier
            let mut has_modifier = false;
            while j < bytes.len() && "hljzqtL".contains(bytes[j]) {
                has_modifier = true;
                tok.push(bytes[j]);
                j += 1;
            }
            // conversion specifier letter => POSIX style placeholder
            if j < bytes.len() && "diuoxXfFeEgGaAcspn".contains(bytes[j]) {
                tok.push(bytes[j]);
                out.push(tok);
                i = j + 1;
                continue;
            }
            // No conversion letter: it is a Qt positional placeholder `%<index>`
            // (e.g. `%1`, `%n`). The token so far is just digits (no modifiers).
            if has_num && !has_modifier {
                // tok currently is `%` followed by digits & possibly dots; keep as-is.
                out.push(tok);
                i = j;
                continue;
            }
            // `%n` Qt plural marker
            if bytes[i + 1] == 'n' {
                out.push("%n".to_string());
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    out
}

/// Check that the multiset of placeholders in `translation` matches that of `source`.
///
/// Both are sorted before comparison so ordering differences (e.g. `%2` before `%1`)
/// are accepted, but dropping, adding or altering a placeholder is rejected.
pub fn placeholders_match(source: &str, translation: &str) -> bool {
    let mut src = extract_placeholders(source);
    let mut tgt = extract_placeholders(translation);
    src.sort();
    tgt.sort();
    src == tgt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tst_qt_positional() {
        assert!(placeholders_match("File %1 not found", "ファイル %1 が見つかりません"));
        assert!(placeholders_match("%1 and %2", "%2 と %1"));
        assert!(!placeholders_match("%1 and %2", "%1 only"));
    }

    #[test]
    fn tst_qt_n_plural() {
        assert!(placeholders_match("%n photos", "%n 枚の写真"));
        assert!(!placeholders_match("%n photos", "no photos"));
    }

    #[test]
    fn tst_posix() {
        assert!(placeholders_match("capacity %d%%", "容量 %d%%"));
        assert!(placeholders_match("%d items, %.2f%%", "%d 個、%.2f%%"));
        assert!(placeholders_match("%lld bytes", "%lld バイト"));
        assert!(!placeholders_match("%d items", "many items"));
    }

    #[test]
    fn tst_no_placeholder() {
        assert!(placeholders_match("plain text", "プレーンテキスト"));
    }
}
