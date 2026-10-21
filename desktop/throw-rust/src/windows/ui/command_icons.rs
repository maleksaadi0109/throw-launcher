//! Original, scale-independent GDI line icons for launcher commands.
//! No embedded assets or icon resources are used.
use super::*;
use throw_launcher::commands::{SettingsPage, SystemCommand, UtilityCommand};

#[derive(Clone, Copy)]
enum Mark {
    Power,
    Restart,
    Recovery,
    Logout,
    Lock,
    Moon,
    Hibernate,
    Bin,
    Settings,
    Utility,
    User,
    Shield,
    Folder,
    FolderArrow,
    Copy,
    Hide,
}

fn action_mark(action: &Action) -> Option<Mark> {
    Some(match action {
        Action::System(command) => match command {
            SystemCommand::Restart => Mark::Restart,
            SystemCommand::Shutdown => Mark::Power,
            SystemCommand::AdvancedRestart => Mark::Recovery,
            SystemCommand::SignOut => Mark::Logout,
            SystemCommand::Lock => Mark::Lock,
            SystemCommand::Sleep => Mark::Moon,
            SystemCommand::Hibernate => Mark::Hibernate,
            SystemCommand::EmptyRecycleBin => Mark::Bin,
        },
        Action::Utility(command) => match command {
            UtilityCommand::RecycleBin => Mark::Bin,
            UtilityCommand::IndexOptions => Mark::Settings,
            UtilityCommand::WindowsSettings(SettingsPage::Update) => Mark::Recovery,
            UtilityCommand::LightTheme | UtilityCommand::DarkTheme => Mark::Moon,
            UtilityCommand::WindowsSettings(_)
            | UtilityCommand::ControlPanel
            | UtilityCommand::TaskManager => Mark::Utility,
            _ => return None,
        },
        Action::Settings => Mark::Settings,
        Action::Reindex => Mark::Recovery,
        Action::Quit => Mark::Logout,
        _ => return None,
    })
}

#[inline]
fn coord(tile: RECT, side: i32, x: i32, y: i32) -> POINT {
    let side = side.max(1);
    let left = tile.left + (tile.right - tile.left - side) / 2;
    let top = tile.top + (tile.bottom - tile.top - side) / 2;
    POINT {
        x: left + x * side / 32,
        y: top + y * side / 32,
    }
}

unsafe fn stroke(dc: HDC, tile: RECT, side: i32, points: &[(i32, i32)], close: bool) {
    if points.len() < 2 {
        return;
    }
    let first = coord(tile, side, points[0].0, points[0].1);
    MoveToEx(dc, first.x, first.y, null_mut());
    for &(x, y) in &points[1..] {
        let p = coord(tile, side, x, y);
        LineTo(dc, p.x, p.y);
    }
    if close {
        LineTo(dc, first.x, first.y);
    }
}

unsafe fn ellipse(dc: HDC, tile: RECT, side: i32, bounds: (i32, i32, i32, i32)) {
    let a = coord(tile, side, bounds.0, bounds.1);
    let b = coord(tile, side, bounds.2, bounds.3);
    Ellipse(dc, a.x, a.y, b.x, b.y);
}

unsafe fn arc(dc: HDC, tile: RECT, side: i32, cx: i32, cy: i32, radius: i32, start: i32, end: i32) {
    let mut points = Vec::with_capacity(((end - start).unsigned_abs() / 12 + 2) as usize);
    for degrees in (start..=end).step_by(12) {
        let radians = degrees as f64 * std::f64::consts::PI / 180.0;
        points.push((
            cx + (radians.cos() * radius as f64).round() as i32,
            cy + (radians.sin() * radius as f64).round() as i32,
        ));
    }
    let radians = end as f64 * std::f64::consts::PI / 180.0;
    points.push((
        cx + (radians.cos() * radius as f64).round() as i32,
        cy + (radians.sin() * radius as f64).round() as i32,
    ));
    stroke(dc, tile, side, &points, false);
}

unsafe fn draw_mark(dc: HDC, tile: RECT, side: i32, mark: Mark) {
    match mark {
        Mark::Power => {
            // A broken ring and independent stem read cleanly even at small sizes.
            arc(dc, tile, side, 16, 17, 11, 38, 245);
            arc(dc, tile, side, 16, 17, 11, 295, 398);
            stroke(dc, tile, side, &[(16, 4), (16, 17)], false);
        }
        Mark::Restart | Mark::Recovery => {
            arc(dc, tile, side, 16, 16, 11, 42, 330);
            stroke(dc, tile, side, &[(20, 7), (26, 10), (20, 14)], false);
            if matches!(mark, Mark::Recovery) {
                stroke(dc, tile, side, &[(16, 12), (16, 20)], false);
                stroke(dc, tile, side, &[(12, 16), (20, 16)], false);
            }
        }
        Mark::Logout => {
            stroke(
                dc,
                tile,
                side,
                &[(18, 5), (25, 5), (25, 27), (18, 27)],
                false,
            );
            stroke(dc, tile, side, &[(5, 16), (20, 16)], false);
            stroke(dc, tile, side, &[(14, 10), (20, 16), (14, 22)], false);
        }
        Mark::Lock => {
            stroke(
                dc,
                tile,
                side,
                &[(7, 15), (25, 15), (25, 28), (7, 28)],
                true,
            );
            arc(dc, tile, side, 16, 15, 7, 180, 360);
            stroke(dc, tile, side, &[(9, 15), (9, 12)], false);
            stroke(dc, tile, side, &[(23, 12), (23, 15)], false);
            ellipse(dc, tile, side, (14, 18, 18, 22));
            stroke(dc, tile, side, &[(16, 21), (16, 24)], false);
        }
        Mark::Moon => {
            // Two open curves form a crescent without relying on background color.
            arc(dc, tile, side, 15, 16, 11, 55, 305);
            arc(dc, tile, side, 22, 11, 8, 90, 270);
            stroke(dc, tile, side, &[(16, 5), (21, 4)], false);
            stroke(dc, tile, side, &[(8, 25), (12, 27)], false);
        }
        Mark::Hibernate => {
            arc(dc, tile, side, 14, 17, 10, 55, 305);
            arc(dc, tile, side, 20, 11, 7, 90, 270);
            stroke(
                dc,
                tile,
                side,
                &[(20, 20), (27, 20), (20, 27), (27, 27)],
                false,
            );
        }
        Mark::Bin => {
            stroke(
                dc,
                tile,
                side,
                &[(8, 10), (24, 10), (22, 27), (10, 27)],
                true,
            );
            stroke(dc, tile, side, &[(6, 7), (26, 7)], false);
            stroke(dc, tile, side, &[(12, 7), (13, 4), (19, 4), (20, 7)], false);
            stroke(dc, tile, side, &[(14, 14), (14, 23)], false);
            stroke(dc, tile, side, &[(18, 14), (18, 23)], false);
        }
        Mark::Settings => {
            ellipse(dc, tile, side, (10, 10, 22, 22));
            ellipse(dc, tile, side, (14, 14, 18, 18));
            for &(x1, y1, x2, y2) in &[
                (15, 3, 17, 8),
                (15, 24, 17, 29),
                (3, 15, 8, 17),
                (24, 15, 29, 17),
                (6, 6, 10, 10),
                (22, 22, 26, 26),
                (22, 6, 26, 10),
                (6, 22, 10, 26),
            ] {
                stroke(dc, tile, side, &[(x1, y1), (x2, y2)], false);
            }
        }
        Mark::Utility => {
            stroke(dc, tile, side, &[(5, 8), (27, 8)], false);
            stroke(dc, tile, side, &[(5, 16), (27, 16)], false);
            stroke(dc, tile, side, &[(5, 24), (27, 24)], false);
            ellipse(dc, tile, side, (10, 5, 16, 11));
            ellipse(dc, tile, side, (19, 13, 25, 19));
            ellipse(dc, tile, side, (7, 21, 13, 27));
        }
        Mark::User => {
            ellipse(dc, tile, side, (12, 4, 20, 12));
            arc(dc, tile, side, 16, 28, 12, 205, 335);
            stroke(dc, tile, side, &[(4, 28), (4, 25)], false);
            stroke(dc, tile, side, &[(28, 25), (28, 28)], false);
        }
        Mark::Shield => {
            stroke(
                dc,
                tile,
                side,
                &[
                    (16, 3),
                    (27, 7),
                    (26, 18),
                    (22, 25),
                    (16, 29),
                    (10, 25),
                    (6, 18),
                    (5, 7),
                ],
                true,
            );
            stroke(dc, tile, side, &[(11, 16), (15, 20), (22, 12)], false);
        }
        Mark::Folder | Mark::FolderArrow => {
            stroke(
                dc,
                tile,
                side,
                &[(4, 10), (13, 10), (16, 13), (28, 13), (26, 26), (5, 26)],
                true,
            );
            stroke(dc, tile, side, &[(4, 10), (4, 7), (13, 7), (16, 10)], false);
            if matches!(mark, Mark::FolderArrow) {
                stroke(dc, tile, side, &[(11, 20), (22, 20)], false);
                stroke(dc, tile, side, &[(18, 16), (22, 20), (18, 24)], false);
            }
        }
        Mark::Copy => {
            stroke(
                dc,
                tile,
                side,
                &[(10, 7), (23, 7), (23, 24), (10, 24)],
                true,
            );
            stroke(
                dc,
                tile,
                side,
                &[(7, 11), (6, 11), (6, 28), (19, 28), (19, 26)],
                false,
            );
        }
        Mark::Hide => {
            stroke(dc, tile, side, &[(5, 6), (27, 6), (27, 26), (5, 26)], true);
            stroke(dc, tile, side, &[(5, 11), (27, 11)], false);
            stroke(dc, tile, side, &[(11, 18), (21, 18)], false);
        }
    }
}

unsafe fn paint(dc: HDC, tile: RECT, size: i32, color: u32, mark: Mark) {
    if dc.is_null() {
        return;
    }
    let available = (tile.right - tile.left).min(tile.bottom - tile.top).max(1);
    let side = size.max(1).min(available);
    let saved = SaveDC(dc);
    let pen = CreatePen(PS_SOLID, (side / 16).max(1), color);
    if pen.is_null() {
        RestoreDC(dc, saved);
        return;
    }
    SelectObject(dc, pen);
    SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
    draw_mark(dc, tile, side, mark);
    RestoreDC(dc, saved);
    DeleteObject(pen);
}

/// Draw a supported launcher action and return whether it had an original icon.
pub(super) unsafe fn draw(dc: HDC, tile: RECT, size: i32, color: u32, action: &Action) -> bool {
    let Some(mark) = action_mark(action) else {
        return false;
    };
    paint(dc, tile, size, color, mark);
    true
}

/// Draw the native context-action icon: 1 Open, 2 Admin, 3 Different user,
/// 4 Containing folder, 5 Target folder, 6 Copy path, 7 Hide launcher.
pub(super) unsafe fn draw_context(dc: HDC, tile: RECT, size: i32, color: u32, id: usize) {
    let mark = match id {
        1 => Mark::FolderArrow,
        2 => Mark::Shield,
        3 => Mark::User,
        4 | 5 => Mark::Folder,
        6 => Mark::Copy,
        7 => Mark::Hide,
        _ => return,
    };
    paint(dc, tile, size, color, mark);
}
