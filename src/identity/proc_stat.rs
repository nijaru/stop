//! Linux stat records, shared by birth validation and process acquisition.

use std::fs::File;
use std::io::{self, Read};

pub(crate) struct Stat<'a> {
    pub name: &'a [u8],
    fields: [Option<&'a [u8]>; 22],
}

impl<'a> Stat<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        // comm can contain whitespace, parentheses and non-UTF8 bytes. Only
        // the final ')' delimits it; never decode the entire record as UTF-8.
        let open = bytes.iter().position(|&b| b == b'(')?;
        let close = bytes.iter().rposition(|&b| b == b')')?;
        if open == 0 || close < open || bytes.get(close + 1) != Some(&b' ') {
            return None;
        }
        let mut fields = bytes[close + 2..]
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty());
        Some(Self {
            name: &bytes[open + 1..close],
            fields: std::array::from_fn(|_| fields.next()),
        })
    }

    pub fn number<T: std::str::FromStr>(&self, field: usize) -> Option<T> {
        std::str::from_utf8(self.fields.get(field.checked_sub(3)?)?.as_ref()?)
            .ok()?
            .parse()
            .ok()
    }

    pub fn state(&self) -> Option<char> {
        let bytes = self.fields[0]?;
        (bytes.len() == 1).then(|| char::from(bytes[0]))
    }
}

pub(crate) fn read<T>(pid: u32, parse: impl FnOnce(Stat<'_>) -> Option<T>) -> Option<T> {
    read_record(File::open(format!("/proc/{pid}/stat")).ok()?, parse)
}

fn read_record<T>(mut reader: impl Read, parse: impl FnOnce(Stat<'_>) -> Option<T>) -> Option<T> {
    // Procfs reports size zero. Avoid File's metadata/seek-based sizing path.
    let mut bytes = [0u8; 4096];
    let mut used = 0;
    while used < bytes.len() {
        match reader.read(&mut bytes[used..]) {
            Ok(0) => return parse(Stat::parse(&bytes[..used])?),
            Ok(n) => used += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    // Never parse an incomplete prefix, even with unusually long comm records.
    let mut bytes = bytes.to_vec();
    reader.read_to_end(&mut bytes).ok()?;
    parse(Stat::parse(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_fields_after_arbitrary_comm_bytes_and_short_reads() {
        let long_name = b"a ) S 1 2 ".repeat(500);
        for name in [b"normal".as_slice(), b"a (name) with )\n\xff", &long_name] {
            let mut stat = b"123 (".to_vec();
            stat.extend_from_slice(name);
            stat.extend_from_slice(
                b") S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654321 999 -1\n",
            );
            let reader = (&stat[..3]).chain(&stat[3..]);
            assert_eq!(
                read_record(reader, |s| {
                    assert_eq!(s.name, name);
                    assert_eq!(s.state(), Some('S'));
                    assert_eq!(s.number::<u32>(4), Some(1));
                    assert_eq!(s.number::<u64>(14), Some(11));
                    assert_eq!(s.number::<u64>(15), Some(12));
                    assert_eq!(s.number::<u64>(23), Some(999));
                    assert_eq!(s.number::<u64>(24), None);
                    s.number::<u64>(22)
                }),
                Some(987654321)
            );
        }
    }

    #[test]
    fn rejects_truncated_or_invalid_start_ticks() {
        for stat in [
            b"123 (missing delimiter S 1".as_slice(),
            b"123 (comm) S 1 2",
            b"123 (comm) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 bad",
            b"123 (comm) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 18446744073709551616",
        ] {
            assert_eq!(read_record(stat, |s| s.number::<u64>(22)), None);
        }
    }
}
