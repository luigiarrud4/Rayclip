//! Publica uma imagem no clipboard em TODOS os formatos que o Windows entende,
//! para que "colar" funcione em qualquer app — o problema do paste de imagens
//! "erradas"/vazias acontece quando só o CF_BITMAP é setado (muitos apps ignoram).
//!
//! Formatos emitidos: CF_DIB, CF_DIBV5 (32bpp com alpha), CF_BITMAP (HBITMAP)
//! e o formato registrado "PNG".

use image::GenericImageView;

macro_rules! cstr16 {
    ($s:expr) => {{
        let mut v: Vec<u16> = $s.encode_utf16().collect();
        v.push(0);
        v
    }};
}

/// Publica uma imagem que está em memória (bytes PNG/GIF/JPEG do banco).
pub fn write_image_bytes(bytes: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    publish(img, bytes)
}

#[allow(dead_code)]
pub fn write_image(png_path: &std::path::Path) -> Result<(), String> {
    let img = image::open(png_path).map_err(|e| e.to_string())?;
    let png = std::fs::read(png_path).map_err(|e| e.to_string())?;
    publish(img, &png)
}

fn publish(img: image::DynamicImage, png_bytes: &[u8]) -> Result<(), String> {
    let (w, h) = img.dimensions();
    // BGRA, bottom-up (biHeight > 0)
    let rgba = img.to_rgba8();
    let pixels = rgba.as_raw();
    let mut dib = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let src: usize = (y * w * 4) as usize;
        let dst: usize = (((h - 1 - y) * w) * 4) as usize;
        for x in 0..(w as usize) {
            let (r, g, b, a) = (
                pixels[src + x * 4],
                pixels[src + x * 4 + 1],
                pixels[src + x * 4 + 2],
                pixels[src + x * 4 + 3],
            );
            dib[dst + x * 4] = b;
            dib[dst + x * 4 + 1] = g;
            dib[dst + x * 4 + 2] = r;
            dib[dst + x * 4 + 3] = a;
        }
    }

    unsafe {
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        };
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("OpenClipboard failed".into());
        }
        let result = (|| -> Result<(), String> {
            if EmptyClipboard() == 0 {
                return Err("EmptyClipboard failed".into());
            }

            // ---- CF_DIB (BITMAPINFOHEADER + pixels BGRA 32bpp) ----
            let header_size = 40usize;
            let mut info = vec![0u8; header_size];
            info[0..4].copy_from_slice(&(40u32).to_le_bytes()); // biSize
            info[4..8].copy_from_slice(&w.to_le_bytes());
            info[8..12].copy_from_slice(&h.to_le_bytes());
            info[12..14].copy_from_slice(&1u16.to_le_bytes()); // biPlanes
            info[14..16].copy_from_slice(&32u16.to_le_bytes()); // biBitCount
            info[16..20].copy_from_slice(&0u32.to_le_bytes()); // BI_RGB
            info[20..24].copy_from_slice(&(w * h * 4).to_le_bytes()); // biSizeImage
            let mut blob = info;
            blob.extend_from_slice(&dib);
            let blob_len = blob.len();
            set_blob(CF_DIB, &blob)?;

            // ---- CF_DIBV5 ----
            let v5hdr_size = 124usize;
            let mut v5 = vec![0u8; v5hdr_size];
            v5[0..4].copy_from_slice(&(124u32).to_le_bytes());
            v5[4..8].copy_from_slice(&w.to_le_bytes());
            v5[8..12].copy_from_slice(&h.to_le_bytes());
            v5[12..14].copy_from_slice(&1u16.to_le_bytes());
            v5[14..16].copy_from_slice(&32u16.to_le_bytes());
            v5[16..20].copy_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
            v5[20..24].copy_from_slice(&(w * h * 4).to_le_bytes());
            // masks BGR A
            v5[40..44].copy_from_slice(&0x00FF0000u32.to_le_bytes());
            v5[44..48].copy_from_slice(&0x0000FF00u32.to_le_bytes());
            v5[48..52].copy_from_slice(&0x000000FFu32.to_le_bytes());
            v5[52..56].copy_from_slice(&0xFF000000u32.to_le_bytes());
            v5[56..60].copy_from_slice(&0x73524742u32.to_le_bytes()); // LCS_sRGB
            v5[60..64].copy_from_slice(&1u32.to_le_bytes()); // bV5AlphaMask? no—keep 0; alpha via mask
            v5[64..68].copy_from_slice(&1u32.to_le_bytes()); // bV5CSType follow-up: bV5Intent 1 (LCS_GM_GRAPHICS)
            let mut blob5 = v5;
            blob5.extend_from_slice(&dib);
            set_blob(CF_DIBV5, &blob5)?;

            // ---- CF_BITMAP (HBITMAP) via DIB section ----
            let hbitmap = make_bitmap(w, h, &dib)?;
            if SetClipboardData(CF_BITMAP, hbitmap).is_null() {
                return Err("SetClipboardData(CF_BITMAP) failed".into());
            }
            crate::log(&format!(
                "clipimg: {w}x{h} dib={} v5 ok bitmap ok",
                blob_len
            ));

            // ---- "PNG" ----
            let png_name = cstr16!("PNG");
            let png_fmt = RegisterClipboardFormatW(png_name.as_ptr());
            crate::log(&format!(
                "clipimg: png_fmt={png_fmt} bytes={}",
                png_bytes.len()
            ));
            if png_fmt != 0 {
                match set_blob(png_fmt, png_bytes) {
                    Ok(_) => crate::log("clipimg: PNG ok"),
                    Err(e) => crate::log(&format!("clipimg: PNG failed: {e}")),
                }
            }
            Ok(())
        })();

        if result.is_err() {
            let _ = CloseClipboard();
            return result;
        }
        if CloseClipboard() == 0 {
            return Err("CloseClipboard failed".into());
        }
        Ok(())
    }
}

const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;
const CF_BITMAP: u32 = 2;

unsafe fn set_blob(fmt: u32, data: &[u8]) -> Result<(), String> {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::SetClipboardData;
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if h.is_null() {
        return Err("GlobalAlloc failed".into());
    }
    let p = GlobalLock(h);
    if p.is_null() {
        let _ = GlobalFree(h);
        return Err("GlobalLock failed".into());
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), p as *mut u8, data.len());
    let _ = GlobalUnlock(h);
    if SetClipboardData(fmt, h).is_null() {
        let _ = GlobalFree(h);
        return Err(format!("SetClipboardData({}) failed", fmt));
    }
    Ok(())
}

unsafe fn make_bitmap(w: u32, h: u32, dib: &[u8]) -> Result<*mut core::ffi::c_void, String> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    // BITMAPINFO already counts the header + palette (none), so one struct.
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w as i32;
    bmi.bmiHeader.biHeight = h as i32;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = 0; // BI_RGB
    bmi.bmiHeader.biSizeImage = dib.len() as u32;
    let mut bits = std::ptr::null_mut();
    let hbmp = CreateDIBSection(
        std::ptr::null_mut(),
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if hbmp.is_null() || bits.is_null() {
        if !hbmp.is_null() {
            let _ = DeleteObject(hbmp);
        }
        return Err("CreateDIBSection failed".into());
    }
    std::ptr::copy_nonoverlapping(dib.as_ptr(), bits as *mut u8, dib.len());
    Ok(hbmp)
}
