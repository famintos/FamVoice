#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{GetLastError, LocalFree},
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
};

#[cfg(windows)]
pub fn protect_string(plaintext: &str, context: &str) -> Result<String, String> {
    protect_bytes(plaintext.as_bytes(), context)
}

#[cfg(not(windows))]
pub fn protect_string(_plaintext: &str, _context: &str) -> Result<String, String> {
    Err("DPAPI is only available on Windows".to_string())
}

#[cfg(windows)]
pub fn unprotect_string(encoded: &str, context: &str) -> Result<String, String> {
    let decrypted = unprotect_bytes(encoded, context)?;
    String::from_utf8(decrypted)
        .map_err(|error| format!("DPAPI {context} payload is not valid UTF-8: {error}"))
}

#[cfg(not(windows))]
pub fn unprotect_string(_encoded: &str, _context: &str) -> Result<String, String> {
    Err("DPAPI is only available on Windows".to_string())
}

#[cfg(windows)]
fn protect_bytes(plaintext: &[u8], context: &str) -> Result<String, String> {
    let input = blob_from_bytes(plaintext)?;
    let mut output = CRYPT_INTEGER_BLOB::default();

    let success = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if success == 0 {
        let error_code = unsafe { GetLastError() };
        return Err(format!(
            "DPAPI failed to protect {context} (Win32 error {error_code})"
        ));
    }

    let encrypted = unsafe { copy_blob(&output) };
    unsafe {
        LocalFree(output.pbData.cast());
    }

    Ok(hex_encode(&encrypted))
}

#[cfg(windows)]
fn unprotect_bytes(encoded: &str, context: &str) -> Result<Vec<u8>, String> {
    let encrypted = hex_decode(encoded, context)?;
    let input = blob_from_bytes(&encrypted)?;
    let mut description_ptr = std::ptr::null_mut();
    let mut output = CRYPT_INTEGER_BLOB::default();

    let success = unsafe {
        CryptUnprotectData(
            &input,
            &mut description_ptr,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if !description_ptr.is_null() {
        unsafe {
            LocalFree(description_ptr.cast());
        }
    }

    if success == 0 {
        let error_code = unsafe { GetLastError() };
        return Err(format!(
            "DPAPI failed to unprotect {context} (Win32 error {error_code})"
        ));
    }

    let decrypted = unsafe { copy_blob(&output) };
    unsafe {
        LocalFree(output.pbData.cast());
    }

    Ok(decrypted)
}

#[cfg(windows)]
fn blob_from_bytes(bytes: &[u8]) -> Result<CRYPT_INTEGER_BLOB, String> {
    let byte_len = u32::try_from(bytes.len())
        .map_err(|_| "DPAPI payload exceeded Windows size limits".to_string())?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: byte_len,
        pbData: bytes.as_ptr() as *mut u8,
    })
}

#[cfg(windows)]
unsafe fn copy_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if blob.pbData.is_null() || blob.cbData == 0 {
        return Vec::new();
    }

    std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec()
}

#[cfg(windows)]
fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(nibble_to_hex(byte >> 4));
        encoded.push(nibble_to_hex(byte & 0x0f));
    }
    encoded
}

#[cfg(windows)]
fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble is always 4 bits"),
    }
}

#[cfg(windows)]
fn hex_decode(encoded: &str, context: &str) -> Result<Vec<u8>, String> {
    let trimmed = encoded.trim();
    if !trimmed.len().is_multiple_of(2) {
        return Err(format!("DPAPI {context} payload has an invalid hex length"));
    }

    let mut decoded = Vec::with_capacity(trimmed.len() / 2);
    let bytes = trimmed.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let high = hex_value(chunk[0])
            .ok_or_else(|| format!("DPAPI {context} payload contains invalid hex"))?;
        let low = hex_value(chunk[1])
            .ok_or_else(|| format!("DPAPI {context} payload contains invalid hex"))?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

#[cfg(windows)]
fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
