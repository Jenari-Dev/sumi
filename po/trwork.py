#!/usr/bin/env python3
"""Translation workflow helper for po/<lang>.po (no dependencies).

  po/trwork.py init                 create missing po/<lang>.po from sumi.pot (LINGUAS)
  po/trwork.py export <lang>        untranslated/fuzzy entries -> po/work/<lang>.json
  po/trwork.py fill <lang> <file>   apply {"<n>": "translation"} from <file> (keyed by
                                    the export's "n") and normalise the .po
  po/trwork.py check [lang...]      placeholders, untranslated counts, msgfmt --check
"""
import json
import os
import re
import subprocess
import sys

PO = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(PO, "work")


def linguas():
    with open(os.path.join(PO, "LINGUAS"), encoding="utf-8") as f:
        return [w for line in f for w in line.split("#")[0].split()]


def unquote(s):
    s = s.strip()
    assert s.startswith('"') and s.endswith('"'), s
    out, i, body = [], 0, s[1:-1]
    while i < len(body):
        c = body[i]
        if c == "\\":
            n = body[i + 1]
            out.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(n, "\\" + n))
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def quote(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t") + '"'


class Entry:
    def __init__(self, lines):
        self.comments, self.fields, self.order = [], {}, []
        key = None
        for line in lines:
            if line.startswith("#"):
                self.comments.append(line)
                continue
            m = re.match(r"^(msgctxt|msgid|msgid_plural|msgstr(?:\[\d+\])?)\s+(\".*\")\s*$", line)
            if m:
                key = m.group(1)
                self.fields[key] = unquote(m.group(2))
                self.order.append(key)
            elif line.startswith('"') and key:
                self.fields[key] += unquote(line)
            else:
                raise ValueError(f"unexpected po line: {line!r}")

    @property
    def header(self):
        return self.fields.get("msgid") == "" and "msgctxt" not in self.fields

    @property
    def obsolete(self):
        return not self.fields and any(c.startswith("#~") for c in self.comments)

    @property
    def fuzzy(self):
        return any(c.startswith("#,") and "fuzzy" in c for c in self.comments)

    def todo(self):
        return not self.header and not self.obsolete and (self.fuzzy or not self.fields.get("msgstr"))

    def set_translation(self, text):
        self.fields["msgstr"] = text
        kept = []
        for c in self.comments:
            if c.startswith("#|"):
                continue
            if c.startswith("#,"):
                flags = [f.strip() for f in c[2:].split(",") if f.strip() and f.strip() != "fuzzy"]
                if not flags:
                    continue
                c = "#, " + ", ".join(flags)
            kept.append(c)
        self.comments = kept

    def render(self):
        return "\n".join(self.comments + [f"{k} {quote(self.fields[k])}" for k in self.order])


def read_po(path):
    with open(path, encoding="utf-8") as f:
        blocks = f.read().split("\n\n")
    return [Entry([l for l in b.splitlines() if l.strip()]) for b in blocks if b.strip()]


def write_po(path, entries):
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n\n".join(e.render() for e in entries) + "\n")
    # Normalise wrapping/escaping the gettext way.
    subprocess.run(["msgcat", "--width=79", "-o", path, path], check=True)


def placeholders(s):
    return sorted(re.findall(r"%\d", s))


def cmd_init():
    pot = os.path.join(PO, "sumi.pot")
    for lang in linguas():
        path = os.path.join(PO, f"{lang}.po")
        if os.path.exists(path):
            continue
        subprocess.run(["msginit", "--no-translator", "--no-wrap", "-l", f"{lang}.UTF-8", "-i", pot, "-o", path],
                       check=True, capture_output=True)
        # msginit fills the team/translator lines with placeholders; keep them generic.
        entries = read_po(path)
        head = entries[0]
        lines = head.fields["msgstr"].split("\n")
        lines = [l for l in lines if not l.startswith(("Last-Translator:", "Language-Team:", "Report-Msgid-Bugs-To:",
                                                     "POT-Creation-Date:", "PO-Revision-Date:"))]
        head.fields["msgstr"] = "\n".join(lines)
        head.comments = ["# Sumi translation.", "# This file is distributed under the same license as Sumi (GPL-2.0-or-later)."]
        write_po(path, entries)
        print(f"created po/{lang}.po")


def cmd_export(lang):
    entries = read_po(os.path.join(PO, f"{lang}.po"))
    out, n = [], 0
    for e in entries:
        if not e.todo():
            continue
        n += 1
        item = {"n": n, "id": e.fields["msgid"]}
        if "msgctxt" in e.fields:
            item["ctx"] = e.fields["msgctxt"]
        if e.fuzzy and e.fields.get("msgstr"):
            item["old_translation"] = e.fields["msgstr"]
        refs = [c[3:].split()[0] for c in e.comments if c.startswith("#: ")]
        if refs:
            item["where"] = refs[0].rsplit(":", 1)[0].rsplit("/", 1)[-1]
        notes = [c[3:] for c in e.comments if c.startswith("#. ")]
        if notes:
            item["note"] = " ".join(notes)
        out.append(item)
    os.makedirs(WORK, exist_ok=True)
    path = os.path.join(WORK, f"{lang}.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"lang": lang, "entries": out}, f, ensure_ascii=False, indent=1)
    print(f"{path}: {len(out)} entries to translate")


def cmd_fill(lang, answers_path):
    with open(os.path.join(WORK, f"{lang}.json"), encoding="utf-8") as f:
        work = {str(i["n"]): i for i in json.load(f)["entries"]}
    with open(answers_path, encoding="utf-8") as f:
        answers = json.load(f)
    path = os.path.join(PO, f"{lang}.po")
    entries = read_po(path)
    index = {(e.fields.get("msgctxt"), e.fields.get("msgid")): e for e in entries if not e.obsolete}
    applied, problems = 0, []
    for n, text in answers.items():
        item = work.get(str(n))
        if item is None:
            problems.append(f"#{n}: not in the export")
            continue
        e = index.get((item.get("ctx"), item["id"]))
        if e is None:
            problems.append(f"#{n}: msgid no longer in {lang}.po")
            continue
        if not isinstance(text, str) or not text.strip():
            problems.append(f"#{n}: empty translation")
            continue
        if placeholders(text) != placeholders(item["id"]):
            problems.append(f"#{n}: placeholders {placeholders(text)} != {placeholders(item['id'])}")
            continue
        e.set_translation(text)
        applied += 1
    write_po(path, entries)
    missing = sorted(set(work) - {str(k) for k in answers}, key=int)
    print(f"po/{lang}.po: applied {applied}/{len(work)}")
    if missing:
        print(f"  not answered: {', '.join(missing[:40])}{' …' if len(missing) > 40 else ''}")
    for p in problems:
        print("  " + p)
    return 0 if not problems else 1


def cmd_check(langs):
    bad = 0
    for lang in langs or linguas():
        path = os.path.join(PO, f"{lang}.po")
        if not os.path.exists(path):
            print(f"{lang}: missing")
            bad += 1
            continue
        entries = [e for e in read_po(path) if not e.header and not e.obsolete]
        todo = sum(e.todo() for e in entries)
        ph = [e.fields["msgid"] for e in entries
              if e.fields.get("msgstr") and placeholders(e.fields["msgstr"]) != placeholders(e.fields["msgid"])]
        r = subprocess.run(["msgfmt", "--check", "-o", "/dev/null", path], capture_output=True, text=True)
        ok = not todo and not ph and r.returncode == 0
        bad += not ok
        print(f"{lang}: {len(entries) - todo}/{len(entries)} translated"
              + (f", {len(ph)} placeholder mismatches" if ph else "")
              + ("" if r.returncode == 0 else f", msgfmt: {r.stderr.strip()}"))
    return 1 if bad else 0


def main(argv):
    if len(argv) >= 1 and argv[0] == "init":
        return cmd_init()
    if len(argv) == 2 and argv[0] == "export":
        return cmd_export(argv[1])
    if len(argv) == 3 and argv[0] == "fill":
        return cmd_fill(argv[1], argv[2])
    if len(argv) >= 1 and argv[0] == "check":
        return cmd_check(argv[1:])
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]) or 0)
