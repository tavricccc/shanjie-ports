//! The candidate window (macos/Sources/ShanjieKit/CandidatePanel.swift's role), drawn by the host with GDI: a topmost
//! popup that never takes the focus, under the caret the DLL reported, kept on the caret's monitor.
//! - the bar: one row; the vertical window: one column; the expanded grid: `columns` cells per row;
//! - each cell: its number slot (`panel::numbers`: digits, the Tab glyph of the not-entered prediction row, or nothing),
//!   the text, the note in a smaller grey font; the selected cell highlighted;
//! - the hint: one line of text;
//! - a click on a cell is `candidateSelected` for the session the window shows for.
//! The look follows the system colors and the UI font, not macOS's glass (the glass tint setting is not drawn).
//! UNVERIFIED ON WINDOWS.
use super::{post, wide, Msg};
use shanjie_host::hub::{Cell, Panel, View};
use std::cell::RefCell;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

struct Layout {
    /// Per cell: its rectangle, in client coordinates.
    cells: Vec<RECT>,
    size: SIZE,
}

struct State {
    hwnd: HWND,
    view: Option<View>,
    layout: Layout,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub struct Window;

const PAD: i32 = 6;
const GAP: i32 = 10;

fn scale(hwnd: HWND, v: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
    v * dpi / 96
}

fn font(hwnd: HWND, points: i32) -> HFONT {
    let height = -scale(hwnd, points * 4 / 3);
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Microsoft JhengHei UI"),
        )
    }
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn extent(dc: HDC, s: &str) -> SIZE {
    let mut size = SIZE::default();
    let t = utf16(s);
    unsafe {
        let _ = GetTextExtentPoint32W(dc, &t, &mut size);
    }
    size
}

/// The width of one cell: number, gap, text, note.
fn cell_width(dc: HDC, small: HFONT, big: HFONT, c: &Cell, hwnd: HWND) -> (i32, i32) {
    unsafe {
        SelectObject(dc, small.into());
        let n = c.number.as_deref().map_or(0, |n| extent(dc, n).cx + scale(hwnd, 4));
        let note = c.note.as_deref().map_or(0, |t| extent(dc, t).cx + scale(hwnd, 4));
        SelectObject(dc, big.into());
        let t = extent(dc, &c.text);
        (n + t.cx + note, t.cy)
    }
}

fn layout(hwnd: HWND, view: &View) -> Layout {
    unsafe {
        let dc = GetDC(Some(hwnd));
        let big = font(hwnd, 14);
        let small = font(hwnd, 10);
        let pad = scale(hwnd, PAD);
        let gap = scale(hwnd, GAP);
        let mut cells = Vec::new();
        let size = match view {
            View::Hint { text, .. } => {
                SelectObject(dc, big.into());
                let s = extent(dc, text);
                SIZE { cx: s.cx + 2 * pad, cy: s.cy + 2 * pad }
            }
            View::Candidates { cells: items, columns, vertical, .. } => {
                let per_row = if *vertical { 1 } else if *columns > 0 { *columns as usize } else { items.len().max(1) };
                let sizes: Vec<(i32, i32)> = items.iter().map(|c| cell_width(dc, small, big, c, hwnd)).collect();
                let line = sizes.iter().map(|s| s.1).max().unwrap_or(0) + pad;
                // In the grid every column is as wide as its widest cell, so the numbers line up.
                let ncols = per_row.min(items.len().max(1));
                let mut widths = vec![0; ncols];
                for (i, s) in sizes.iter().enumerate() {
                    let c = i % per_row;
                    widths[c] = widths[c].max(s.0);
                }
                let mut y = pad;
                let mut max_x = 0;
                for (i, _) in items.iter().enumerate() {
                    let col = i % per_row;
                    if col == 0 && i > 0 {
                        y += line;
                    }
                    let x = pad + widths[..col].iter().map(|w| w + gap).sum::<i32>();
                    let w = if per_row == 1 { widths[0] } else { widths[col] };
                    cells.push(RECT { left: x - pad / 2, top: y - pad / 2, right: x + w + pad / 2, bottom: y + line - pad / 2 });
                    max_x = max_x.max(x + w);
                }
                SIZE { cx: max_x + pad, cy: y + line + pad / 2 }
            }
        };
        let _ = DeleteObject(big.into());
        let _ = DeleteObject(small.into());
        ReleaseDC(Some(hwnd), dc);
        Layout { cells, size }
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut ps) };
    STATE.with_borrow(|s| {
        let Some(s) = s else { return };
        let Some(view) = &s.view else { return };
        unsafe {
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            FillRect(dc, &rc, GetSysColorBrush(COLOR_WINDOW));
            SetBkMode(dc, TRANSPARENT);
            let big = font(hwnd, 14);
            let small = font(hwnd, 10);
            let pad = scale(hwnd, PAD);
            match view {
                View::Hint { text, .. } => {
                    SelectObject(dc, big.into());
                    SetTextColor(dc, COLORREF(GetSysColor(COLOR_GRAYTEXT)));
                    let _ = TextOutW(dc, pad, pad, &utf16(text));
                }
                View::Candidates { cells, selected, .. } => {
                    for (i, c) in cells.iter().enumerate() {
                        let Some(r) = s.layout.cells.get(i) else { continue };
                        let on = *selected >= 0 && i == *selected as usize;
                        if on {
                            FillRect(dc, r, GetSysColorBrush(COLOR_HIGHLIGHT));
                        }
                        let fg = COLORREF(GetSysColor(if on { COLOR_HIGHLIGHTTEXT } else { COLOR_WINDOWTEXT }));
                        let dim = COLORREF(GetSysColor(if on { COLOR_HIGHLIGHTTEXT } else { COLOR_GRAYTEXT }));
                        let mut x = r.left + pad / 2;
                        let y = r.top + pad / 2;
                        SelectObject(dc, big.into());
                        let line = extent(dc, &c.text).cy;
                        if let Some(n) = &c.number {
                            SelectObject(dc, small.into());
                            SetTextColor(dc, dim);
                            let e = extent(dc, n);
                            let _ = TextOutW(dc, x, y + (line - e.cy), &utf16(n));
                            x += e.cx + scale(hwnd, 4);
                        }
                        SelectObject(dc, big.into());
                        SetTextColor(dc, fg);
                        let _ = TextOutW(dc, x, y, &utf16(&c.text));
                        x += extent(dc, &c.text).cx + scale(hwnd, 4);
                        if let Some(note) = &c.note {
                            SelectObject(dc, small.into());
                            SetTextColor(dc, dim);
                            let e = extent(dc, note);
                            let _ = TextOutW(dc, x, y + (line - e.cy), &utf16(note));
                        }
                    }
                }
            }
            let _ = DeleteObject(big.into());
            let _ = DeleteObject(small.into());
        }
    });
    unsafe {
        let _ = EndPaint(hwnd, &ps);
    }
}

fn hit(x: i32, y: i32) -> Option<u32> {
    STATE.with_borrow(|s| {
        let s = s.as_ref()?;
        if !matches!(s.view, Some(View::Candidates { .. })) {
            return None;
        }
        s.layout.cells.iter().position(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom).map(|i| i as u32)
    })
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            unsafe { paint(hwnd) };
            LRESULT(0)
        }
        // Never take the focus from the app being typed in.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONUP => {
            let x = (l.0 & 0xffff) as i16 as i32;
            let y = ((l.0 >> 16) & 0xffff) as i16 as i32;
            if let Some(i) = hit(x, y) {
                post(Msg::Click(i));
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

impl Window {
    pub fn create(instance: HINSTANCE) -> Window {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance,
                lpszClassName: w!("ShanjieCandidates"),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                style: CS_DROPSHADOW,
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("ShanjieCandidates"),
                PCWSTR(wide("善解").as_ptr()),
                WS_POPUP | WS_BORDER,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )
            .unwrap_or_default();
            STATE.set(Some(State { hwnd, view: None, layout: Layout { cells: Vec::new(), size: SIZE::default() } }));
        }
        Window
    }
}

/// Below the caret, or above it when the window would leave the monitor's work area at the bottom; inside it sideways.
fn place(anchor: Option<[i32; 4]>, size: SIZE) -> POINT {
    unsafe {
        let [left, top, _right, bottom] = anchor.unwrap_or_else(|| {
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            [p.x, p.y, p.x, p.y + 16]
        });
        let monitor = MonitorFromPoint(POINT { x: left, y: bottom }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let work = info.rcWork;
        let mut x = left;
        let mut y = bottom + 2;
        if y + size.cy > work.bottom {
            y = top - size.cy - 2;
        }
        if x + size.cx > work.right {
            x = work.right - size.cx;
        }
        POINT { x: x.max(work.left), y: y.max(work.top) }
    }
}

impl Panel for Window {
    fn show(&mut self, view: View, anchor: Option<[i32; 4]>) {
        let hwnd = STATE.with_borrow(|s| s.as_ref().map(|s| s.hwnd));
        let Some(hwnd) = hwnd else { return };
        let l = layout(hwnd, &view);
        let size = l.size;
        STATE.with_borrow_mut(|s| {
            if let Some(s) = s {
                s.view = Some(view);
                s.layout = l;
            }
        });
        let at = place(anchor, size);
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), at.x, at.y, size.cx, size.cy, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    }

    fn hide(&mut self) {
        STATE.with_borrow_mut(|s| {
            if let Some(s) = s {
                s.view = None;
                unsafe {
                    let _ = ShowWindow(s.hwnd, SW_HIDE);
                }
            }
        });
    }
}
