use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Master,
    Columns,
    Rows,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

// Integer partition boundaries preserve every pixel, including odd display sizes.
pub fn arrange(area: Rect, count: usize, layout: Layout, gap: i32, ratio: f32) -> Vec<Rect> {
    if count == 0 || count > 256 || area.width <= 0 || area.height <= 0 {
        return vec![];
    }
    let gap = gap
        .clamp(0, 100)
        .min((area.width - 1) / 2)
        .min((area.height - 1) / 2);
    let area = Rect {
        x: area.x + gap,
        y: area.y + gap,
        width: area.width - 2 * gap,
        height: area.height - 2 * gap,
    };
    let split = |area, n, vertical| split(area, n, vertical, gap);
    match layout {
        Layout::Columns => split(area, count, false),
        Layout::Rows => split(area, count, true),
        Layout::Master if count > 1 => {
            let ratio = if ratio.is_finite() {
                ratio.clamp(0.2, 0.8)
            } else {
                0.6
            };
            let divider = (area.width as f32 * ratio).round() as i32;
            let actual_gap = gap.min((area.width - 2).max(0));
            if area.width < 2 {
                return vec![];
            }
            let width = divider.clamp(1, area.width - actual_gap - 1);
            let mut result = vec![Rect { width, ..area }];
            result.extend(split(
                Rect {
                    x: area.x + width + actual_gap,
                    width: area.width - width - actual_gap,
                    ..area
                },
                count - 1,
                true,
            ));
            if result.len() == count {
                result
            } else {
                vec![]
            }
        }
        Layout::Master => vec![area],
        Layout::Grid => {
            let columns = (count as f64).sqrt().ceil() as usize;
            let rows = count.div_ceil(columns);
            let mut result = Vec::with_capacity(count);
            for row in split(area, rows, true) {
                result.extend(split(row, columns.min(count - result.len()), false));
            }
            if result.len() == count {
                result
            } else {
                vec![]
            }
        }
    }
}

fn split(area: Rect, count: usize, vertical: bool, gap: i32) -> Vec<Rect> {
    if count == 0 {
        return vec![];
    }
    let size = if vertical { area.height } else { area.width };
    if size < count as i32 {
        return vec![];
    }
    let gap = gap.min((size - count as i32) / (count as i32 - 1).max(1));
    let usable = size - gap * (count as i32 - 1);
    (0..count as i32)
        .map(|i| {
            let start = usable * i / count as i32 + gap * i;
            let length = usable * (i + 1) / count as i32 - usable * i / count as i32;
            if vertical {
                Rect {
                    y: area.y + start,
                    height: length,
                    ..area
                }
            } else {
                Rect {
                    x: area.x + start,
                    width: length,
                    ..area
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partitions_stay_in_bounds_and_never_overlap() {
        let area = Rect {
            x: -1920,
            y: -80,
            width: 1919,
            height: 1079,
        };
        for layout in [Layout::Master, Layout::Grid, Layout::Rows, Layout::Columns] {
            for n in 1..=128 {
                for gap in [0, 12, 100] {
                    let tiles = arrange(area, n, layout, gap, 0.6);
                    assert_eq!(tiles.len(), n);
                    for (i, a) in tiles.iter().enumerate() {
                        assert!(a.width > 0 && a.height > 0);
                        assert!(area.contains(a.x, a.y));
                        assert!(area.contains(a.x + a.width - 1, a.y + a.height - 1));
                        for b in &tiles[i + 1..] {
                            assert!(
                                a.x + a.width <= b.x
                                    || b.x + b.width <= a.x
                                    || a.y + a.height <= b.y
                                    || b.y + b.height <= a.y
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rejects_impossible_layouts() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert!(arrange(area, 2, Layout::Master, 100, 0.6).is_empty());
        assert!(arrange(area, 257, Layout::Grid, 0, 0.6).is_empty());
    }
}
