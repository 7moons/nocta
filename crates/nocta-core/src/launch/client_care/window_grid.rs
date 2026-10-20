//! Where client windows go when they are arranged: a grid on each chosen monitor.
use crate::launch::instances::Monitor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Roblox's window shape.
const TARGET_ASPECT: f64 = 16.0 / 9.0;

/// The column count whose grid leaves the fewest empty cells, then whose cells are closest to
/// 16:9.
fn best_columns(monitor: &Monitor, windows: usize) -> usize {
    (1..=windows)
        .min_by(|a, b| {
            let score = |columns: usize| {
                let rows = windows.div_ceil(columns);
                let empty = (columns * rows - windows) as f64;
                let aspect = (monitor.width as f64 / columns as f64)
                    / (monitor.height.max(1) as f64 / rows as f64);
                empty * 0.5 + (aspect / TARGET_ASPECT).ln().abs()
            };
            score(*a).total_cmp(&score(*b))
        })
        .unwrap_or(1)
}

/// Spreads `count` windows over `monitors` in proportion to their area, and fills each monitor
/// with a grid of equal cells.
pub fn grid(monitors: &[Monitor], count: usize) -> Vec<Cell> {
    if monitors.is_empty() || count == 0 {
        return Vec::new();
    }
    let areas: Vec<u64> = monitors
        .iter()
        .map(|monitor| monitor.width as u64 * monitor.height as u64)
        .collect();
    let total: u64 = areas.iter().sum::<u64>().max(1);
    // Largest-remainder split of the windows between monitors.
    let mut shares: Vec<(usize, usize, u64)> = areas
        .iter()
        .enumerate()
        .map(|(index, area)| {
            let exact = count as u64 * area;
            (index, (exact / total) as usize, exact % total)
        })
        .collect();
    let mut left = count - shares.iter().map(|share| share.1).sum::<usize>();
    let mut by_remainder: Vec<usize> = (0..shares.len()).collect();
    by_remainder.sort_by_key(|index| std::cmp::Reverse(shares[*index].2));
    for index in by_remainder {
        if left == 0 {
            break;
        }
        shares[index].1 += 1;
        left -= 1;
    }
    let mut cells = Vec::with_capacity(count);
    for (index, windows, _) in shares {
        if windows == 0 {
            continue;
        }
        let monitor = &monitors[index];
        let columns = best_columns(monitor, windows);
        let rows = windows.div_ceil(columns);
        let width = monitor.width / columns as u32;
        let height = monitor.height / rows as u32;
        for slot in 0..windows {
            cells.push(Cell {
                x: monitor.x + (slot % columns) as i32 * width as i32,
                y: monitor.y + (slot / columns) as i32 * height as i32,
                width,
                height,
            });
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, width: u32, height: u32) -> Monitor {
        Monitor {
            x,
            y: 0,
            width,
            height,
            primary: x == 0,
        }
    }

    #[test]
    fn four_windows_fill_one_screen_in_a_two_by_two_grid() {
        let cells = grid(&[monitor(0, 1920, 1040)], 4);
        assert_eq!(
            cells,
            vec![
                Cell {
                    x: 0,
                    y: 0,
                    width: 960,
                    height: 520
                },
                Cell {
                    x: 960,
                    y: 0,
                    width: 960,
                    height: 520
                },
                Cell {
                    x: 0,
                    y: 520,
                    width: 960,
                    height: 520
                },
                Cell {
                    x: 960,
                    y: 520,
                    width: 960,
                    height: 520
                },
            ]
        );
    }

    #[test]
    fn windows_are_shared_between_monitors_by_area_and_never_overlap() {
        let screens = [monitor(0, 1920, 1080), monitor(1920, 1280, 1024)];
        let cells = grid(&screens, 10);
        assert_eq!(cells.len(), 10);
        let on_first = cells.iter().filter(|cell| cell.x < 1920).count();
        assert_eq!(on_first, 6, "the bigger screen gets more windows");
        for (index, a) in cells.iter().enumerate() {
            for b in &cells[index + 1..] {
                let overlap = a.x < b.x + b.width as i32
                    && b.x < a.x + a.width as i32
                    && a.y < b.y + b.height as i32
                    && b.y < a.y + a.height as i32;
                assert!(!overlap, "{a:?} overlaps {b:?}");
            }
        }
        assert!(grid(&screens, 0).is_empty());
        assert!(grid(&[], 3).is_empty());
    }
}
