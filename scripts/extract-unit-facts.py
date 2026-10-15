"""Fold an authenticated, bounded numeric CIL subset without loading a CLR.

Reads REA evidence and original PE bytes. Emits independently derived numeric
facts and a Rust lookup table, never original instructions or proprietary source.
"""
import argparse
import hashlib
import json
import math
import pathlib
import re
import struct

ROOT = pathlib.Path(__file__).resolve().parents[1]
DIMENSIONS = ["Length", "Mass", "Time", "Current", "Temperature", "Substance", "Luminosity"]
PHYSICAL_CONSTANTS = {"e.c", "h", "ℏ", "k", "m.u", "N.A", "R.gas", "R.∞", "α", "Φ.0", "σ", "μ.0", "ε.0", "a.0", "c"}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compressed(data, offset):
    head = data[offset]
    if head < 128:
        return head, offset + 1
    if head < 192:
        return ((head & 63) << 8) | data[offset + 1], offset + 2
    if head < 224:
        return ((head & 31) << 24) | int.from_bytes(data[offset + 1:offset + 4], "big"), offset + 4
    raise ValueError("Invalid compressed integer")


class StaticModule:
    def __init__(self, evidence):
        self.evidence = json.loads(evidence.read_text(encoding="utf-8-sig"))
        self.result = self.evidence["normalized_result"]
        self.data = pathlib.Path(self.result["artifact"]["path"]).read_bytes()
        assert digest(self.data) == self.result["artifact"]["sha256"], "Artifact differs from REA"
        self.methods = {m["token"]: m for m in self.result["methods"]}
        self.fields = {f["token"]: f for f in self.result["fields"]}
        self.refs = {m["token"]: m for m in self.result["member_refs"]}
        self.strings = {s["token"]: s["value"] for s in self.result["user_strings"]["literals"]}
        self.values = {}
        self.assignments = {}
        self.decoded = {}

    def instructions(self, token):
        if token in self.decoded:
            return self.decoded[token]
        body = self.methods[token]["body"]
        assert body["status"] == "present", "Method body unavailable"
        start = body["file_offset"]
        header = 1 if self.data[start] & 3 == 2 else (int.from_bytes(self.data[start:start + 2], "little") >> 12) * 4
        code = self.data[start + header:start + header + body["il_size"]]
        assert digest(code) == body["il_sha256"], "CIL bytes differ from REA"
        result = []
        p = 0
        while p < len(code):
            at = p
            op = code[p]
            p += 1
            value = None
            if op == 0xFE:
                assert code[p] == 0x14, "Only tail. is admitted as a two-byte opcode"
                p += 1
                name = "tail"
            elif op == 0x23:
                name, value = "number", struct.unpack_from("<d", code, p)[0]
                p += 8
            elif op in [0x28, 0x72, 0x73, 0x7E, 0x80, 0x8C, 0x8D]:
                name = {0x28: "call", 0x72: "string", 0x73: "newobj", 0x7E: "load", 0x80: "store", 0x8C: "box", 0x8D: "array"}[op]
                value = f"0x{int.from_bytes(code[p:p + 4], 'little'):08x}"
                p += 4
            elif 0x16 <= op <= 0x1E:
                name, value = "number", op - 0x16
            elif 0x02 <= op <= 0x05:
                name, value = "argument", op - 0x02
            else:
                name = {0x2A: "return", 0x25: "dup", 0x14: "null", 0xA2: "array_store"}.get(op)
                assert name is not None, f"Unsupported CIL opcode {op:x} at {at}"
            result.append((at, name, value))
        self.decoded[token] = result
        return result

    def fold(self, token, base=None, args=(), depth=0):
        assert depth < 12, "Static helper recursion budget exceeded"
        stack = []
        for offset, op, operand in self.instructions(token):
            if op == "number":
                stack.append(operand)
            elif op == "argument":
                stack.append(args[operand])
            elif op in ("box", "tail"):
                pass
            elif op == "load":
                field = self.fields.get(operand) or self.refs[operand]
                owner = field.get("declaring_type", field.get("parent_name"))
                if owner == "McdStaticUnitSystem.values" and base is not None:
                    stack.append(base.values[field["name"]])
                elif owner in ("McdStaticUnitSystem.values", "McdStaticUnitSystemSI.values"):
                    stack.append(self.values[field["name"]])
                elif owner == "Mcd.NumVal":
                    stack.append({"unfolded": owner + "." + field["name"]})
                else:
                    raise ValueError("Unadmitted static field " + str(field))
            elif op == "store":
                field = self.fields[operand]
                self.values[field["name"]] = stack.pop()
                self.assignments[field["name"]] = {"field_token": operand, "method_token": token, "il_offset": offset, "method_il_sha256": self.methods[token]["body"]["il_sha256"]}
            elif op == "string":
                stack.append(self.strings[operand])
            elif op == "null":
                stack.append(None)
            elif op == "array":
                count = stack.pop()
                assert isinstance(count, int) and 0 <= count <= 8
                stack.append([None] * count)
            elif op == "dup":
                stack.append(stack[-1])
            elif op == "array_store":
                value, index, target = stack.pop(), stack.pop(), stack.pop()
                target[index] = value
            elif op in ("call", "newobj"):
                method = self.methods.get(operand) or self.refs[operand]
                owner = method.get("declaring_type", method.get("parent_name"))
                name = method["name"]
                count = method["signature"]["parameter_count"]
                arguments = stack[-count:] if count else []
                if count:
                    del stack[-count:]
                if owner == "MplPrelude.values" and name in ("*.", "/.", "sqrt"):
                    assert all(isinstance(v, (int, float)) for v in arguments)
                    value = arguments[0] * arguments[1] if name == "*." else arguments[0] / arguments[1] if name == "/." else math.sqrt(arguments[0])
                elif owner == "McdStaticUnitSystem.values":
                    target = base or self
                    selected = next(m["token"] for m in target.methods.values() if m["name"] == name)
                    value = target.fold(selected, args=arguments, depth=depth + 1)
                elif owner == "McdRun.values" and name == "srv":
                    assert len(arguments) == 1
                    value = arguments[0]
                elif (owner == "McdRun.values" and name in ("unit", "unit_system", "unit_system_group")) or (owner == "Mpl.Run.Cons" and name == ".ctor"):
                    # Presentation/system objects never become numeric facts.
                    value = {"unfolded": owner + "." + name}
                else:
                    raise ValueError("Unadmitted call " + owner + "." + name)
                stack.append(value)
            elif op == "return":
                assert len(stack) <= 1, "Unbalanced static evaluation stack"
                return stack[0] if stack else None
        raise ValueError("No return instruction")


def authenticated_interface(module, evidence_path):
    evidence = json.loads(evidence_path.read_text(encoding="utf-8-sig"))
    result = evidence["normalized_result"]
    assert result["artifact"]["sha256"] == module.result["artifact"]["sha256"]
    attribute = next(a for a in result["attributes"] if a["type_name"] == "Mpl.Run.ModuleInterfaceAttribute")
    b = module.data
    u = lambda at, n: int.from_bytes(b[at:at+n], "little")
    pe = u(0x3C, 4)
    optional = pe + 24
    sec = optional + u(pe + 20, 2)
    sections = [(u(sec+i*40+12,4),u(sec+i*40+8,4),u(sec+i*40+20,4),u(sec+i*40+16,4)) for i in range(u(pe+6,2))]
    def off(rva):
        return next(raw + rva - va for va, vs, raw, rs in sections if va <= rva < va + max(vs, rs))
    directories = optional + (112 if u(optional, 2) == 0x20B else 96)
    cli = off(u(directories + 14 * 8, 4))
    metadata = off(u(cli + 8, 4))
    p = (metadata + 16 + u(metadata + 12, 4) + 3) & ~3
    count = u(p + 2, 2)
    p += 4
    streams = {}
    for _ in range(count):
        relative = u(p, 4)
        p += 8
        end = b.index(0, p)
        streams[b[p:end].decode("ascii")] = metadata + relative
        p = (end + 4) & ~3
    table = streams["#~"]
    heap = b[table + 6]
    valid = u(table + 8, 8)
    p = table + 24
    rows = {}
    for i in range(64):
        if valid >> i & 1:
            rows[i] = u(p, 4)
            p += 4
    def coded(tables, bits):
        return 4 if max(rows.get(t, 0) for t in tables) >= (1 << (16-bits)) else 2
    parent = coded([6,4,1,2,8,9,10,0,14,23,20,17,26,27,32,35,38,39,40,42,44,43],5)
    constructor = coded([6,10],3)
    blob_index = u(attribute["row_offset"] + parent + constructor, 4 if heap & 4 else 2)
    length, start = compressed(b, streams["#Blob"] + blob_index)
    payload = b[start:start+length]
    assert len(payload) == attribute["value_length"] and digest(payload) == attribute["value_sha256"], "Interface attribute differs from REA"
    assert payload[:2] == b"\x01\x00"
    length, start = compressed(payload, 2)
    text = payload[start:start+length].decode("utf-8")
    # This shipped compiler omits the optional zero-named-argument trailer.
    # Admit only an absent trailer or an explicit zero count, never named data.
    assert payload[start+length:] in (b"",b"\x00\x00"), "Unadmitted named attribute arguments"
    return text, {"evidence_id":evidence["evidence_id"],"attribute_token":attribute["token"],"value_sha256":attribute["value_sha256"],"text_sha256":digest(text.encode())}


def rust_float(value):
    text = repr(float(value))
    return text if "." in text or "e" in text else text + ".0"


def rust_string(value):
    return json.dumps(value, ensure_ascii=False).replace("\\u001f", "\\u{1f}")


def rust_source(facts):
    lines = ["//! Independently derived Prime 12 SI numeric facts. Generated by scripts/extract-unit-facts.py.", "//! See docs/analysis/0010-unit-semantics.md for evidence and limits.", "use crate::Quantity;", "use num_complex::Complex64;", "", "type Fact = (&'static str, f64, [f64; 7]);"]
    for category, symbol in [("unit", "UNITS"), ("physical_constant", "CONSTANTS")]:
        lines += [""]
        if symbol == "UNITS":
            lines += ["// Preserve authenticated numeric facts rather than replacing them by heuristic constants.", "#[allow(clippy::approx_constant)]"]
        lines += ["#[rustfmt::skip]", f"static {symbol}: &[Fact] = &["]
        for fact in facts:
            if fact["category"] == category:
                dimensions = ", ".join(rust_float(d) for d in fact["dimension"])
                lines.append(f"    ({rust_string(fact['name'])}, {rust_float(fact['scale'])}, [{dimensions}]),")
        lines.append("];" )
    lines += ["", "fn find(table: &[Fact], name: &str) -> Option<Quantity> {", "    table.iter().find(|fact| fact.0 == name).map(|fact| Quantity {", "        value: Complex64::new(fact.1, 0.0), dimension: fact.2,", "    })", "}", "", "/// Exact shipped unit symbols only; caller owns generic prefix fallback.", "pub fn lookup(name: &str) -> Option<Quantity> { find(UNITS, name) }", "pub fn evaluate(name: &str) -> Option<Quantity> { lookup(name) }", "pub fn names() -> impl Iterator<Item = &'static str> { UNITS.iter().map(|fact| fact.0) }", "", "/// Physical constants remain separate from the UNIT label namespace.", "pub fn constant_lookup(name: &str) -> Option<Quantity> { find(CONSTANTS, name) }", "pub fn constant_names() -> impl Iterator<Item = &'static str> { CONSTANTS.iter().map(|fact| fact.0) }", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-root", type=pathlib.Path, default=ROOT/"artifacts/rea/engine")
    parser.add_argument("--output", type=pathlib.Path, default=ROOT/"docs/spec/mathcad12-si-units.json")
    parser.add_argument("--rust", type=pathlib.Path, default=ROOT/"crates/mathcraft-core/src/units.rs")
    parser.add_argument("--check", action="store_true", help="Verify existing facts and Rust table without changing files (ignores Rust formatting whitespace)")
    args = parser.parse_args()
    base = StaticModule(args.evidence_root/"unit-system-members.json")
    si = StaticModule(args.evidence_root/"si-unit-members.json")
    base.fold("0x06000001")
    si.fold("0x06000001", base=base)
    interface, authentication = authenticated_interface(si,args.evidence_root/"mcdstaticunitsystemsi.dll-attributes.json")
    facts = []
    excluded = []
    signatures = re.findall(r'^\s*(mc_\S+) :: Mpl\.Value \{([^}]*)\} @ "([^"]+)";',interface,re.MULTILINE)
    for field, signature, target in signatures:
        assert target == "McdStaticUnitSystemSI."+field
        name = re.sub(r'_([0-9A-Fa-f]{2})_',lambda m:chr(int(m[1],16)),field[3:])
        value = si.values[field]
        powers = dict(re.findall(r'McdStaticUnitSystem\.(\w+)\s*(-?\d+)',signature))
        assert not re.sub(r'McdStaticUnitSystem\.\w+\s*-?\d+', '', signature).strip(), "Unadmitted dimensional signature"
        assert all(p in DIMENSIONS+["Money"] for p in powers), "Unrecognized physical dimension"
        if "Money" in powers:
            excluded.append({"name":name,"reason":"Eight-dimensional Money type cannot be represented by seven-dimensional Quantity"})
            continue
        assert isinstance(value,(int,float)) and math.isfinite(value) and value>0, (field,value)
        facts.append({"name":name,"field_name":field,"scale":float(value),"dimension":[int(powers.get(d,0))/60000 for d in DIMENSIONS],"category":"physical_constant" if name in PHYSICAL_CONSTANTS else "unit","assignment":si.assignments[field]})
    facts.sort(key=lambda f:f["name"])
    assert len({f["name"] for f in facts})==len(facts), "Decoded symbol collision"
    sources = [{"evidence_id":m.evidence["evidence_id"],"artifact":m.result["artifact"],"mvid":m.result["module"]["mvid"],"decoded_cil_methods":[{"token":token,"raw_il_sha256":m.methods[token]["body"]["il_sha256"]} for token in sorted(m.decoded)]} for m in [base,si]]
    output = {"schema_version":1,"original_code_executed":False,"authority":"Independent static constant folding of hash-verified shipped CIL and MPL dimensional signatures","dimension_order":DIMENSIONS,"dimension_divisor":60000,"sources":sources,"interface_authentication":authentication,"facts":facts,"excluded":excluded,"limitations":["No affine temperature conversion; delta temperature units only","No Money dimension","No full label inference; physical constants kept separate","No target CLR or original program execution","Prefix expansion is owned by the caller"]}
    generated_rust=rust_source(facts)
    if args.check:
        assert json.loads(args.output.read_text(encoding="utf-8"))==output, "Committed facts differ from authenticated regeneration"
        assert re.sub(r'\s+','',args.rust.read_text(encoding="utf-8"))==re.sub(r'\s+','',generated_rust), "Rust lookup table differs from numeric facts"
    else:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(output,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
        args.rust.write_text(generated_rust,encoding="utf-8")
    print(json.dumps({"facts":len(facts),"units":sum(f["category"]=="unit" for f in facts),"physical_constants":sum(f["category"]=="physical_constant" for f in facts),"excluded":excluded,"base_assignments":len(base.values),"si_assignments":len(si.values)},ensure_ascii=False))


if __name__ == "__main__":
    main()
