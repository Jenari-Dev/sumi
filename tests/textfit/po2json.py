#!/usr/bin/env python3
"""po2json.py <file.po> -> JSON {msgid: msgstr} (no fuzzy/empty entries; plural: msgid->[0], msgid_plural->[1])."""
import json, re, sys

def unq(s):
    return json.loads(s) if s.startswith('"') else s

def parse(path):
    out, cur, field, fuzzy = {}, {}, None, False
    def flush():
        nonlocal cur, fuzzy
        mid = cur.get("msgid")
        if mid and not fuzzy:
            if "msgstr" in cur and cur["msgstr"]:
                out[mid] = cur["msgstr"]
            if cur.get("msgid_plural"):
                if cur.get("msgstr[0]"):
                    out[mid] = cur["msgstr[0]"]
                if cur.get("msgstr[1]"):
                    out[cur["msgid_plural"]] = cur["msgstr[1]"]
        cur, fuzzy = {}, False
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("#,") and "fuzzy" in line:
            fuzzy = True
            continue
        if line.startswith("#"):
            continue
        if not line.strip():
            flush(); field = None
            continue
        m = re.match(r'(msgctxt|msgid_plural|msgid|msgstr(?:\[\d+\])?)\s+(".*")$', line)
        if m:
            field = m.group(1)
            if field == "msgctxt" and cur.get("msgid") is not None:
                flush()
            if field == "msgid" and "msgid" in cur:
                flush()
            cur[field] = unq(m.group(2))
        elif line.startswith('"') and field:
            cur[field] = cur.get(field, "") + unq(line)
    flush()
    return out

if __name__ == "__main__":
    print(json.dumps(parse(sys.argv[1]), ensure_ascii=False))
