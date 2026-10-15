"""Generate fresh, minimal Mathcad runtime probes from an app-saved blank package.

Does not edit the original blank or any user worksheet. Stdlib only.
"""
import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

WS = "http://schemas.mathsoft.com/worksheet50"
ML = "http://schemas.mathsoft.com/math50"
ET.register_namespace("", WS)
ET.register_namespace("ml", ML)


def real(value):
    n = ET.Element(f"{{{ML}}}real")
    n.text = str(value)
    return n


def ident(name, label="VARIABLE"):
    n = ET.Element(f"{{{ML}}}id", {"labels": label, "{http://www.w3.org/XML/1998/namespace}space": "preserve"})
    n.text = name
    return n


def apply(op, *operands):
    n = ET.Element(f"{{{ML}}}apply")
    n.append(ET.Element(f"{{{ML}}}{op}"))
    n.extend(operands)
    return n


def call(name, *operands):
    n = ET.Element(f"{{{ML}}}apply")
    n.append(ident(name, "FUNCTION"))
    n.extend(operands)
    return n


def define(name, value, global_=False):
    n = ET.Element(f"{{{ML}}}{'globalDefine' if global_ else 'define'}")
    n.extend([ident(name), value])
    return n


def function(name, param, value):
    n = ET.Element(f"{{{ML}}}define")
    f = ET.SubElement(n, f"{{{ML}}}function")
    f.append(ident(name, "FUNCTION"))
    b = ET.SubElement(f, f"{{{ML}}}boundVars")
    b.append(ident(param))
    n.append(value)
    return n


def evaluate(value, unit=None):
    n = ET.Element(f"{{{ML}}}eval")
    n.append(value)
    override = ET.SubElement(n, f"{{{ML}}}unitOverride")
    override.append(unit if unit is not None else ET.Element(f"{{{ML}}}placeholder"))
    return n


def scale(value, unit):
    return apply("scale", real(value), ident(unit, "UNIT"))


def matrix(*values):
    n = ET.Element(f"{{{ML}}}matrix", {"rows": str(len(values)), "cols": "1"})
    n.extend(real(v) for v in values)
    return n


def build_cases():
    cases = []

    def add(name, question, expressions, positions=None):
        positions = positions or [(20, 20 + 65 * i) for i in range(len(expressions))]
        cases.append((name, question, expressions, positions))

    add("arithmetic", "Basic explicit arithmetic AST", [evaluate(apply("plus", real(2), real(3)))])
    add("power-right", "Right-nested power AST", [evaluate(apply("pow", real(2), apply("pow", real(3), real(2))))])
    add("negative-square", "Negation of a square AST", [evaluate(apply("neg", apply("pow", real(2), real(2))))])
    add("negative-base-square", "Square of a negative base AST", [evaluate(apply("pow", apply("neg", real(2)), real(2)))])
    add("zero-divide", "Division by zero error", [evaluate(apply("div", real(1), real(0)))])
    add("zero-power-zero", "Zero to zero power", [evaluate(apply("pow", real(0), real(0)))])
    add("negative-root", "Square root of negative real", [evaluate(apply("nthRoot", ET.Element(f"{{{ML}}}placeholder"), real(-1)))])
    add("imaginary-square", "Imaginary constant spelling", [evaluate(apply("pow", ident("i", "CONSTANT"), real(2)))])
    add("complex-abs", "Magnitude of 3+4i", [evaluate(apply("absval", apply("plus", real(3), apply("mult", real(4), ident("i", "CONSTANT")))))])
    add("sin-pi-half", "Dimensionless trigonometry", [evaluate(call("sin", apply("div", ident("π", "CONSTANT"), real(2))))])
    add("log-negative", "Complex natural log", [evaluate(call("ln", real(-1)))])
    add("missing-variable", "Unbound identifier error", [evaluate(ident("probe_missing"))])
    add("forward-local", "Local definition after query", [evaluate(ident("a")), define("a", real(7))])
    add("normal-local", "Local definition before query", [define("a", real(7)), evaluate(ident("a"))])
    add("xml-reversed", "XML order reversed; definition physically above query", [evaluate(ident("a")), define("a", real(7))], [(20, 150), (20, 20)])
    add("geometry-reversed", "XML order puts definition first but it is physically below", [define("a", real(7)), evaluate(ident("a"))], [(20, 150), (20, 20)])
    add("same-y-left-definition", "Equal Y: definition on left", [evaluate(ident("a")), define("a", real(7))], [(350, 20), (20, 20)])
    add("same-y-right-definition", "Equal Y: definition on right", [define("a", real(7)), evaluate(ident("a"))], [(350, 20), (20, 20)])
    add("equal-position", "Two regions at identical X/Y", [define("a", real(7)), evaluate(ident("a"))], [(20, 20), (20, 20)])
    add("near-equal-y", "Y differs by 1e-8 DIP, left/right opposite", [define("a", real(7)), evaluate(ident("a"))], [(350, 20), (20, 20.00000001)])
    add("redefinition", "Queries around two definitions", [define("a", real(1)), evaluate(ident("a")), define("a", real(2)), evaluate(ident("a"))])
    add("self-redefinition", "Definition reads previous version of itself", [define("a", real(1)), define("a", apply("plus", ident("a"), real(1))), evaluate(ident("a"))])
    add("failed-redefinition", "Invalid redefinition versus prior value", [define("a", real(1)), define("a", ident("probe_missing")), evaluate(ident("a"))])
    add("global-forward", "Global definition below query", [evaluate(ident("g")), define("g", real(7), True)])
    add("global-local-shadow", "Local and global definition interaction", [define("g", real(7), True), evaluate(ident("g")), define("g", real(8)), evaluate(ident("g"))])
    add("function-capture", "Function external variable binding before redefinition", [define("a", real(2)), function("f", "x", apply("mult", ident("a"), ident("x"))), define("a", real(3)), evaluate(call("f", real(4)))])
    add("function-forward-reference", "Function body refers to a later definition", [function("f", "x", apply("mult", ident("a"), ident("x"))), define("a", real(3)), evaluate(call("f", real(4)))])
    add("units-add", "Addition with compatible prefixed units", [evaluate(apply("plus", scale(1, "m"), scale(100, "cm")), ident("cm", "UNIT"))])
    add("units-mismatch", "Addition with incompatible dimensions", [evaluate(apply("plus", scale(1, "m"), scale(1, "s")))])
    add("units-root", "Square root of squared unit", [evaluate(apply("nthRoot", ET.Element(f"{{{ML}}}placeholder"), apply("pow", scale(2, "m"), real(2))), ident("m", "UNIT"))])
    add("units-ohm", "Voltage/current dimension reduction", [evaluate(apply("div", scale(12, "V"), scale(2, "A")), ident("Ω", "UNIT"))])
    add("sin-degrees", "Degree conversion in trig", [evaluate(call("sin", scale(90, "deg")))])
    add("sin-dimensional", "Dimensional argument to sine", [evaluate(call("sin", scale(1, "m")))])
    add("origin-default", "Default ORIGIN value", [evaluate(ident("ORIGIN"))])
    add("origin-redefined", "ORIGIN can be redefined", [define("ORIGIN", real(1)), evaluate(ident("ORIGIN"))])
    add("matrix-literal", "Column matrix result serialization", [evaluate(matrix(10, 20))])
    add("matrix-index", "Matrix index with default ORIGIN", [define("v", matrix(10,20)), evaluate(apply("indexer",ident("v"),real(0)))])
    add("matrix-index-origin1", "Matrix index after setting ORIGIN=1", [define("ORIGIN",real(1)),define("v",matrix(10,20)),evaluate(apply("indexer",ident("v"),real(1)))])
    return cases


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("blank",type=Path)
    parser.add_argument("output",type=Path)
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    with ZipFile(args.blank) as archive:
        parts={name:archive.read(name) for name in archive.namelist()}
    manifest=[]
    for name,question,expressions,positions in build_cases():
        root=ET.fromstring(parts["mathcad/worksheet.xml"])
        regions=root.find(f"{{{WS}}}regions")
        regions.clear()
        queries=[]
        for index,(expr,(x,y)) in enumerate(zip(expressions,positions)):
            region=ET.SubElement(regions,f"{{{WS}}}region",{"region-id":str(index),"left":str(x),"top":str(y),"actualWidth":"200","actualHeight":"30"})
            math=ET.SubElement(region,f"{{{WS}}}math",{"resultRef":str(index)})
            math.append(expr)
            if expr.tag==f"{{{ML}}}eval":queries.append(str(index))
        destination=args.output/f"{name}.input.mcdx"
        if destination.exists():raise FileExistsError(destination)
        with ZipFile(destination,"x",compression=ZIP_DEFLATED) as archive:
            for part,content in parts.items():
                archive.writestr(part,ET.tostring(root,encoding="utf-8") if part=="mathcad/worksheet.xml" else content)
        manifest.append({"id":name,"question":question,"input":str(destination.resolve()),"output":str((args.output/f"{name}.output.mcdx").resolve()),"query_region_ids":queries})
    (args.output/"manifest.json").write_text(json.dumps(manifest,indent=2),encoding="utf-8")
    print(f"Generated {len(manifest)} fresh probes")


if __name__=="__main__":main()
