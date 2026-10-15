"""Extract binding facts from REA-identified installed module interfaces.

Requires dnfile only for supplemental blob access. Verifies both complete PE
and attribute blob SHA-256 against REA before decoding. Original interface
text is written only under the caller's ignored evidence directory.
No assembly is loaded and no proprietary implementation enters the catalog.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

import dnfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compressed(data, position):
    first = data[position]
    if first < 0x80:
        return first, position + 1
    if first < 0xC0:
        return ((first & 63) << 8) | data[position + 1], position + 2
    if first < 0xE0:
        return ((first & 31) << 24) | int.from_bytes(data[position + 1:position + 4], "big"), position + 4
    raise ValueError("Unsupported compressed string length")


def arity(signature):
    # Function arguments may themselves contain arrows inside parentheses.
    level = 0
    count = 0
    for position, char in enumerate(signature):
        if char == "(":
            level += 1
        elif char == ")":
            level -= 1
        elif char == "-" and signature[position:position + 2] == "->" and level == 0:
            count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    records = json.loads(args.evidence.read_text(encoding="utf-8-sig"))
    modules = []
    bindings = []
    for record in records:
        artifact = record["artifact"]
        data = Path(artifact["path"]).read_bytes()
        if digest(data) != artifact["sha256"]:
            raise ValueError("Artifact changed: " + artifact["path"])
        pe = dnfile.dnPE(data=data)
        attr = next(a for a in record["attributes"] if a["type_name"] == "Mpl.Run.ModuleInterfaceAttribute")
        index = int(attr["token"], 16) & 0xFFFFFF
        blob = pe.net.mdtables.CustomAttribute.rows[index - 1].Value.value
        if len(blob) != attr["value_length"] or digest(blob) != attr["value_sha256"]:
            raise ValueError("REA attribute identity mismatch")
        if blob[:2] != b"\x01\x00":
            raise ValueError("Unsupported attribute format")
        size, start = compressed(blob, 2)
        interface = blob[start:start + size].decode("utf-8")
        if len(blob[start:start + size]) != size:
            raise ValueError("Truncated interface")
        (args.evidence.parent / (record["module"] + "-interface.txt")).write_text(interface, encoding="utf-8")
        module = dict(module=record["module"], sha256=artifact["sha256"], mvid=record["module_identity"]["mvid"],
                      evidence_id=record["evidence_id"], attribute_token=attr["token"], attribute_sha256=attr["value_sha256"])
        modules.append(module)
        # Only names, routing, arity and dimension-variable facts; never bodies.
        for match in re.finditer(r"^\s*([\w%]+)\s*::\s*(.*?)\s*@\s*(\"[^\"]*\"|\d+)", interface, re.M):
            name, signature, target = match.groups()
            if not name.startswith(("mc_", "op_")):
                continue
            bindings.append(dict(module=record["module"], binding=name,
                                 kind="operator" if name.startswith("op_") else "function" if arity(signature) else "value",
                                 name=name[3:], engine_arity=arity(signature),
                                 optional_arguments="McdRun.D " in signature,
                                 variadic="McdRun.N " in signature,
                                 origin_argument="McdRun.O" in signature,
                                 dimension_variables=sorted(set(re.findall(r"\{([a-z](?:\s+-?\d+)?)\}", signature))),
                                 target=target.strip('"'), evidence_id=record["evidence_id"]))
        pe.close()
    result = dict(target="Mathcad Prime 12.0.0.1", method="REA-identified ModuleInterfaceAttribute blobs; SHA-verified supplemental decode",
                  executed_target=False,
                  limitations=["Binding inventory does not prove numerical correctness or complete feature coverage.",
                               "Engine arity includes implicit ORIGIN and packed optional/variadic arguments; it is not user-call arity.",
                               "Symbolic-only bindings, compatibility names and internal operators remain present.",
                               "Only the explicitly recorded modules are covered."],
                  modules=modules, bindings=bindings)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dict(modules=len(modules), bindings=len(bindings), functions=sum(x["kind"] == "function" for x in bindings)), ensure_ascii=False))


if __name__ == "__main__":
    main()
