//! `--screenshot <file.png>`: capture the window's picture (including the web view) and save it.

use std::path::Path;

#[cfg(windows)]
pub fn capture_window(hwnd: isize, out: &Path) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    unsafe {
        let hwnd = hwnd as HWND;
        let mut r: RECT = std::mem::zeroed();
        if GetClientRect(hwnd, &mut r) == 0 {
            return Err("could not read the window size".into());
        }
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        if w <= 0 || h <= 0 {
            return Err("window has no size".into());
        }
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp as _);
        // 1 = PW_CLIENTONLY, 2 = PW_RENDERFULLCONTENT (needed for GPU-drawn web content)
        let ok = PrintWindow(hwnd, mem, 1 | 2);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down rows
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(mem, bmp, 0, h as u32, buf.as_mut_ptr() as _, &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp as _);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 || lines == 0 {
            return Err("the window picture could not be taken".into());
        }
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA -> RGBA
            px[3] = 255;
        }
        if let Some(d) = out.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        image::save_buffer(out, &buf, w as u32, h as u32, image::ColorType::Rgba8).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
pub fn capture_window(_hwnd: isize, _out: &Path) -> Result<(), String> {
    Err("screenshots are only supported on Windows in this version".into())
}
