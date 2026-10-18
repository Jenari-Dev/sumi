//! MultiFrame window layouts (DisplayWidget's 13 zone layouts), expressed as
//! KWin tile trees. KWin snaps windows into tiles natively (Shift+drag), which
//! replaces DisplayWidget's global mouse hook and drop overlay.

use serde::Serialize;

/// KWin `Tile.LayoutDirection`: children side by side / stacked.
const H: u8 = 1;
const V: u8 = 2;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TileNode {
    /// Output-relative rectangle [x, y, w, h] in 0..1.
    pub rect: [f64; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TileNode>,
}

/// A split part: its share of the parent, and an optional nested split.
struct Part(f64, Option<(u8, Vec<Part>)>);

fn split(rect: [f64; 4], dir: u8, parts: Vec<Part>) -> TileNode {
    let [x, y, w, h] = rect;
    let mut pos = 0.0;
    let children = parts
        .into_iter()
        .map(|Part(frac, sub)| {
            let r = if dir == H { [x + pos * w, y, frac * w, h] } else { [x, y + pos * h, w, frac * h] };
            pos += frac;
            match sub {
                Some((d, p)) => split(r, d, p),
                None => TileNode { rect: r, dir: None, children: Vec::new() },
            }
        })
        .collect();
    TileNode { rect, dir: Some(dir), children }
}

fn p(f: f64) -> Part {
    Part(f, None)
}

fn halves(dir: u8) -> Option<(u8, Vec<Part>)> {
    Some((dir, vec![p(0.5), p(0.5)]))
}

/// Layout names in DisplayWidget's numbering (1–13).
pub const LAYOUTS: &[(u8, &str)] = &[
    (1, "Top / bottom"),
    (2, "Left / right"),
    (3, "Wide left, narrow right"),
    (4, "Narrow left, wide right"),
    (5, "Tall top, short bottom"),
    (6, "Short top, tall bottom"),
    (7, "Two left, one right"),
    (8, "One left, two right"),
    (9, "One top, two bottom"),
    (10, "Two top, one bottom"),
    (11, "Three columns"),
    (12, "Three rows"),
    (13, "Four quadrants"),
];

/// The tile tree for a layout on an output of the given aspect ratio
/// (width / height). Ultrawide screens get DisplayWidget's adjusted splits.
pub fn layout(n: u8, aspect: f64) -> Option<TileNode> {
    let full = [0.0, 0.0, 1.0, 1.0];
    let ultrawide = (2.2..3.4).contains(&aspect); // 21:9
    let super_ultrawide = aspect >= 3.4; // 32:9
    let (big, small) = if ultrawide { (16.0 / 21.0, 5.0 / 21.0) } else { (2.0 / 3.0, 1.0 / 3.0) };
    Some(match n {
        1 => split(full, V, vec![p(0.5), p(0.5)]),
        2 => split(full, H, vec![p(0.5), p(0.5)]),
        3 => split(full, H, vec![p(big), p(small)]),
        4 => split(full, H, vec![p(small), p(big)]),
        5 => split(full, V, vec![p(2.0 / 3.0), p(1.0 / 3.0)]),
        6 => split(full, V, vec![p(1.0 / 3.0), p(2.0 / 3.0)]),
        7 => split(full, H, vec![Part(0.5, halves(V)), p(0.5)]),
        8 => split(full, H, vec![p(0.5), Part(0.5, halves(V))]),
        9 => split(full, V, vec![p(0.5), Part(0.5, halves(H))]),
        10 => split(full, V, vec![Part(0.5, halves(H)), p(0.5)]),
        11 if super_ultrawide => split(full, H, vec![p(0.25), p(0.5), p(0.25)]),
        11 => split(full, H, vec![p(1.0 / 3.0), p(1.0 / 3.0), p(1.0 / 3.0)]),
        12 => split(full, V, vec![p(1.0 / 3.0), p(1.0 / 3.0), p(1.0 / 3.0)]),
        13 => split(full, H, vec![Part(0.5, halves(V)), Part(0.5, halves(V))]),
        _ => return None,
    })
}

/// Leaf rectangles of a tree, depth-first (KWin reports them in this order).
pub fn leaves(node: &TileNode) -> Vec<[f64; 4]> {
    if node.children.is_empty() {
        return vec![node.rect];
    }
    node.children.iter().flat_map(leaves).collect()
}

/// Which layout a set of leaf rectangles corresponds to (within 1 %).
pub fn identify(found: &[[f64; 4]], aspect: f64) -> Option<u8> {
    let close = |a: &[f64; 4], b: &[f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01);
    LAYOUTS.iter().map(|(n, _)| *n).find(|&n| {
        let want = leaves(&layout(n, aspect).expect("known layout"));
        want.len() == found.len() && want.iter().zip(found).all(|(a, b)| close(a, b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_layouts_tile_the_screen() {
        for aspect in [16.0 / 9.0, 21.0 / 9.0, 32.0 / 9.0, 9.0 / 16.0] {
            for (n, _) in LAYOUTS {
                let l = leaves(&layout(*n, aspect).unwrap());
                let area: f64 = l.iter().map(|r| r[2] * r[3]).sum();
                assert!((area - 1.0).abs() < 1e-9, "layout {n} at {aspect} covers {area}");
                assert_eq!(identify(&l, aspect), Some(*n));
            }
        }
    }

    #[test]
    fn matches_kwin_readback() {
        // Leaves as KWin reported them for layout 7 in a virtual KWin session.
        let kwin = [[0.0, 0.0, 0.5, 0.5], [0.0, 0.5, 0.5, 0.5], [0.5, 0.0, 0.5, 1.0]];
        assert_eq!(identify(&kwin, 16.0 / 9.0), Some(7));
        assert_eq!(layout(3, 21.0 / 9.0).unwrap().children[0].rect[2], 16.0 / 21.0);
    }
}
