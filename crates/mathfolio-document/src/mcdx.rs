//! Read-only subset of the observed Mathcad Prime 12 ZIP/XML structure.
use super::*;
use num_complex::Complex64;
use roxmltree::{Document, Node};
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
use std::{collections::HashMap, io::Cursor};

const MATH: &str = "http://schemas.mathsoft.com/math50";
fn children<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    node.children().filter(Node::is_element).collect()
}
fn math_children<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().namespace() == Some(MATH))
        .collect()
}
fn read_part<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<String> {
    let mut part = zip.by_name(name).map_err(|e| e.to_string())?;
    if part.size() > 8 * 1024 * 1024 {
        return Err(format!("Oversized XML part: {name}"));
    }
    let mut text = String::new();
    part.by_ref()
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 8 * 1024 * 1024 {
        return Err("XML part exceeds budget".into());
    }
    Ok(text)
}
fn xml(text: &str) -> Result<Document<'_>> {
    Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
fn identifier(node: Node<'_, '_>) -> Result<String> {
    let mut name = String::new();
    for n in node.descendants().filter(Node::is_text) {
        let value = n.text().unwrap_or("");
        if n.ancestors()
            .any(|a| a.is_element() && a.tag_name().name() == "Subscript")
            && !name.ends_with('_')
        {
            name.push('_');
        }
        name.push_str(value);
    }
    let name = name.trim().replace([' ', '\n', '\r'], "");
    if name.is_empty() {
        Err("Empty identifier".into())
    } else {
        Ok(name)
    }
}
fn expression(node: Node<'_, '_>, depth: usize) -> Result<String> {
    if depth > 48 {
        return Err("Math XML nesting limit".into());
    }
    let ch = math_children(node);
    match node.tag_name().name() {
        "id" => identifier(node),
        "real" => {
            let s = node.text().unwrap_or("").trim();
            s.parse::<f64>().map_err(|_| "Invalid real literal")?;
            Ok(s.into())
        }
        "imag" => {
            let s = node.text().unwrap_or("1").trim();
            s.parse::<f64>().map_err(|_| "Invalid imaginary literal")?;
            Ok(format!("({s}*i)"))
        }
        "eval" | "parens" => expression(*ch.first().ok_or("Empty expression")?, depth + 1),
        "apply" => {
            let op = *ch.first().ok_or("Empty apply")?;
            let args = &ch[1..];
            let render = |n: Node<'_, '_>| expression(n, depth + 1);
            match op.tag_name().name() {
                "plus" | "minus" | "mult" | "scale" | "div" | "pow" => {
                    let symbol = match op.tag_name().name() {
                        "plus" => '+',
                        "minus" => '-',
                        "div" => '/',
                        "pow" => '^',
                        _ => '*',
                    };
                    if args.len() == 1 && symbol == '-' {
                        return Ok(format!("(-{})", render(args[0])?));
                    }
                    if args.len() != 2 {
                        return Err(format!("Unexpected arity for {}", op.tag_name().name()));
                    }
                    Ok(format!(
                        "({} {symbol} {})",
                        render(args[0])?,
                        render(args[1])?
                    ))
                }
                "nthRoot" => {
                    if args.len() != 2 {
                        return Err("Unexpected root arity".into());
                    }
                    if args[0].tag_name().name() == "placeholder" {
                        Ok(format!("sqrt({})", render(args[1])?))
                    } else {
                        Ok(format!("({})^(1/({}))", render(args[1])?, render(args[0])?))
                    }
                }
                "absval" => Ok(format!(
                    "abs({})",
                    render(*args.first().ok_or("Empty abs")?)?
                )),
                "id" => Ok(format!(
                    "{}({})",
                    identifier(op)?,
                    args.iter()
                        .map(|n| render(*n))
                        .collect::<Result<Vec<_>>>()?
                        .join(",")
                )),
                other => Err(format!("Math operator {other}")),
            }
        }
        other => Err(format!("Math node {other}")),
    }
}
fn math_line(node: Node<'_, '_>) -> Result<String> {
    let first = *math_children(node).first().ok_or("Empty math region")?;
    let mut result = if first.tag_name().name() == "define" {
        let parts = math_children(first);
        if parts.len() != 2 {
            return Err("Unexpected definition shape".into());
        }
        if parts[0].tag_name().name() != "id" {
            return Err("Function definitions in imported XML are not supported yet".into());
        }
        format!("{} := {}", identifier(parts[0])?, expression(parts[1], 0)?)
    } else {
        expression(first, 0)?
    };
    if let Some(override_node) = first
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "unitOverride")
        && let Some(unit) = math_children(override_node)
            .first()
            .filter(|n| n.tag_name().name() != "placeholder")
    {
        result.push_str(" -> ");
        result.push_str(&expression(*unit, 0)?);
    }
    Ok(result)
}
fn cached_number(node: Node<'_, '_>) -> Option<Complex64> {
    let result = node.descendants().find(|n| {
        n.is_element() && n.tag_name().namespace() == Some(MATH) && n.tag_name().name() == "result"
    })?;
    let real = result
        .descendants()
        .find(|n| n.has_tag_name((MATH, "real")))
        .and_then(|n| n.text())?
        .parse()
        .ok()?;
    if result
        .descendants()
        .any(|n| n.is_element() && matches!(n.tag_name().name(), "matrix" | "sequence"))
    {
        return None;
    }
    let imag = result
        .descendants()
        .find(|n| n.has_tag_name((MATH, "imag")))
        .and_then(|n| n.text())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    Some(Complex64::new(real, imag))
}
#[cfg(not(target_arch = "wasm32"))]
pub fn import_mcdx(path: &Path) -> Result<Worksheet> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("MCDX exceeds 64 MiB".into());
    }
    import_mcdx_reader(path, file)
}

pub fn import_mcdx_bytes(name: &Path, bytes: &[u8]) -> Result<Worksheet> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("MCDX exceeds 64 MiB".into());
    }
    import_mcdx_reader(name, Cursor::new(bytes))
}
fn import_mcdx_reader<R: Read + std::io::Seek>(path: &Path, reader: R) -> Result<Worksheet> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    if zip.len() > 4096 {
        return Err("Too many package entries".into());
    }
    let worksheet = read_part(&mut zip, "mathcad/worksheet.xml")?;
    let document = xml(&worksheet)?;
    if !document
        .root_element()
        .has_tag_name(("http://schemas.mathsoft.com/worksheet50", "worksheet"))
    {
        return Err("Only the observed worksheet50 format is supported".into());
    }
    let result_text = read_part(&mut zip, "mathcad/result.xml").unwrap_or_default();
    let result_doc = xml(&result_text).ok();
    let mut cached = HashMap::new();
    if let Some(result_doc) = &result_doc {
        for n in result_doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "resultData")
        {
            if let Some(value) = cached_number(n) {
                cached.insert(n.attribute("result-id").unwrap_or("").to_string(), value);
            }
        }
    }
    let rel_text = read_part(&mut zip, "mathcad/_rels/worksheet.xml.rels").unwrap_or_default();
    let rel_doc = xml(&rel_text).ok();
    let rels: HashMap<String, String> = rel_doc
        .iter()
        .flat_map(|d| d.descendants())
        .filter_map(|n| {
            Some((
                n.attribute("Id")?.to_string(),
                n.attribute("Target")?.to_string(),
            ))
        })
        .collect();
    let mut rows = Vec::new();
    for region in document
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "region")
    {
        if rows.len() >= 1000 {
            return Err("Too many worksheet regions".into());
        }
        let mut imported = Imported {
            region_id: region.attribute("region-id").unwrap_or("").into(),
            left: region
                .attribute("left")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.),
            top: region
                .attribute("top")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.),
            unsupported: None,
            cached_display: None,
            cached_value: None,
        };
        if let Some(math) = children(region)
            .into_iter()
            .find(|n| n.tag_name().name() == "math")
        {
            imported.cached_value = cached
                .get(math.attribute("resultRef").unwrap_or(""))
                .copied();
            imported.cached_display = imported.cached_value.map(mathfolio_core::format_complex);
            let input = match math_line(math) {
                Ok(s) => s,
                Err(reason) => {
                    imported.unsupported = Some(reason);
                    format!("# Unsupported Mathcad region {}", imported.region_id)
                }
            };
            rows.push(Row {
                input,
                note: false,
                imported: Some(imported),
            });
        } else if let Some(text) = children(region)
            .into_iter()
            .find(|n| n.tag_name().name() == "text")
        {
            let input = read_note(&mut zip, text, &rels)
                .unwrap_or_else(|_| "[Text region could not be decoded]".into());
            rows.push(Row {
                input,
                note: true,
                imported: Some(imported),
            });
        } else {
            imported.unsupported = Some("Non-math/text region".into());
            rows.push(Row {
                input: "# Unsupported region".into(),
                note: false,
                imported: Some(imported),
            });
        }
    }
    let doc = Worksheet {
        version: 1,
        title: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported worksheet")
            .into(),
        rows,
    };
    doc.validate()?;
    Ok(doc)
}
fn read_note<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    node: Node<'_, '_>,
    rels: &HashMap<String, String>,
) -> Result<String> {
    let id = node.attribute("item-idref").ok_or("No text relationship")?;
    let target = rels.get(id).ok_or("Text relationship is missing")?;
    // Look up inside the archive only. Never resolve a relationship against the filesystem/network.
    let path = if target.starts_with('/') {
        target.trim_start_matches('/').to_owned()
    } else {
        format!("mathcad/{target}")
    };
    let mut entry = zip.by_name(&path).map_err(|e| e.to_string())?;
    if entry.size() > 2 * 1024 * 1024 {
        return Err("Text package exceeds budget".into());
    }
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    drop(entry);
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Text package exceeds budget".into());
    }
    let mut nested = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let text = read_part(&mut nested, "Xaml/Document.xaml")?;
    let doc = xml(&text)?;
    let paragraphs: Vec<String> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Paragraph")
        .map(|p| {
            p.descendants()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect::<String>()
        })
        .collect();
    Ok(paragraphs.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_math_and_subscripts() {
        let doc=xml(r#"<math xmlns:ml="http://schemas.mathsoft.com/math50"><ml:define><ml:id><Span>L<Subscript>rx</Subscript></Span></ml:id><ml:apply><ml:scale/><ml:real>12</ml:real><ml:id>uH</ml:id></ml:apply></ml:define></math>"#).unwrap();
        assert_eq!(math_line(doc.root_element()).unwrap(), "L_rx := (12 * uH)");
    }
    #[test]
    fn unknown_operator_is_explicit() {
        let d=xml(r#"<math xmlns:ml="http://schemas.mathsoft.com/math50"><ml:apply><ml:integral/><ml:real>1</ml:real></ml:apply></math>"#).unwrap();
        assert!(math_line(d.root_element()).is_err());
    }
    #[test]
    fn external_entities_are_rejected() {
        assert!(xml("<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///secret'>]><a>&x;</a>").is_err());
    }
}
