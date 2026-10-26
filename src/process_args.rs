//! Native argv reads: backend normalization must not erase empty or padded arguments.

pub(crate) fn read(pid: u32) -> Vec<String> {
    platform::read(pid).unwrap_or_default()
}

#[cfg(target_os = "linux")]
mod platform {
    pub(super) fn read(pid: u32) -> Option<Vec<String>> {
        let bytes = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        Some(parse(&bytes))
    }

    fn parse(bytes: &[u8]) -> Vec<String> {
        if bytes.is_empty() {
            return Vec::new();
        }
        // Only the final NUL is framing. Every other NUL delimits an argument,
        // including leading, interior and trailing empty arguments.
        bytes
            .strip_suffix(&[0])
            .unwrap_or(bytes)
            .split(|&byte| byte == 0)
            .map(|arg| String::from_utf8_lossy(arg).into_owned())
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn preserves_argument_boundaries_and_whitespace() {
            for (bytes, expected) in [
                (b"".as_slice(), vec![]),
                (b"\0", vec![""]),
                (
                    b"\0two words\0\0 \t padded \n \0\0",
                    vec!["", "two words", "", " \t padded \n ", ""],
                ),
                (b"one\0unterminated", vec!["one", "unterminated"]),
            ] {
                assert_eq!(parse(bytes), expected);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::mem::size_of;
    use std::ptr;

    pub(super) fn read(pid: u32) -> Option<Vec<String>> {
        let native_pid = libc::pid_t::try_from(pid).ok()?;
        let info = crate::identity::bsd_info(pid)?;
        // From macOS sys/proc_info.h; not exported by libc.
        const PROC_FLAG_LP64: u32 = 0x10;
        let word_size = if info.pbi_flags & PROC_FLAG_LP64 != 0 {
            8
        } else {
            4
        };
        let mut arg_max: libc::c_int = 0;
        let mut len = size_of::<libc::c_int>();
        let mut limit_mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
        // SAFETY: the MIB, integer and length are valid for their declared
        // sizes; null newp requests a read and no pointers are retained.
        let result = unsafe {
            libc::sysctl(
                limit_mib.as_mut_ptr(),
                2,
                (&mut arg_max as *mut libc::c_int).cast(),
                &mut len,
                ptr::null_mut(),
                0,
            )
        };
        if result != 0 || len != size_of::<libc::c_int>() || arg_max <= 0 {
            return None;
        }
        // A size query for this PID races exec. XNU can return truncated data
        // with success, so use the kernel maximum and reject a full buffer.
        let capacity = usize::try_from(arg_max)
            .ok()?
            .checked_add(size_of::<libc::c_int>())?;
        let mut bytes = vec![0u8; capacity];
        len = bytes.len();
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, native_pid];
        // SAFETY: bytes is initialized writable storage for len bytes; mib and
        // len are valid, and the API retains none of these pointers.
        let result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                bytes.as_mut_ptr().cast(),
                &mut len,
                ptr::null_mut(),
                0,
            )
        };
        if result != 0 || len >= bytes.len() {
            return None;
        }
        parse(&bytes[..len], word_size)
    }

    fn parse(bytes: &[u8], word_size: usize) -> Option<Vec<String>> {
        let argc = i32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?);
        let argc = usize::try_from(argc).ok()?;
        let data = bytes.get(4..)?;
        if argc > data.len() {
            return None;
        }
        let path_len = data.iter().position(|&b| b == 0)?;
        if path_len == 0 {
            return None;
        }
        let path_end = path_len + 1;
        // XNU exec_extract_strings aligns the executable_path= prefix to the
        // target ABI's pointer width. KERN_PROCARGS2 strips the 16-byte key,
        // retaining that alignment. Skipping all NULs would erase empty argv[0].
        let start = path_end.checked_add(word_size - 1)? & !(word_size - 1);
        if data.get(path_end..start)?.iter().any(|&b| b != 0) {
            return None;
        }
        let mut data = data.get(start..)?;
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            let end = data.iter().position(|&b| b == 0)?;
            args.push(String::from_utf8_lossy(&data[..end]).into_owned());
            data = &data[end + 1..];
        }
        // Environment strings can follow argv; never include them as arguments.
        Some(args)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn decodes_target_alignment_and_exact_argc_without_environment() {
            for word_size in [4, 8] {
                let mut bytes = 5i32.to_ne_bytes().to_vec();
                let mut data = b"/program\0".to_vec();
                data.resize(data.len().next_multiple_of(word_size), 0);
                data.extend_from_slice(b"\0two words\0\0 \t padded \n \0\0ENV=not_an_argument\0");
                bytes.extend(data);
                assert_eq!(
                    parse(&bytes, word_size),
                    Some(vec![
                        "".into(),
                        "two words".into(),
                        "".into(),
                        " \t padded \n ".into(),
                        "".into()
                    ])
                );
            }
        }

        #[test]
        fn rejects_invalid_or_truncated_native_records() {
            for bytes in [
                b"".as_slice(),
                b"\xff\xff\xff\xff/path\0\0\0",
                b"\x01\0\0\0\0\0\0\0",
                b"\x02\0\0\0/a/path\0only-one\0",
                b"\x01\0\0\0/a/path\0unterminated",
            ] {
                assert!(parse(bytes, 8).is_none());
            }
        }
    }
}
