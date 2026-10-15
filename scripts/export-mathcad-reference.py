"""Export our synthetic probes and settled original results; never drives the GUI.

Only generated worksheet expressions and normalized output values/errors are
published. Original product packages, resource strings and schemas stay local.
"""
import argparse
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
from zipfile import ZipFile

WS = "http://schemas.mathsoft.com/worksheet50"
ML = "http://schemas.mathsoft.com/math50"
RESULT = "http://schemas.mathsoft.com/result10"


def value(node):
    kind = node.tag.split("}")[-1]
    if kind in ("real", "imag"):
        number = float(node.text)
        return {"kind": "scalar", "real": number if kind == "real" else 0.0,
                "imag": number if kind == "imag" else 0.0}
    if kind == "complex":
        parts = [value(child) for child in node]
        return {"kind": "scalar", "real": sum(p["real"] for p in parts),
                "imag": sum(p["imag"] for p in parts)}
    if kind == "matrix":
        return {"kind": "matrix", "rows": int(node.attrib["rows"]),
                "cols": int(node.attrib["cols"]), "elements": [value(c) for c in node]}
    raise ValueError(f"Unimplemented reference result {kind}; do not silently skip")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("probe_directory", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    base = args.probe_directory
    observations = json.loads((base / "observations.json").read_text(encoding="utf-8-sig"))
    manifest = {c["id"]: c for c in json.loads((base / "manifest.json").read_text())}
    cases = []
    for observation in observations:
        if observation.get("error") or not observation.get("settled"):
            raise ValueError(f"Unsettled/failed probe: {observation['id']}")
        case = manifest[observation["id"]]
        source = base / Path(case["input"]).name
        target = base / Path(case["output"]).name
        for path, key in ((source, "input_sha256"), (target, "output_sha256")):
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != observation[key].lower():
                raise ValueError(f"Evidence changed: {path}")
        with ZipFile(source) as archive:
            input_xml = archive.read("mathcad/worksheet.xml").decode("utf-8-sig")
            presentation_bytes = archive.read("mathcad/settings/presentation.xml")
            presentation = ET.fromstring(presentation_bytes)
            page = presentation.find("{http://schemas.ptc.com/mathcad/settings/presentation10}pageModel")
            # Publish only the page settings needed for original placement, not
            # the product's styles/fonts/resource bundle.
            settings = ET.Element("{http://schemas.ptc.com/mathcad/settings/presentation10}presentation")
            ET.SubElement(settings, "{http://schemas.ptc.com/mathcad/settings/presentation10}pageModel",
                          {key: page.attrib[key] for key in ("paper-code", "grid-size")})
        with ZipFile(target) as archive:
            saved_xml = archive.read("mathcad/worksheet.xml").decode("utf-8-sig")
        results = {}
        for item in observation["results"]:
            if item["status"] != "Synchronized":
                raise ValueError("Pending is never a reference answer")
            if item["result_id"] in results:
                raise ValueError("Duplicate result ID")
            results[item["result_id"]] = ET.fromstring(item["xml"])
        rows = []
        for region in ET.fromstring(saved_xml).find(f"{{{WS}}}regions"):
            math = region.find(f"{{{WS}}}math")
            # SaveAs can renumber BOTH region IDs and result IDs. Resolve from saved XML.
            ref = math.attrib["resultRef"]
            result = results[ref]
            answer = result.find(f"{{{ML}}}result")
            errors = []
            for error in result.findall(f"{{{RESULT}}}engineErrors/{{{RESULT}}}engineError"):
                errors.append({"code": error.findtext(f"{{{RESULT}}}errorCode"),
                               "previous_error_id": error.attrib.get("previous-error-id")})
            rows.append({"region_id": region.attrib["region-id"], "result_ref": ref,
                         "left": float(region.attrib["left"]), "top": float(region.attrib["top"]),
                         "status": "Synchronized", "value": value(answer[0]) if answer is not None else None,
                         "errors": errors,
                         "redefinition_warning": result.find(f"{{{RESULT}}}Warnings/{{{RESULT}}}Warning") is not None})
        cases.append({"id": case["id"], "question": case["question"],
                      "input_sha256": observation["input_sha256"].lower(),
                      "output_sha256": observation["output_sha256"].lower(),
                      "input_worksheet_xml": input_xml, "saved_worksheet_xml": saved_xml,
                      "presentation_xml": ET.tostring(settings, encoding="unicode"),
                      "presentation_sha256": hashlib.sha256(presentation_bytes).hexdigest(),
                      "expected": rows})
    payload = {"schema_version": 1, "product": "PTC Mathcad Express Prime", "version": "12.0.0.1",
               "scope": "Synthetic top-level worksheets; saved region/result IDs; values, error tags and redefinition flags. Not full product conformance.",
               "numeric_tolerance": {"absolute": 1e-12, "relative": 1e-8}, "cases": cases}
    args.destination.parent.mkdir(parents=True, exist_ok=True)
    args.destination.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Exported {len(cases)} settled cases -> {args.destination}")


if __name__ == "__main__":
    main()
