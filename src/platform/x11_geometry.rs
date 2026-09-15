use crate::layout::Rect;

/// Convert visible tile bounds to X11 client geometry for StaticGravity.
/// _NET_FRAME_EXTENTS adds WM decorations outside the client rectangle;
/// _GTK_FRAME_EXTENTS describes invisible margins inside that rectangle.
pub(super) fn client_rect(target: Rect, frame: &[u32], shadow: &[u32]) -> Rect {
    let [left, right, top, bottom] = extents(frame);
    let [shadow_left, shadow_right, shadow_top, shadow_bottom] = extents(shadow);
    Rect {
        x: target.x.saturating_add(left).saturating_sub(shadow_left),
        y: target.y.saturating_add(top).saturating_sub(shadow_top),
        width: target
            .width
            .saturating_sub(left + right)
            .saturating_add(shadow_left + shadow_right)
            .max(1),
        height: target
            .height
            .saturating_sub(top + bottom)
            .saturating_add(shadow_top + shadow_bottom)
            .max(1),
    }
}

fn extents(values: &[u32]) -> [i32; 4] {
    if let [left, right, top, bottom] = values
        && values.iter().all(|value| *value <= u16::MAX.into())
    {
        [*left as i32, *right as i32, *top as i32, *bottom as i32]
    } else {
        [0; 4]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Rect {
        Rect {
            x: -1000,
            y: 0,
            width: 600,
            height: 1080,
        }
    }

    #[test]
    fn terminal_shadows_extend_outside_the_visible_tile() {
        let client = client_rect(target(), &[], &[26, 26, 23, 29]);
        assert_eq!(
            client,
            Rect {
                x: -1026,
                y: -23,
                width: 652,
                height: 1132
            }
        );
        assert_eq!(client.x + 26, target().x);
        assert_eq!(client.y + 23, target().y);
        assert_eq!(client.x + client.width - 26, target().x + target().width);
        assert_eq!(client.y + client.height - 29, target().y + target().height);
    }

    #[test]
    fn server_title_bar_stays_inside_the_tile() {
        assert_eq!(
            client_rect(target(), &[0, 0, 37, 0], &[]),
            Rect {
                x: -1000,
                y: 37,
                width: 600,
                height: 1043
            }
        );
    }

    #[test]
    fn undecorated_windows_and_invalid_properties_do_not_add_gaps() {
        assert_eq!(client_rect(target(), &[], &[]), target());
        assert_eq!(client_rect(target(), &[u32::MAX; 4], &[1, 2, 3]), target());
    }
}
