//! Quiet, keyboard-first launcher drawing. GDI only: no decoding, blur or per-row assets.
use super::*;

// COLORREF for a restrained steel-blue accent (#749FC8).
const ACCENT: u32 = 0x00C89F74;
const SEARCH_BORDER: u32 = 0x00515C68;
const KEYCAP_BORDER: u32 = 0x00515C68;

unsafe fn text(dc: HDC, value: &str, mut rect: RECT, color: u32, flags: u32) {
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT as i32);
    DrawTextW(
        dc,
        wide(value).as_ptr(),
        -1,
        &mut rect,
        flags | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
}

unsafe fn rounded(dc: HDC, rect: RECT, brush: HBRUSH, radius: i32, border: Option<u32>) {
    let old_brush = SelectObject(dc, brush);
    let pen = border.map(|color| CreatePen(PS_SOLID, 1, color));
    let old_pen = SelectObject(dc, pen.unwrap_or_else(|| GetStockObject(NULL_PEN)));
    RoundRect(
        dc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius,
        radius,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    if let Some(pen) = pen {
        DeleteObject(pen);
    }
}

unsafe fn magnifier(dc: HDC, center_x: i32, center_y: i32, size: i32) {
    let stroke = CreatePen(PS_SOLID, (size / 9).max(1), ACCENT);
    let old_pen = SelectObject(dc, stroke);
    let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
    let r = size / 3;
    Ellipse(dc, center_x - r, center_y - r, center_x + r, center_y + r);
    MoveToEx(dc, center_x + r / 2, center_y + r / 2, null_mut());
    LineTo(dc, center_x + size / 2, center_y + size / 2);
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(stroke);
}

pub(super) unsafe fn paint(app: &App, hwnd: HWND) {
    let mut ps = zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let saved = SaveDC(dc);
    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    FillRect(dc, &client, app.background);

    if hwnd == app.hwnd {
        let p = |n| app.px(n);
        // Search is the one prominent surface; the edit control itself is a child
        // window and sits above this inset panel.
        let search_box = RECT {
            left: p(16),
            top: p(16),
            right: client.right - p(16),
            bottom: p(80),
        };
        rounded(
            dc,
            search_box,
            app.field,
            p(14),
            Some(if GetFocus() == app.edit {
                ACCENT
            } else {
                SEARCH_BORDER
            }),
        );
        magnifier(dc, client.right - p(41), p(48), p(20));

        SelectObject(dc, app.small_font);
        text(
            dc,
            "QUICK LAUNCH",
            RECT {
                left: p(20),
                top: p(88),
                right: p(180),
                bottom: p(112),
            },
            app.muted(),
            DT_LEFT | DT_VCENTER,
        );
        text(
            dc,
            &format!("{} RESULTS", app.rows.len()),
            RECT {
                left: client.right - p(246),
                top: p(88),
                right: client.right - p(136),
                bottom: p(112),
            },
            app.muted(),
            DT_RIGHT | DT_VCENTER,
        );

        if client.right >= p(520) {
            text(
                dc,
                "UP / DOWN  SELECT     ENTER  OPEN     ESC  HIDE",
                RECT {
                    left: client.right - p(390),
                    top: client.bottom - p(30),
                    right: client.right - p(20),
                    bottom: client.bottom - p(8),
                },
                app.muted(),
                DT_RIGHT | DT_VCENTER,
            );
        }

        if app.rows.is_empty() {
            let center = (p(116) + client.bottom - p(38)) / 2;
            SelectObject(dc, app.font);
            text(
                dc,
                if app.worker.is_some() {
                    "Finding your applications..."
                } else {
                    "No matching results"
                },
                RECT {
                    left: p(30),
                    top: center - p(23),
                    right: client.right - p(30),
                    bottom: center,
                },
                app.text_color(),
                DT_CENTER | DT_VCENTER,
            );
            SelectObject(dc, app.small_font);
            text(
                dc,
                "Try an app name, = 12 * 3, ? search terms, or > commands",
                RECT {
                    left: p(30),
                    top: center + p(5),
                    right: client.right - p(30),
                    bottom: center + p(27),
                },
                app.muted(),
                DT_CENTER | DT_VCENTER,
            );
        }
    }

    RestoreDC(dc, saved);
    EndPaint(hwnd, &ps);
}

pub(super) unsafe fn draw_row(app: &App, item: &DRAWITEMSTRUCT) {
    if item.itemID == u32::MAX {
        return;
    }
    let Some(row) = app.rows.get(item.itemID as usize) else {
        return;
    };

    let p = |n| app.px(n);
    let dc = item.hDC;
    let saved = SaveDC(dc);
    let selected = item.itemState & ODS_SELECTED != 0;
    FillRect(dc, &item.rcItem, app.background);

    let mut card = item.rcItem;
    card.left += p(6);
    card.right -= p(6);
    card.top += p(2);
    card.bottom -= p(2);
    rounded(
        dc,
        card,
        if selected {
            app.selected
        } else {
            app.background
        },
        p(11),
        None,
    );
    if selected {
        let accent = CreateSolidBrush(ACCENT);
        rounded(
            dc,
            RECT {
                left: card.left,
                top: card.top + p(15),
                right: card.left + p(3),
                bottom: card.bottom - p(15),
            },
            accent,
            p(3),
            None,
        );
        DeleteObject(accent);
    }

    let icon_x = item.rcItem.left + p(18);
    let icon_y = item.rcItem.top + p(16);
    let icon = match row.action {
        Action::App(index) => app
            .entries
            .get(index)
            .and_then(|entry| app.icons.as_ref().and_then(|cache| cache.get(&entry.path))),
        _ => None,
    };
    if let Some(icon) = icon {
        DrawIconEx(
            dc,
            icon_x,
            icon_y,
            icon,
            p(32),
            p(32),
            0,
            null_mut(),
            DI_NORMAL,
        );
    } else {
        let tile = RECT {
            left: icon_x,
            top: icon_y,
            right: icon_x + p(32),
            bottom: icon_y + p(32),
        };
        rounded(dc, tile, app.field, p(9), None);
        if matches!(row.action, Action::App(_)) {
            DrawIconEx(
                dc,
                icon_x + p(4),
                icon_y + p(4),
                app.icon,
                p(24),
                p(24),
                0,
                null_mut(),
                DI_NORMAL,
            );
        } else {
            let glyph = match row.action {
                Action::Copy(_) => "=",
                Action::Url(_) => "↗",
                Action::Settings => "S",
                Action::Reindex => "↻",
                Action::Quit => "×",
                _ => "•",
            };
            SelectObject(dc, app.input_font);
            text(dc, glyph, tile, ACCENT, DT_CENTER | DT_VCENTER);
        }
    }

    let hint = if item.itemID < 9 {
        format!("Alt+{}", item.itemID + 1)
    } else {
        String::new()
    };
    let hint_right = item.rcItem.right - p(18);
    let hint_left = hint_right - p(52);
    if !hint.is_empty() {
        let keycap = RECT {
            left: hint_left,
            top: item.rcItem.top + p(20),
            right: hint_right,
            bottom: item.rcItem.top + p(44),
        };
        rounded(dc, keycap, app.field, p(6), Some(KEYCAP_BORDER));
        SelectObject(dc, app.small_font);
        text(
            dc,
            &hint,
            keycap,
            if selected { ACCENT } else { app.muted() },
            DT_CENTER | DT_VCENTER,
        );
    }

    SelectObject(dc, app.font);
    text(
        dc,
        &row.title,
        RECT {
            left: item.rcItem.left + p(64),
            top: item.rcItem.top + p(8),
            right: hint_left - p(12),
            bottom: item.rcItem.top + p(34),
        },
        app.text_color(),
        DT_LEFT | DT_VCENTER,
    );
    SelectObject(dc, app.small_font);
    text(
        dc,
        &row.detail,
        RECT {
            left: item.rcItem.left + p(64),
            top: item.rcItem.top + p(34),
            right: hint_left - p(12),
            bottom: item.rcItem.bottom - p(5),
        },
        app.muted(),
        DT_LEFT | DT_VCENTER,
    );

    RestoreDC(dc, saved);
}

pub(super) unsafe fn draw_settings_button(app: &App, item: &DRAWITEMSTRUCT) {
    let saved = SaveDC(item.hDC);
    FillRect(item.hDC, &item.rcItem, app.background);
    let mut rect = item.rcItem;
    InflateRect(&mut rect, -1, -1);
    rounded(
        item.hDC,
        rect,
        if item.itemState & ODS_SELECTED != 0 {
            app.selected
        } else {
            app.field
        },
        app.px(8),
        (item.itemState & ODS_FOCUS != 0).then_some(ACCENT),
    );
    SelectObject(item.hDC, app.small_font);
    text(
        item.hDC,
        "SETTINGS",
        rect,
        app.text_color(),
        DT_CENTER | DT_VCENTER,
    );
    RestoreDC(item.hDC, saved);
}

pub(super) unsafe fn draw_search_hint(app: &App, edit: HWND) {
    let dc = GetDC(edit);
    if dc.is_null() {
        return;
    }
    let saved = SaveDC(dc);
    let mut rect: RECT = zeroed();
    GetClientRect(edit, &mut rect);
    rect.left += app.px(2);
    SelectObject(dc, app.input_font);
    text(
        dc,
        "Search apps, = calculate, ? search web",
        rect,
        app.muted(),
        DT_LEFT | DT_VCENTER,
    );
    RestoreDC(dc, saved);
    ReleaseDC(edit, dc);
}
