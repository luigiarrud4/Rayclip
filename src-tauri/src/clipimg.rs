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
    let is_gif = bytes.starts_with(b"GIF8");
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    publish(img, bytes, is_gif)
}

#[allow(dead_code)]
pub fn write_image(png_path: &std::path::Path) -> Result<(), String> {
    let img = image::open(png_path).map_err(|e| e.to_string())?;
    let png = std::fs::read(png_path).map_err(|e| e.to_string())?;
    let is_gif = png.starts_with(b"GIF8");
    publish(img, &png, is_gif)
}

fn publish(img: image::DynamicImage, bytes: &[u8], is_gif: bool) -> Result<(), String> {
    let (w, h) = img.dimensions();
    // BGRA, bottom-up (biHeight > 0)
    let rgba = img.to_rgba8();
    let pixels = rgba.as_raw();
    let (wu, hu) = (w as usize, h as usize);
    let dib_len = wu
        .checked_mul(hu)
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n <= u32::MAX as usize)
        .ok_or_else(|| "imagem grande demais para o clipboard".to_string())?;
    if pixels.len() < dib_len {
        return Err("buffer RGBA inconsistente".into());
    }
    let mut dib = vec![0u8; dib_len];
    for y in 0..hu {
        let src = y * wu * 4;
        let dst = (hu - 1 - y) * wu * 4;
        for x in 0..wu {
            let i = src + x * 4;
            let o = dst + x * 4;
            dib[o] = pixels[i + 2];
            dib[o + 1] = pixels[i + 1];
            dib[o + 2] = pixels[i];
            dib[o + 3] = pixels[i + 3];
        }
    }

    unsafe {
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        };
        // A clipboard pode estar aberta por outro app (visualizador,
        // Office, Clipboard History) por alguns ms: tenta de novo em vez
        // de falhar de primeira.
        let mut opened = 0;
        for _ in 0..10 {
            opened = OpenClipboard(std::ptr::null_mut());
            if opened != 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        if opened == 0 {
            return Err("OpenClipboard failed (clipboard em uso)".into());
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
            info[20..24].copy_from_slice(&(dib_len as u32).to_le_bytes()); // biSizeImage
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
            v5[20..24].copy_from_slice(&(dib_len as u32).to_le_bytes());
            // masks BGR A
            v5[40..44].copy_from_slice(&0x00FF0000u32.to_le_bytes());
            v5[44..48].copy_from_slice(&0x0000FF00u32.to_le_bytes());
            v5[48..52].copy_from_slice(&0x000000FFu32.to_le_bytes());
            v5[52..56].copy_from_slice(&0xFF000000u32.to_le_bytes());
            v5[56..60].copy_from_slice(&0x73524742u32.to_le_bytes()); // LCS_sRGB
            v5[108..112].copy_from_slice(&4u32.to_le_bytes()); // bV5Intent: LCS_GM_IMAGES
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

            if is_gif {
                // GIF: publica os bytes ORIGINAIS no formato "GIF" (mantém a
                // animação nos apps que entendem, ex.: navegadores/Office).
                // Antes esses mesmos bytes iam no formato "PNG" — um "PNG"
                // inválido que quebrava apps que pedem PNG de verdade.
                let gif_name = cstr16!("GIF");
                let gif_fmt = RegisterClipboardFormatW(gif_name.as_ptr());
                crate::log(&format!(
                    "clipimg: gif_fmt={gif_fmt} bytes={}",
                    bytes.len()
                ));
                if gif_fmt != 0 {
                    match set_blob(gif_fmt, bytes) {
                        Ok(_) => crate::log("clipimg: GIF ok"),
                        Err(e) => crate::log(&format!("clipimg: GIF failed: {e}")),
                    }
                }
            } else {
                // ---- "PNG" ----
                let png_name = cstr16!("PNG");
                let png_fmt = RegisterClipboardFormatW(png_name.as_ptr());
                crate::log(&format!(
                    "clipimg: png_fmt={png_fmt} bytes={}",
                    bytes.len()
                ));
                if png_fmt != 0 {
                    match set_blob(png_fmt, bytes) {
                        Ok(_) => crate::log("clipimg: PNG ok"),
                        Err(e) => crate::log(&format!("clipimg: PNG failed: {e}")),
                    }
                }
            }
            Ok(())
        })();

        if result.is_err() {
            let _ = CloseClipboard();
            return result;
        }
        // Se falhar, os dados já estão no clipboard: erro aqui só impediria
        // outros apps de abrirem depois (o handle fica preso até outra janela
        // fechar). Não vale abortar a operação.
        if CloseClipboard() == 0 {
            crate::log("clipimg: CloseClipboard falhou");
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
