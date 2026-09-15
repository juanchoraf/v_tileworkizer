//! Per-monitor work areas must never remove a connected monitor from inventory.
use crate::layout::Rect;

fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let bottom =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (right > x && bottom > y).then_some(Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    })
}

pub(super) fn areas(values: &[u32]) -> Vec<Rect> {
    values
        .as_chunks::<4>()
        .0
        .iter()
        .filter_map(|a| {
            let area = Rect {
                x: a[0] as i32,
                y: a[1] as i32,
                width: i32::try_from(a[2]).ok()?,
                height: i32::try_from(a[3]).ok()?,
            };
            (area.width > 0 && area.height > 0).then_some(area)
        })
        .collect()
}

pub(super) fn apply(monitors: &mut [Rect], per_monitor: &[Rect], global: Option<Rect>) {
    for monitor in monitors {
        // Mutter's list can have a different order from RandR (primary first).
        let local = per_monitor
            .iter()
            .filter_map(|area| intersection(*monitor, *area))
            .max_by_key(|a| i64::from(a.width) * i64::from(a.height));
        *monitor = local
            .or_else(|| global.and_then(|a| intersection(*monitor, a)))
            .unwrap_or(*monitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gnome_workarea_does_not_discard_primary_display() {
        let mut monitors = [
            Rect {
                x: 3840,
                y: 2160,
                width: 3840,
                height: 2160,
            },
            Rect {
                x: 0,
                y: 0,
                width: 3840,
                height: 2160,
            },
            Rect {
                x: 0,
                y: 2160,
                width: 3840,
                height: 2160,
            },
        ];
        let global = Rect {
            x: 0,
            y: 0,
            width: 3840,
            height: 4320,
        };
        let per_monitor = areas(&[
            0, 0, 3840, 2160, 0, 2160, 3840, 2160, 3840, 2192, 3840, 2128,
        ]);
        apply(&mut monitors, &per_monitor, Some(global));
        assert_eq!(monitors[0], per_monitor[2]);
        assert_eq!(monitors[1], per_monitor[0]);
        assert_eq!(monitors[2], per_monitor[1]);
    }
    #[test]
    fn ten_displays_survive_partial_or_missing_desktop_workarea() {
        let original: Vec<_> = (-5..5)
            .map(|i| Rect {
                x: i * 1920,
                y: 0,
                width: 1920,
                height: 1080,
            })
            .collect();
        for global in [
            None,
            Some(Rect {
                x: 0,
                y: 24,
                width: 1920,
                height: 1056,
            }),
        ] {
            let mut monitors = original.clone();
            apply(&mut monitors, &[], global);
            assert_eq!(monitors.len(), 10);
            for (i, monitor) in monitors.iter().enumerate() {
                assert_eq!(monitor.x, original[i].x);
                assert!(monitor.width > 0 && monitor.height > 0);
            }
        }
    }
}
