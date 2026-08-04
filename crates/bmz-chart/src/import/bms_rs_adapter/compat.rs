/// beatoraja / jbms-parser が受け付ける、区切り文字のない拡張定義を
/// bms-rs が解釈できる形式へ変換する。
///
/// 一部の譜面では `#BPMxx:value` / `#STOPxx:value` が使われる。jbms-parser
/// は object id の直後から値を読むが、bms-rs はヘッダ名と値の間に空白が
/// 必要なため、パーサーへ渡すテキストだけを正規化する。ハッシュ計算や
/// 生ヘッダ保存に使う原文は変更しない。
pub(super) fn normalize_beatoraja_header_separators(text: &str) -> String {
    let mut rewritten = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some(colon) = legacy_header_colon(line) {
            rewritten.push_str(&line[..colon]);
            rewritten.push(' ');
            rewritten.push_str(&line[colon + 1..]);
        } else {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    rewritten
}

fn legacy_header_colon(line: &str) -> Option<usize> {
    let leading = line.len() - line.trim_start().len();
    let body = &line[leading..];
    let bytes = body.as_bytes();

    if bytes.len() >= 7
        && bytes[0] == b'#'
        && ascii_prefix_ignore_case(&bytes[1..], b"BPM")
        && is_ascii_base36(bytes[4])
        && is_ascii_base36(bytes[5])
        && bytes[6] == b':'
    {
        return Some(leading + 6);
    }

    if bytes.len() >= 8
        && bytes[0] == b'#'
        && ascii_prefix_ignore_case(&bytes[1..], b"STOP")
        && is_ascii_base36(bytes[5])
        && is_ascii_base36(bytes[6])
        && bytes[7] == b':'
    {
        return Some(leading + 7);
    }

    None
}

fn ascii_prefix_ignore_case(value: &[u8], prefix: &[u8]) -> bool {
    value.get(..prefix.len()).is_some_and(|candidate| {
        candidate.iter().zip(prefix).all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
    })
}

fn is_ascii_base36(byte: u8) -> bool {
    byte.is_ascii_digit() || byte.is_ascii_uppercase() || byte.is_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_beatoraja_header_separators;

    #[test]
    fn normalizes_colon_separated_bpm_and_stop_definitions() {
        let source = "#BPM0A:160000160\n#STOP01:8000008.0\n#BPM 160\n#00108:0A\n";

        assert_eq!(
            normalize_beatoraja_header_separators(source),
            "#BPM0A 160000160\n#STOP01 8000008.0\n#BPM 160\n#00108:0A\n"
        );
    }

    #[test]
    fn preserves_leading_whitespace_and_unrelated_colons() {
        let source = "  #bpm01:240\n#TITLE Artist: Song\n#BPM:invalid\n";

        assert_eq!(
            normalize_beatoraja_header_separators(source),
            "  #bpm01 240\n#TITLE Artist: Song\n#BPM:invalid\n"
        );
    }
}
