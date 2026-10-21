//! An integrated search header and quiet, keyboard-first result shelf.
//! GDI only: no decoding, blur, per-row assets or synthetic controls.
use super::*;

// COLORREF values: a restrained, slightly desaturated Spotlight blue.
const ACCENT: u32 = 0x00A77A4B;

fn hairline(app: &App) -> u32 {
    if app.settings.theme == Theme::Ink {
        rgb(58, 61, 69)
    } else {
        rgb(223, 226, 232)
    }
}

unsafe fn rule(dc: HDC, left: i32, right: i32, y: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, 1, color);
    let old = SelectObject(dc, pen);
    MoveToEx(dc, left, y, null_mut());
    LineTo(dc, right, y);
    SelectObject(dc, old);
    DeleteObject(pen);
}

// Reads only: never install placeholder text in the native EDIT or disturb undo.
unsafe fn query_scope(app: &App) -> &'static str {
    if app.context.is_some() {
        return "Actions";
    }
    let mut value = [0u16; 513];
    let len = GetWindowTextW(app.edit, value.as_mut_ptr(), value.len() as i32);
    let query = String::from_utf16_lossy(&value[..len.max(0) as usize]);
    match query.trim_start().chars().next() {
        Some('>') => "Commands",
        Some('=') => "Calculator",
        Some('?') => "Web search",
        _ => "Search anything",
    }
}

unsafe fn command_mark(dc: HDC, tile: RECT, scale: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, (scale / 16).max(1), color);
    let old = SelectObject(dc, pen);
    let x = tile.left + scale / 4;
    let y = tile.top + scale / 3;
    MoveToEx(dc, x, y, null_mut());
    LineTo(dc, x + scale / 5, tile.top + scale / 2);
    LineTo(dc, x, tile.top + scale * 2 / 3);
    MoveToEx(
        dc,
        tile.left + scale / 2,
        tile.top + scale * 2 / 3,
        null_mut(),
    );
    LineTo(dc, tile.left + scale * 3 / 4, tile.top + scale * 2 / 3);
    SelectObject(dc, old);
    DeleteObject(pen);
}

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
    let stroke = CreatePen(PS_SOLID, (size / 10).max(1), ACCENT);
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

unsafe fn folder_mark(dc: HDC, tile: RECT, scale: i32, color: u32) {
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    let x = tile.left + scale * 7 / 32;
    let y = tile.top + scale * 9 / 32;
    let tab = RECT {
        left: x,
        top: y,
        right: x + scale * 10 / 32,
        bottom: y + scale * 4 / 32,
    };
    let body = RECT {
        left: x,
        top: y + scale * 3 / 32,
        right: tile.right - scale * 6 / 32,
        bottom: tile.bottom - scale * 7 / 32,
    };
    RoundRect(
        dc,
        tab.left,
        tab.top,
        tab.right,
        tab.bottom,
        scale / 8,
        scale / 8,
    );
    RoundRect(
        dc,
        body.left,
        body.top,
        body.right,
        body.bottom,
        scale / 7,
        scale / 7,
    );
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    DeleteObject(brush);
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

        // One continuous search surface, rather than a form field inside a card.
        // It uses the EDIT's own brush, so the native caret/text area disappears
        // into the header. Child geometry and the three-row list stay untouched.
        let search_surface = RECT {
            left: 0,
            top: 0,
            right: client.right,
            bottom: p(80),
        };
        FillRect(dc, &search_surface, app.field);
        rule(dc, p(20), client.right - p(20), p(80), hairline(app));
        magnifier(dc, client.right - p(41), p(48), p(19));

        let scope = query_scope(app);
        SelectObject(dc, app.font);
        text(
            dc,
            scope,
            RECT {
                left: p(20),
                top: p(88),
                right: client.right - p(150),
                bottom: p(112),
            },
            app.muted(),
            DT_LEFT | DT_VCENTER,
        );

        if client.right >= p(520) {
            SelectObject(dc, app.small_font);
            text(
                dc,
                &format!(
                    "{} {}",
                    app.rows.len(),
                    if app.rows.len() == 1 {
                        "result"
                    } else {
                        "results"
                    }
                ),
                RECT {
                    left: client.right - p(246),
                    top: p(88),
                    right: client.right - p(136),
                    bottom: p(112),
                },
                app.muted(),
                DT_RIGHT | DT_VCENTER,
            );
        }

        // Separate the result shelf from the low-priority native status strip.
        rule(
            dc,
            p(20),
            client.right - p(20),
            client.bottom - p(36),
            hairline(app),
        );
        // Status ends at width - 400; these hints live entirely to its right.
        if client.right >= p(520) {
            let selected = SendMessageW(app.list, LB_GETCURSEL, 0, 0);
            let verb = app
                .rows
                .get(selected as usize)
                .map_or("Open", |row| match &row.action {
                    Action::Copy(_) => "Copy",
                    Action::App(_) | Action::Folder(_) | Action::File(_) | Action::Url(_) => "Open",
                    _ => "Run",
                });
            SelectObject(dc, app.small_font);
            text(
                dc,
                &format!(
                    "↑↓ Navigate    ↵ {verb}    Esc {}",
                    if app.context.is_some() {
                        "Back"
                    } else {
                        "Hide"
                    }
                ),
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
                    "Finding apps and folders…"
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
                if scope == "Commands" {
                    "Try > settings or > to browse commands"
                } else {
                    "Apps & folders  ·  = calculate  ·  ? web  ·  > commands"
                },
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
        p(10),
        selected.then_some(if app.settings.theme == Theme::Ink {
            rgb(65, 79, 101)
        } else {
            rgb(214, 224, 240)
        }),
    );

    let icon_x = item.rcItem.left + p(18);
    let icon_y = item.rcItem.top + p(16);
    let icon_size = p(32);
    let icon = match row.action {
        Action::App(index) => app
            .entries
            .get(index)
            .and_then(|entry| app.icons.as_ref().and_then(|cache| cache.get(&entry.path))),
        Action::Folder(ref path) | Action::File(ref path) => {
            app.icons.as_ref().and_then(|cache| cache.get(path))
        }
        _ => None,
    };

    if let Some(icon) = icon {
        DrawIconEx(
            dc,
            icon_x,
            icon_y,
            icon,
            icon_size,
            icon_size,
            0,
            null_mut(),
            DI_NORMAL,
        );
    } else {
        let tile = RECT {
            left: icon_x,
            top: icon_y,
            right: icon_x + icon_size,
            bottom: icon_y + icon_size,
        };
        rounded(dc, tile, app.field, p(10), None);
        match row.action {
            Action::Context(id) => command_icons::draw_context(dc, tile, icon_size, ACCENT, id),
            Action::App(_) => {
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
            }
            Action::Folder(_) => folder_mark(
                dc,
                tile,
                icon_size,
                if selected { ACCENT } else { app.muted() },
            ),
            Action::Copy(_) | Action::File(_) | Action::Url(_) => {
                let glyph = match row.action {
                    Action::Copy(_) => "=",
                    Action::File(_) => "▤",
                    Action::Url(_) => "↗",
                    _ => unreachable!(),
                };
                SelectObject(dc, app.input_font);
                text(dc, glyph, tile, ACCENT, DT_CENTER | DT_VCENTER);
            }
            // System and Utility commands (and future command kinds) get a
            // crisp terminal mark without depending on icon fonts or assets.
            _ => {
                if !command_icons::draw(dc, tile, icon_size, ACCENT, &row.action) {
                    command_mark(dc, tile, icon_size, ACCENT);
                }
            }
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
        // Shortcut labels are quiet annotations, not a second set of buttons.
        if selected {
            rounded(dc, keycap, app.field, p(6), None);
        }
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
            top: item.rcItem.top + p(if app.context.is_some() { 16 } else { 7 }),
            right: hint_left - p(12),
            bottom: item.rcItem.top + p(if app.context.is_some() { 48 } else { 34 }),
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
            app.background
        },
        app.px(6),
        (item.itemState & ODS_FOCUS != 0).then_some(ACCENT),
    );
    SelectObject(item.hDC, app.small_font);
    text(
        item.hDC,
        if app.context.is_some() {
            "Back"
        } else {
            "Settings"
        },
        rect,
        app.muted(),
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
    // Use the normal text face for discovery copy; the typed query retains the
    // larger input_font. Choose shorter wording instead of a crowded ellipsis.
    SelectObject(dc, app.font);
    let hint = if app.context.is_some() {
        "Filter actions…"
    } else if rect.right - rect.left >= app.px(490) {
        "Apps & folders  ·  = calculate  ·  ? web  ·  > commands"
    } else if rect.right - rect.left >= app.px(390) {
        "Apps, folders  ·  = calc  ·  ? web  ·  > commands"
    } else {
        "Search anything  ·  > commands"
    };
    text(dc, hint, rect, app.muted(), DT_LEFT | DT_VCENTER);
    RestoreDC(dc, saved);
    ReleaseDC(edit, dc);
}
