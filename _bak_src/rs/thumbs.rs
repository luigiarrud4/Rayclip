//! Miniaturas de arquivos (PDF, vídeo, imagem, doc) usando o mesmo provedor
//! que o Explorer usa — `IShellItemImageFactory` — sem dependência externa.

use std::path::{Path, PathBuf};
use windows::core::HSTRING;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    SIIGBF_THUMBNAILONLY,
};

/// Extrai um HBITMAP como RGBA top-down.
fn hbitmap_to_rgba(
    hbm: windows::Win32::Graphics::Gdi::HBITMAP,
    w: i32,
    h: i32,
) -> Option<image::RgbaImage> {
    unsafe {
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return None;
        }
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;

        let mut buf = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            hdc,
            hbm,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bi,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(hdc);
        if got == 0 {
            return None;
        }
        // BGRA -> RGBA
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
            if px[3] == 0 {
                px[3] = 255;
            }
        }
        image::RgbaImage::from_raw(w as u32, h as u32, buf)
    }
}

/// Gera (uma vez) a miniatura de um arquivo e devolve o caminho do PNG.
/// Miniatura pequena (PNG) a partir dos bytes de uma imagem já decodificável.
/// Usada na lista: sem isso a lista carregava a imagem **inteira** de cada
/// linha (1305 imagens, 348 MB) — o app travava ao rolar.
pub fn thumb_from_bytes(src: &[u8], max: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(src).ok()?;
    let th = img.thumbnail(max, max);
    let mut out = std::io::Cursor::new(Vec::new());
    th.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub fn file_thumb(path: &Path, out: &Path, size: i32) -> Option<PathBuf> {
    if out.is_file() {
        return Some(out.to_path_buf());
    }
    if !path.exists() {
        return None;
    }
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let _ = hr;
        let factory: windows::core::Result<IShellItemImageFactory> =
            SHCreateItemFromParsingName(&HSTRING::from(path.to_string_lossy().as_ref()), None);
        let result = (|| -> Option<PathBuf> {
            let factory = factory.ok()?;
            let flags =
                windows::Win32::UI::Shell::SIIGBF(SIIGBF_THUMBNAILONLY.0 | SIIGBF_BIGGERSIZEOK.0);
            let hbm = factory.GetImage(SIZE { cx: size, cy: size }, flags).ok()?;
            let (w, h) = (size, size);
            let img = hbitmap_to_rgba(hbm, w, h);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            let img = img?;
            img.save(out).ok()?;
            Some(out.to_path_buf())
        })();
        CoUninitialize();
        result
    }
}

/// Extrai o **ícone** de um executável/arquivo (não a miniatura) e grava em PNG.
/// Usado para mostrar de qual app a entrada veio, como o Raycast faz.
pub fn file_icon(path: &Path, out: &Path, size: i32) -> Option<PathBuf> {
    if out.is_file() {
        return Some(out.to_path_buf());
    }
    if !path.exists() {
        return None;
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let factory: windows::core::Result<IShellItemImageFactory> =
            SHCreateItemFromParsingName(&HSTRING::from(path.to_string_lossy().as_ref()), None);
        let result = (|| -> Option<PathBuf> {
            let factory = factory.ok()?;
            let flags =
                windows::Win32::UI::Shell::SIIGBF(SIIGBF_ICONONLY.0 | SIIGBF_BIGGERSIZEOK.0);
            let hbm = factory.GetImage(SIZE { cx: size, cy: size }, flags).ok()?;
            let img = hbitmap_to_rgba(hbm, size, size);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            let img = img?;
            img.save(out).ok()?;
            Some(out.to_path_buf())
        })();
        CoUninitialize();
        result
    }
}
