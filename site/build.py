#!/usr/bin/env python3
"""Builds site/dist/index.html: one self-contained page.

Reads site/src/index.html and replaces {{img:name}} with the WebP in
site/src/shots/name.webp and {{data:file}} with site/src/file, each as a
data URI, so the page needs no other file.

    python3 site/build.py
"""
import base64
import mimetypes
import pathlib
import re

SITE = pathlib.Path(__file__).resolve().parent
SRC = SITE / "src"
OUT = SITE / "dist" / "index.html"


def data_uri(path: pathlib.Path) -> str:
    kind = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
    return f"data:{kind};base64,{base64.b64encode(path.read_bytes()).decode()}"


def main() -> None:
    page = (SRC / "index.html").read_text(encoding="utf-8")
    page = re.sub(
        r"\{\{img:([a-z0-9-]+)\}\}",
        lambda m: data_uri(SRC / "shots" / f"{m.group(1)}.webp"),
        page,
    )
    page = re.sub(
        r"\{\{data:([A-Za-z0-9._-]+)\}\}", lambda m: data_uri(SRC / m.group(1)), page
    )
    left = re.findall(r"\{\{[^}]*\}\}", page)
    if left:
        raise SystemExit(f"unresolved placeholders: {left}")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(page, encoding="utf-8")
    print(f"{OUT.relative_to(SITE.parent)}: {OUT.stat().st_size / 1e6:.2f} MB")


if __name__ == "__main__":
    main()
