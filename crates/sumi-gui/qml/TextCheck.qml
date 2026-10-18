import QtQuick

// Debug aid for translations (SUMI_CHECK_TEXT=1, see Main.qml): walks the item tree and
// reports every visible text that does not fit where it is drawn:
//
//   elided      the text was cut and ends in "…" (Text.truncated)
//   wider       the text is wider than the box it was given
//   taller      the text needs more lines than its box has height for (4 px slack)
//   outside     the drawn text sticks out of its parent (a button, a tile, a row) or out of the
//               nearest clipping ancestor / the window
//   overlap     two visible texts are drawn over each other
//
// Each finding is one console line, `TEXTFIT <label> | <kind> | "<text>" | <path> | <sizes>`,
// so a run with nothing to report prints nothing. Items scrolled out of a Flickable are
// still checked (their size does not depend on where they scroll), items outside the window
// or fully transparent are not.
QtObject {
    id: check

    // Findings of the last run(): [{kind, text, path, detail}]
    property var findings: []
    // Skip texts whose box is narrower than this (Texts used as spacers).
    property real minBox: 2

    function typeName(item) {
        const m = String(item).match(/^([A-Za-z0-9]+?)(_QMLTYPE_\d+|_QML_\d+)?\(/);
        return m ? m[1].replace(/^QQuick/, "") : String(item);
    }

    function path(item) {
        const parts = [];
        let it = item;
        for (let i = 0; it && i < 6; i++) {
            parts.push(it.objectName !== "" && it.objectName !== undefined ? it.objectName : typeName(it));
            it = it.parent;
        }
        return parts.join(" < ");
    }

    function isText(item) {
        return item.contentWidth !== undefined && item.contentHeight !== undefined && typeof item.truncated === "boolean"
            && item.text !== undefined && item.font !== undefined && item.lineCount !== undefined;
    }

    function shown(item) {
        if (!item.visible || item.width <= 0 || item.height <= 0) {
            return false;
        }
        for (let it = item; it; it = it.parent) {
            if (it.opacity !== undefined && it.opacity < 0.05) {
                return false;
            }
        }
        return true;
    }

    // The span of the text actually drawn, in the item's own x coordinates.
    function drawn(item) {
        const cw = Math.min(item.contentWidth, 100000);
        let left = 0;
        const w = item.width;
        if (cw < w) {
            switch (item.horizontalAlignment) {
            case Text.AlignRight: left = w - cw; break;
            case Text.AlignHCenter: left = (w - cw) / 2; break;
            default: break;
            }
        } else if (item.horizontalAlignment === Text.AlignRight) {
            left = w - cw;
        }
        return [left, left + cw];
    }

    function add(list, label, kind, item, detail) {
        const key = kind + "|" + item.text + "|" + path(item);
        if (list.some(f => f.key === key)) {
            return;
        }
        const f = { key: key, kind: kind, text: String(item.text), path: path(item), detail: detail };
        list.push(f);
        console.log("TEXTFIT " + label + " | " + kind + " | \"" + f.text.replace(/\n/g, " ") + "\" | " + f.path + " | " + detail);
    }

    function collect(item, out) {
        if (!item) {
            return;
        }
        if (isText(item) && String(item.text) !== "" && shown(item)) {
            out.push(item);
        }
        const kids = item.children || [];
        for (let i = 0; i < kids.length; i++) {
            collect(kids[i], out);
        }
    }

    // The nearest ancestor that clips (not a Flickable: its content is meant to scroll).
    function clipper(item) {
        for (let it = item.parent; it; it = it.parent) {
            if (it.clip === true && it.contentWidth === undefined && it.width > 0) {
                return it;
            }
        }
        return null;
    }

    function round(v) {
        return Math.round(v * 10) / 10;
    }

    // The part of the text's box that can be seen: its rectangle in `root` coordinates cut down
    // by every clipping ancestor (a Flickable viewport, a page's clip) and the root itself.
    // null when nothing of it shows.
    function visibleRect(t, root) {
        const tl = t.mapToItem(root, 0, 0);
        const br = t.mapToItem(root, t.width, t.height);
        let r = { l: tl.x, t: tl.y, r: br.x, b: br.y };
        const clips = [root];
        for (let it = t.parent; it && it !== root; it = it.parent) {
            if (it.clip === true) {
                clips.push(it);
            }
        }
        for (const c of clips) {
            const a = c.mapToItem(root, 0, 0);
            const b = c.mapToItem(root, c.width, c.height);
            r = { l: Math.max(r.l, a.x), t: Math.max(r.t, a.y), r: Math.min(r.r, b.x), b: Math.min(r.b, b.y) };
        }
        return r.r - r.l > 1 && r.b - r.t > 1 ? r : null;
    }

    // Check everything under `root`; returns the number of findings.
    function run(root, label) {
        const texts = [];
        collect(root, texts);
        const list = [];
        const rects = [];
        for (const t of texts) {
            if (t.width < minBox || visibleRect(t, root) === null) {
                continue;   // not on screen: a scrolled-away row, a tile outside the strip
            }
            const sizes = "box " + round(t.width) + "x" + round(t.height) + ", text " + round(t.contentWidth) + "x" + round(t.contentHeight);
            if (t.truncated) {
                add(list, label, "elided", t, sizes);
            } else if (t.contentWidth > t.width + 0.5 && t.wrapMode === Text.NoWrap) {
                add(list, label, "wider", t, sizes);
            }
            if (t.contentHeight > t.height + 4) {
                add(list, label, "taller", t, sizes);
            }
            // Does the drawn text stay inside its parent?
            const span = drawn(t);
            const p = t.parent;
            if (p && p.width > 0 && !t.truncated) {
                if (t.x + span[0] < -1 || t.x + span[1] > p.width + 1) {
                    add(list, label, "outside", t, sizes + ", parent " + typeName(p) + " " + round(p.width) + " wide");
                }
            }
            const c = clipper(t);
            if (c) {
                const a = t.mapToItem(c, span[0], 0);
                const b = t.mapToItem(c, span[1], 0);
                if (a.x < -1 || b.x > c.width + 1) {
                    add(list, label, "outside", t, sizes + ", clipped by " + typeName(c) + " " + round(c.width) + " wide");
                }
            }
            const a = t.mapToItem(root, span[0], 0);
            const b = t.mapToItem(root, span[1], 0);
            if (a.x < -1 || b.x > root.width + 1) {
                add(list, label, "outside", t, sizes + ", window " + round(root.width) + " wide");
            }
            // For the overlap test: the drawn text, cut down to what shows.
            const v = visibleRect(t, root);
            const tl = t.mapToItem(root, span[0], 0);
            const dr = t.mapToItem(root, span[1], t.contentHeight);
            const q = { item: t, l: Math.max(tl.x, v.l), r: Math.min(dr.x, v.r), t: Math.max(tl.y, v.t), b: Math.min(dr.y, v.b) };
            if (q.r > q.l && q.b > q.t) {
                rects.push(q);
            }
        }
        // Texts drawn over each other.
        for (let i = 0; i < rects.length; i++) {
            for (let j = i + 1; j < rects.length; j++) {
                const p = rects[i];
                const q = rects[j];
                const w = Math.min(p.r, q.r) - Math.max(p.l, q.l);
                const h = Math.min(p.b, q.b) - Math.max(p.t, q.t);
                if (w > 3 && h > 3) {
                    add(list, label, "overlap", p.item, "with \"" + String(q.item.text).replace(/\n/g, " ") + "\" (" + path(q.item) + ")");
                }
            }
        }
        findings = list;
        return list.length;
    }
}
