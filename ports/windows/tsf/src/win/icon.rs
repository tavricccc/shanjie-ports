//! The language bar icon: 善 drawn at the small icon size with GDI (the package carries no icon resource), dark grey on a
//! transparent background. A placeholder until the port ships an icon resource made from upstream's glyph.
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe fn glyph(dc: HDC, size: i32, background: HGDIOBJ, color: COLORREF) {
    unsafe {
        let rc = RECT { left: 0, top: 0, right: size, bottom: size };
        FillRect(dc, &rc, HBRUSH(background.0));
        let font = CreateFontW(-size, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, NONANTIALIASED_QUALITY, 0, w!("Microsoft JhengHei UI"));
        let old = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, color);
        let mut text: Vec<u16> = "善".encode_utf16().collect();
        let mut r = rc;
        DrawTextW(dc, &mut text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SelectObject(dc, old);
        let _ = DeleteObject(font.into());
    }
}

pub unsafe fn make() -> windows::core::Result<HICON> {
    unsafe {
        let size = GetSystemMetrics(SM_CXSMICON).max(16);
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let color = CreateCompatibleBitmap(screen, size, size);
        let mask = CreateBitmap(size, size, 1, 1, None);
        ReleaseDC(None, screen);
        // Color: the glyph on black. Mask: 1 (transparent) around it, 0 under it.
        let old = SelectObject(dc, color.into());
        glyph(dc, size, GetStockObject(BLACK_BRUSH), COLORREF(0x00303030));
        SelectObject(dc, mask.into());
        glyph(dc, size, GetStockObject(WHITE_BRUSH), COLORREF(0));
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}
