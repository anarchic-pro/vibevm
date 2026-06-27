//! Inline tar extraction for `git archive` output — pull one file out
//! of the tar byte stream without a `tar` crate. Split from `shell.rs`
//! per the file-length budget; pure byte parsing, no git involved.

/// Pull a single file's bytes out of a tar stream. Returns `None` if the
/// requested path is not present.
///
/// Implemented inline (no `tar` crate) because the data shape is trivial:
/// a tar archive is a sequence of 512-byte headers, each followed by
/// `ceil(size / 512) * 512` bytes of payload, terminated by two empty
/// headers. We read filename, size, payload; skip over directory and
/// other-type entries.
pub(super) fn extract_single_file_from_tar(bytes: &[u8], target_path: &str) -> Option<Vec<u8>> {
    let target_norm = target_path.trim_start_matches("./");
    let mut offset = 0usize;
    while offset + 512 <= bytes.len() {
        let header = &bytes[offset..offset + 512];
        // Empty header marks end-of-archive.
        if header.iter().all(|b| *b == 0) {
            return None;
        }

        // Filename is the first 100 bytes, NUL-terminated. Optionally
        // prefixed (UStar long-name extension via `prefix` field at
        // bytes 345..500), but git archive emits short paths.
        let name = read_cstr(&header[0..100]);
        let prefix = read_cstr(&header[345..500]);
        let full_name = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let full_norm = full_name.trim_start_matches("./").to_string();

        // Size is octal in bytes 124..136.
        let size = parse_octal(&header[124..136]).unwrap_or(0);

        // Type flag at byte 156: '0' or '\0' = regular file.
        let typeflag = header[156];

        let payload_start = offset + 512;
        let payload_end = payload_start + size;
        if payload_end > bytes.len() {
            return None;
        }

        let is_regular = typeflag == b'0' || typeflag == 0;
        if is_regular && full_norm == target_norm {
            return Some(bytes[payload_start..payload_end].to_vec());
        }

        // Advance past payload, rounded up to 512.
        let padded = size.div_ceil(512) * 512;
        offset = payload_start + padded;
    }
    None
}

fn read_cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

pub(super) fn parse_octal(buf: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(buf).ok()?;
    let trimmed = s.trim_matches(|c: char| c == ' ' || c == '\0');
    if trimmed.is_empty() {
        return Some(0);
    }
    usize::from_str_radix(trimmed, 8).ok()
}
