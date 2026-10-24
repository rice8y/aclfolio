#!/usr/bin/env python3
"""Compile the pinned RNC grammar into the native validator's pattern data."""

import pathlib
import json
import rnc2rng
from lxml import etree

root = pathlib.Path(__file__).resolve().parent.parent
source = root / "artifacts/upstream/python/acl_anthology/data/schema.rnc"
xml = etree.fromstring(rnc2rng.dumps(rnc2rng.load(str(source))).encode())


def convert(e):
    tag = etree.QName(e).localname
    if tag in ["empty", "text"]:
        return {"op": tag}
    if tag == "ref":
        return {"op": "ref", "name": e.get("name")}
    if tag in ["element", "attribute"]:
        return {"op": tag, "name": e[0].text, "child": combine(e[1:])}
    if tag == "data":
        return {
            "op": "data",
            "name": e.get("type"),
            "params": {p.get("name"): p.text for p in e},
        }
    if tag == "value":
        return {"op": "value", "value": e.text}
    if tag in ["group", "interleave", "choice"]:
        return fold(tag, list(e))
    if tag in ["optional", "zeroOrMore", "oneOrMore"]:
        return {"op": tag, "child": combine(e)}
    raise ValueError(tag)


def fold(tag, children):
    if not children:
        return {"op": "empty"}
    if len(children) == 1:
        return convert(children[0])
    return {"op": tag, "left": convert(children[0]), "right": fold(tag, children[1:])}


def combine(children):
    return fold("group", list(children))


data = {e.get("name") if e.get("name") else "#start": combine(e) for e in xml}
(root / "data/schema.json").write_text(
    json.dumps(data, ensure_ascii=False, indent=2) + "\n"
)
(root / "data/schema.rnc").write_text(source.read_text())
