"""xfl_tree.py <xfl dir> [max depth]: tree of NAMED instances of an FFDec XFL export (main timeline down), with frame
labels and the frame range where each instance exists. Unnamed instances are followed but not printed."""
import os, re, sys
import xml.etree.ElementTree as ET

root_dir = sys.argv[1]
max_depth = int(sys.argv[2]) if len(sys.argv) > 2 else 8
NS = "{http://ns.adobe.com/xfl/2008/}"

def timeline_of(path):
    t = ET.parse(path).getroot()
    return t.find(f".//{NS}DOMTimeline")

def library(name):
    p = os.path.join(root_dir, "LIBRARY", name + ".xml")
    return timeline_of(p) if os.path.exists(p) else None

def walk(tl, depth, seen):
    if tl is None or depth > max_depth: return
    for layer in tl.iter(f"{NS}DOMLayer"):
        for fr in layer.iter(f"{NS}DOMFrame"):
            idx, dur = int(fr.get("index", 0)), int(fr.get("duration", 1))
            if fr.get("name"): print("  " * depth + f"[label '{fr.get('name')}' @ {idx}]")
            for inst in fr.iter(f"{NS}DOMSymbolInstance"):
                lib, nm = inst.get("libraryItemName"), inst.get("name")
                key = (lib, nm, depth)
                if key in seen: continue
                seen.add(key)
                if nm: print("  " * depth + f"{nm}  <{lib}>  frames {idx}-{idx + dur - 1}")
                walk(library(lib), depth + (1 if nm else 0), seen)
            for tf in fr.iter(f"{NS}DOMDynamicText"):
                if tf.get("name") and (tf.get("name"), depth) not in seen:
                    seen.add((tf.get("name"), depth)); print("  " * depth + f"{tf.get('name')}  (text)")

walk(timeline_of(os.path.join(root_dir, "DOMDocument.xml")), 0, set())
