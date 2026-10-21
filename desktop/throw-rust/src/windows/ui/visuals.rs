//! GDI-only drawing: no image decoders, blur, animations or per-row icon loading.
use super::*;

const ACCENT: u32 = 0x002B66FD; // Windows COLORREF for #FD662B.

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

pub(super) unsafe fn paint(app: &App, hwnd: HWND) {
    let mut ps = zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let saved = SaveDC(dc);
    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    FillRect(dc, &client, app.background);
    if hwnd == app.hwnd {
        let p = |n| app.px(n);
        DrawIconEx(
            dc,
            p(22),
            p(25),
            app.icon,
            p(32),
            p(32),
            0,
            null_mut(),
            DI_NORMAL,
        );
        SelectObject(dc, app.heading_font);
        text(
            dc,
            "THROW",
            RECT {
                left: p(68),
                top: p(15),
                right: client.right - p(150),
                bottom: p(49),
            },
            app.text_color(),
            DT_LEFT,
        );
        SelectObject(dc, app.small_font);
        text(
            dc,
            "LESS DISTRACTION. MORE DONE.",
            RECT {
                left: p(70),
                top: p(51),
                right: client.right - p(150),
                bottom: p(70),
            },
            app.muted(),
            DT_LEFT,
        );

        let search_box = RECT {
            left: p(20),
            top: p(86),
            right: client.right - p(20),
            bottom: p(146),
        };
        rounded(
            dc,
            search_box,
            app.field,
            p(16),
            Some(if GetFocus() == app.edit {
                ACCENT
            } else {
                app.muted()
            }),
        );
        // A simple vector magnifier; no font-specific icon dependency.
        let pen = CreatePen(PS_SOLID, p(2).max(1), ACCENT);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        Ellipse(dc, p(36), p(105), p(51), p(120));
        MoveToEx(dc, p(48), p(117), null_mut());
        LineTo(dc, p(56), p(125));
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);

        text(
            dc,
            "QUICK LAUNCH",
            RECT {
                left: p(22),
                top: p(155),
                right: p(220),
                bottom: p(174),
            },
            app.muted(),
            DT_LEFT,
        );
        text(
            dc,
            &format!("{} RESULTS", app.rows.len()),
            RECT {
                left: client.right - p(160),
                top: p(155),
                right: client.right - p(22),
                bottom: p(174),
            },
            app.muted(),
            DT_RIGHT,
        );
        if client.right >= p(520) {
            text(
                dc,
                "↑ ↓ select   Enter open   Esc hide",
                RECT {
                    left: client.right - p(268),
                    top: client.bottom - p(30),
                    right: client.right - p(20),
                    bottom: client.bottom - p(9),
                },
                app.muted(),
                DT_RIGHT,
            );
        }
        if app.rows.is_empty() {
            let center = (p(176) + client.bottom - p(44)) / 2;
            SelectObject(dc, app.font);
            text(
                dc,
                if app.worker.is_some() {
                    "Finding your applications…"
                } else {
                    "No matching results"
                },
                RECT {
                    left: p(30),
                    top: center - p(25),
                    right: client.right - p(30),
                    bottom: center,
                },
                app.text_color(),
                DT_CENTER,
            );
            SelectObject(dc, app.small_font);
            text(
                dc,
                "Try an app name, = 12 * 3, ? search terms, or > commands",
                RECT {
                    left: p(30),
                    top: center + p(8),
                    right: client.right - p(30),
                    bottom: center + p(30),
                },
                app.muted(),
                DT_CENTER,
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
        p(12),
        None,
    );
    if selected {
        let accent = CreateSolidBrush(ACCENT);
        rounded(
            dc,
            RECT {
                left: card.left,
                top: card.top + p(13),
                right: card.left + p(3),
                bottom: card.bottom - p(13),
            },
            accent,
            p(3),
            None,
        );
        DeleteObject(accent);
    }
    let icon_x = item.rcItem.left + p(14);
    let icon_y = item.rcItem.top + p(12);
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
        rounded(dc, tile, app.field, p(8), None);
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
                Action::Settings => "⚙",
                Action::Reindex => "↻",
                Action::Quit => "×",
                _ => "•",
            };
            SelectObject(dc, app.input_font);
            text(dc, glyph, tile, ACCENT, DT_CENTER | DT_VCENTER);
        }
    }
    SelectObject(dc, app.font);
    text(
        dc,
        &row.title,
        RECT {
            left: item.rcItem.left + p(62),
            top: item.rcItem.top + p(6),
            right: item.rcItem.right - p(42),
            bottom: item.rcItem.top + p(30),
        },
        app.text_color(),
        DT_LEFT,
    );
    SelectObject(dc, app.small_font);
    text(
        dc,
        &row.detail,
        RECT {
            left: item.rcItem.left + p(62),
            top: item.rcItem.top + p(32),
            right: item.rcItem.right - p(42),
            bottom: item.rcItem.bottom - p(4),
        },
        app.muted(),
        DT_LEFT,
    );
    if selected {
        SelectObject(dc, app.font);
        text(
            dc,
            "↵",
            RECT {
                left: item.rcItem.right - p(34),
                top: item.rcItem.top,
                right: item.rcItem.right - p(10),
                bottom: item.rcItem.bottom,
            },
            ACCENT,
            DT_CENTER | DT_VCENTER,
        );
    }
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
        app.px(10),
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
