#!/usr/bin/env python3
"""Vendor locale definitions shipped with the pinned citeproc-py reference."""

import pathlib
import json
import citeproc
from lxml import etree

root = pathlib.Path(__file__).resolve().parent.parent
path = pathlib.Path(citeproc.__file__).parent / "data/locales"


def node(e):
    return {
        "tag": etree.QName(e).localname,
        "attrs": {etree.QName(k).localname: v for k, v in e.attrib.items()},
        "text": e.text or "",
        "children": [node(c) for c in e if isinstance(c.tag, str)],
    }


out = {
    "primary": citeproc.PRIMARY_DIALECTS,
    "locales": {
        p.stem.removeprefix("locales-"): node(etree.parse(str(p)).getroot())
        for p in sorted(path.glob("locales-*.xml"))
    },
}
(root / "data/csl-locales.json").write_text(
    json.dumps(out, ensure_ascii=False, separators=(",", ":")) + "\n"
)
