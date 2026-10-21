//! Monitor-relative geometry. Negative secondary-monitor coordinates are valid.
use super::*;

pub(super) unsafe fn work_area(window: HWND) -> RECT {
    let monitor = if window.is_null() {
        let mut cursor: POINT = zeroed();
        GetCursorPos(&mut cursor);
        MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
    } else {
        MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST)
    };
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) != 0 {
        info.rcWork
    } else {
        let mut work: RECT = zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
        work
    }
}

pub(super) fn centered(work: RECT, width: i32, height: i32, margin: i32) -> RECT {
    let width = width.min((work.right - work.left - 2 * margin).max(1));
    let height = height.min((work.bottom - work.top - 2 * margin).max(1));
    let left = work.left + (work.right - work.left - width) / 2;
    let top = work.top + (work.bottom - work.top - height) / 3;
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_on_negative_coordinate_secondary_monitor() {
        let work = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        let placed = centered(work, 640, 352, 12);
        assert_eq!(placed.left, -1280);
        assert!(placed.top >= work.top && placed.bottom <= work.bottom);
        assert!(placed.right <= 0);
    }

    #[test]
    fn small_work_area_constrains_both_dimensions() {
        let work = RECT {
            left: 1920,
            top: 300,
            right: 2420,
            bottom: 600,
        };
        let placed = centered(work, 1280, 704, 12);
        assert_eq!(placed.right - placed.left, 476);
        assert_eq!(placed.bottom - placed.top, 276);
        assert!(placed.left >= work.left && placed.bottom <= work.bottom);
    }
}
