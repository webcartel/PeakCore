//! Executable icons for the process table, cached in memory only.

use std::collections::HashMap;

/// Bounded cache so a long-running window cannot grow without limit.
const MAX_CACHED_ICONS: usize = 512;

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;
    use windows_sys::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

    /// Renders the shell icon of `path` as PNG bytes.
    pub(super) fn icon_png(path: &str) -> Option<Vec<u8>> {
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
        let flags = SHGFI_ICON | SHGFI_LARGEICON;
        // SAFETY: `wide` is NUL-terminated and `info` is a writable SHFILEINFOW.
        let found = unsafe {
            SHGetFileInfoW(
                wide.as_ptr(),
                FILE_ATTRIBUTE_NORMAL,
                &mut info,
                size_of::<SHFILEINFOW>() as u32,
                flags,
            )
        };
        if found == 0 || info.hIcon.is_null() {
            return None;
        }
        let rgba = unsafe { icon_rgba(info.hIcon) };
        unsafe { DestroyIcon(info.hIcon) };
        let (width, height, pixels) = rgba?;
        encode_png(width, height, &pixels)
    }

    /// Expands an `HICON` into top-down RGBA pixels.
    unsafe fn icon_rgba(icon: *mut c_void) -> Option<(i32, i32, Vec<u8>)> {
        let mut info: ICONINFO = unsafe { std::mem::zeroed() };
        // SAFETY: `icon` came from the shell and `info` is writable.
        if unsafe { GetIconInfo(icon, &mut info) } == 0 {
            return None;
        }
        let result = unsafe { bitmap_rgba(info.hbmColor, info.hbmMask) };
        unsafe {
            if !info.hbmMask.is_null() {
                DeleteObject(info.hbmMask.cast());
            }
            if !info.hbmColor.is_null() {
                DeleteObject(info.hbmColor.cast());
            }
        }
        result
    }

    unsafe fn bitmap_rgba(color: *mut c_void, mask: *mut c_void) -> Option<(i32, i32, Vec<u8>)> {
        if color.is_null() {
            return None;
        }
        let mut bitmap: BITMAP = unsafe { std::mem::zeroed() };
        // SAFETY: `color` is a colour bitmap owned by the icon.
        if unsafe {
            GetObjectW(
                color,
                size_of::<BITMAP>() as i32,
                (&mut bitmap as *mut BITMAP).cast(),
            )
        } == 0
            || bitmap.bmWidth <= 0
            || bitmap.bmHeight <= 0
        {
            return None;
        }
        let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
        let mut header: BITMAPINFO = unsafe { std::mem::zeroed() };
        header.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        header.bmiHeader.biWidth = width;
        header.bmiHeader.biHeight = -height;
        header.bmiHeader.biPlanes = 1;
        header.bmiHeader.biBitCount = 32;
        header.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let dc = unsafe { GetDC(null_mut()) };
        // SAFETY: the bitmap and the buffer match the described format.
        let copied = unsafe {
            GetDIBits(
                dc,
                color.cast(),
                0,
                height as u32,
                pixels.as_mut_ptr().cast(),
                &mut header,
                DIB_RGB_COLORS,
            )
        };
        unsafe { ReleaseDC(null_mut(), dc) };
        if copied == 0 {
            return None;
        }

        // Icons without an alpha channel need the mask to carve out the background.
        let has_alpha = (0..pixels.len())
            .step_by(4)
            .any(|index| pixels[index + 3] != 0);
        if !has_alpha {
            if let Some(mask_alpha) = unsafe { mask_alpha(mask, width, height) } {
                for (index, alpha) in mask_alpha.into_iter().enumerate() {
                    pixels[index * 4 + 3] = alpha;
                }
            } else {
                for index in (0..pixels.len()).step_by(4) {
                    pixels[index + 3] = 255;
                }
            }
        }
        // GetDIBits returns BGRA; the web renders RGBA.
        for index in (0..pixels.len()).step_by(4) {
            pixels.swap(index, index + 2);
        }
        Some((width, height, pixels))
    }

    unsafe fn mask_alpha(mask: *mut c_void, width: i32, height: i32) -> Option<Vec<u8>> {
        if mask.is_null() {
            return None;
        }
        let mut header: BITMAPINFO = unsafe { std::mem::zeroed() };
        header.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        header.bmiHeader.biWidth = width;
        header.bmiHeader.biHeight = -height;
        header.bmiHeader.biPlanes = 1;
        header.bmiHeader.biBitCount = 32;
        header.bmiHeader.biCompression = BI_RGB;
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let dc = unsafe { GetDC(null_mut()) };
        // SAFETY: the mask bitmap is the same size as the colour bitmap.
        let copied = unsafe {
            GetDIBits(
                dc,
                mask.cast(),
                0,
                height as u32,
                buffer.as_mut_ptr().cast(),
                &mut header,
                DIB_RGB_COLORS,
            )
        };
        unsafe { ReleaseDC(null_mut(), dc) };
        if copied == 0 {
            return None;
        }
        let mut alpha = vec![0u8; (width * height) as usize];
        for (index, value) in alpha.iter_mut().enumerate() {
            *value = if buffer[index * 4] == 0 { 255 } else { 0 };
        }
        Some(alpha)
    }

    fn encode_png(width: i32, height: i32, pixels: &[u8]) -> Option<Vec<u8>> {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(pixels).ok()?;
        }
        Some(png)
    }
}

#[cfg(not(windows))]
mod platform {
    pub(super) fn icon_png(_path: &str) -> Option<Vec<u8>> {
        None
    }
}

/// In-memory icon cache keyed by executable path.
#[derive(Default)]
pub struct IconCache {
    entries: HashMap<String, Option<String>>,
}

impl IconCache {
    /// Returns a `data:image/png;base64` URL, resolving it at most once per path.
    pub fn data_url(&mut self, path: &str) -> Option<String> {
        let key = path.to_lowercase();
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }
        let value =
            platform::icon_png(path).map(|png| format!("data:image/png;base64,{}", base64(&png)));
        if self.entries.len() >= MAX_CACHED_ICONS {
            self.entries.clear();
        }
        self.entries.insert(key, value.clone());
        value
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b1 = chunk[0];
        let b2 = chunk.get(1).copied().unwrap_or(0);
        let b3 = chunk.get(2).copied().unwrap_or(0);
        encoded.push(ALPHABET[(b1 >> 2) as usize] as char);
        encoded.push(ALPHABET[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[(((b2 & 0x0f) << 2) | (b3 >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(b3 & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
