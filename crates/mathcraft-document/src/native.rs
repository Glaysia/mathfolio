//! Lossless, read-only view of a Mathcad OPC package.
//!
//! Original parts remain authoritative: inspecting an expression never turns it
//! into our calculator's source language, and saved results are never evaluated.
//! `write_new` preserves part names, contents and compression methods, not the
//! compressed ZIP container's byte representation or all ZIP extra fields.
use roxmltree::{Document, Node};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const WORKSHEET_NS: &str = "http://schemas.mathsoft.com/worksheet50";
const MATH_NS: &str = "http://schemas.mathsoft.com/math50";
const RESULT_NS: &str = "http://schemas.mathsoft.com/result10";
const MAX_MEMBERS: usize = 4096;
const MAX_PART_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_XML_BYTES: usize = 8 * 1024 * 1024;
const MAX_XML_NODES: u32 = 250_000;

type NativeResult<T> = Result<T, String>;

/// An original ZIP member. The byte data is deliberately not exposed mutably.
#[derive(Debug)]
pub struct NativePart {
    pub name: String,
    pub compression: CompressionMethod,
    bytes: Vec<u8>,
    modified: Option<zip::DateTime>,
    unix_mode: Option<u32>,
}

impl NativePart {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct NativeAttribute {
    pub namespace: Option<String>,
    pub name: String,
    pub value: String,
}

/// A source-bound XML element, including identifiers' labels and subscripts.
#[derive(Clone, Debug, Serialize)]
pub struct NativeMathElement {
    /// Element-child indices relative to the owning worksheet math element.
    pub path: Vec<usize>,
    pub namespace: Option<String>,
    pub name: String,
    pub attributes: Vec<NativeAttribute>,
    /// Direct text nodes only; descendant subscript text is not flattened.
    pub text: String,
    /// Exact fragment from the decoded original XML, including prefix spelling.
    pub raw_xml: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NativeMath {
    pub result_ref: Option<String>,
    pub attributes: Vec<NativeAttribute>,
    pub elements: Vec<NativeMathElement>,
    pub raw_xml: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NativeRegion {
    pub part: String,
    /// Element-child indices from the part's root, preserving wrapper elements.
    pub path: Vec<usize>,
    /// Nearest enclosing region in this part, never a flattened global index.
    pub parent_path: Option<Vec<usize>>,
    pub region_id: Option<String>,
    pub kind: String,
    pub namespace: Option<String>,
    pub left: Option<f64>,
    pub top: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub actual_width: Option<f64>,
    pub actual_height: Option<f64>,
    pub align_x: Option<f64>,
    pub align_y: Option<f64>,
    pub attributes: Vec<NativeAttribute>,
    pub math: Option<NativeMath>,
    /// Fragment context lives in `part`; outer namespace declarations may apply.
    pub raw_xml: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NativeDiagnostic {
    pub attributes: Vec<NativeAttribute>,
    pub code: Option<String>,
    pub arguments: Vec<String>,
    pub resource_string: Option<String>,
    pub raw_xml: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NativeWarning {
    pub node_id: Option<String>,
    pub old_type: Option<String>,
    pub old_location: Option<String>,
    pub attributes: Vec<NativeAttribute>,
    pub raw_xml: String,
}

/// An observed saved result. `Synchronized` describes the file, not a new run.
#[derive(Clone, Debug, Serialize)]
pub struct NativeCache {
    pub part: String,
    pub result_id: Option<String>,
    pub calculation_status: Option<String>,
    pub tag: Option<String>,
    pub result_kind: Option<String>,
    pub attributes: Vec<NativeAttribute>,
    pub errors: Vec<NativeDiagnostic>,
    pub warnings: Vec<NativeWarning>,
    pub raw_xml: String,
}

#[derive(Debug)]
pub struct NativePackage {
    source: PathBuf,
    parts: Vec<NativePart>,
    worksheet_part: usize,
    archive_comment: Vec<u8>,
}

impl NativePackage {
    /// Reads and validates bounded package contents without extraction or execution.
    /// ZIP64, multi-volume, encrypted and unsupported compression are rejected.
    pub fn open(path: &Path) -> NativeResult<Self> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        preflight(&mut file)?;
        let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
        let archive_comment = zip.comment().to_vec();
        let mut parts = Vec::with_capacity(zip.len());
        let mut names = HashSet::new();
        let mut total = 0u64;
        let mut worksheet_part = None;
        for i in 0..zip.len() {
            let mut member = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = std::str::from_utf8(member.name_raw())
                .map_err(|_| "Non-UTF-8 ZIP names are not supported losslessly")?
                .to_string();
            if !names.insert(name.clone()) {
                return Err(format!("Duplicate ZIP member: {name}"));
            }
            let compression = member.compression();
            if !matches!(
                compression,
                CompressionMethod::Stored | CompressionMethod::Deflated
            ) {
                return Err(format!(
                    "Unsupported compression in {name}: {compression:?}"
                ));
            }
            if member.size() > MAX_PART_BYTES {
                return Err(format!("ZIP member exceeds byte limit: {name}"));
            }
            total = total
                .checked_add(member.size())
                .ok_or("ZIP size overflow")?;
            if total > MAX_TOTAL_BYTES {
                return Err("ZIP total size exceeds byte limit".into());
            }
            let modified = member.last_modified();
            let unix_mode = member.unix_mode();
            let declared_size = member.size();
            let mut bytes = Vec::new();
            member
                .by_ref()
                .take(MAX_PART_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Cannot read {name}: {e}"))?;
            if bytes.len() as u64 != declared_size || bytes.len() as u64 > MAX_PART_BYTES {
                return Err(format!(
                    "ZIP member length mismatch or limit exceeded: {name}"
                ));
            }
            if is_xml_part(&name) {
                let text = xml_text(&bytes)?;
                let doc = parse_xml(&text)?;
                let root = doc.root_element();
                if is_element(root, WORKSHEET_NS, "worksheet")
                    && worksheet_part.replace(parts.len()).is_some()
                {
                    return Err("Ambiguous package: multiple worksheet roots".into());
                }
            }
            parts.push(NativePart {
                name,
                compression,
                bytes,
                modified,
                unix_mode,
            });
        }
        Ok(Self {
            source: path.canonicalize().map_err(|e| e.to_string())?,
            parts,
            worksheet_part: worksheet_part.ok_or("No worksheet50 worksheet part found")?,
            archive_comment,
        })
    }

    pub fn parts(&self) -> &[NativePart] {
        &self.parts
    }

    pub fn original_part(&self, name: &str) -> Option<&[u8]> {
        self.parts
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.bytes.as_slice())
    }

    pub fn worksheet_part_name(&self) -> &str {
        &self.parts[self.worksheet_part].name
    }

    /// Decodes a view for inspection; `original_part` retains exact source bytes.
    pub fn worksheet_xml(&self) -> NativeResult<String> {
        xml_text(&self.parts[self.worksheet_part].bytes)
    }

    /// Returns a source-ordered view without changing hierarchy or evaluation order.
    /// All worksheet50 regions in XML parts are included, including header/footer.
    pub fn regions(&self) -> NativeResult<Vec<NativeRegion>> {
        let mut regions = Vec::new();
        for part in self.parts.iter().filter(|p| is_xml_part(&p.name)) {
            let text = xml_text(&part.bytes)?;
            let doc = parse_xml(&text)?;
            collect_regions(
                doc.root_element(),
                &part.name,
                &text,
                &[],
                None,
                &mut regions,
            )?;
        }
        Ok(regions)
    }

    /// Preserves cache state and diagnostics; does not interpret or evaluate values.
    pub fn caches(&self) -> NativeResult<Vec<NativeCache>> {
        let mut out = Vec::new();
        for part in self.parts.iter().filter(|p| is_xml_part(&p.name)) {
            let text = xml_text(&part.bytes)?;
            let doc = parse_xml(&text)?;
            if !is_element(doc.root_element(), RESULT_NS, "resultsList") {
                continue;
            }
            for node in doc
                .root_element()
                .children()
                .filter(|n| is_element(*n, RESULT_NS, "resultData"))
            {
                let errors = node
                    .descendants()
                    .filter(|n| is_element(*n, RESULT_NS, "engineError"))
                    .map(|n| NativeDiagnostic {
                        attributes: attributes(n),
                        code: child_text(n, RESULT_NS, "errorCode"),
                        arguments: n
                            .children()
                            .filter(|c| is_element(*c, RESULT_NS, "errorArgument"))
                            .map(all_text)
                            .collect(),
                        resource_string: child_text(n, RESULT_NS, "resource-string"),
                        raw_xml: text[n.range()].into(),
                    })
                    .collect();
                let warnings = node
                    .descendants()
                    .filter(|n| is_element(*n, RESULT_NS, "Warning"))
                    .map(|n| NativeWarning {
                        node_id: attr(n, "nodeId"),
                        old_type: attr(n, "oldType"),
                        old_location: attr(n, "oldLocation"),
                        attributes: attributes(n),
                        raw_xml: text[n.range()].into(),
                    })
                    .collect();
                let result_kind = node
                    .children()
                    .find(|n| n.is_element() && n.tag_name().namespace() == Some(MATH_NS))
                    .map(|n| n.tag_name().name().into());
                out.push(NativeCache {
                    part: part.name.clone(),
                    result_id: attr(node, "result-id"),
                    calculation_status: attr(node, "calculation-status"),
                    tag: attr(node, "tag"),
                    result_kind,
                    attributes: attributes(node),
                    errors,
                    warnings,
                    raw_xml: text[node.range()].into(),
                });
            }
        }
        Ok(out)
    }

    /// Creates a distinct .mcdx; never truncates or overwrites an existing file.
    pub fn write_new(&self, path: &Path) -> NativeResult<()> {
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
        {
            return Err("Native package output requires the .mcdx extension".into());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let target = parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(path.file_name().ok_or("Output file name is missing")?);
        let same_source = if cfg!(windows) {
            target
                .to_string_lossy()
                .eq_ignore_ascii_case(&self.source.to_string_lossy())
        } else {
            target == self.source
        };
        if same_source || path.canonicalize().is_ok_and(|p| p == self.source) {
            return Err("Refusing to overwrite the original document".into());
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let result = self.write_to(file);
        if result.is_err() {
            // This path was created exclusively above, so only our incomplete output is removed.
            let _ = std::fs::remove_file(path);
        }
        result
    }

    fn write_to(&self, file: File) -> NativeResult<()> {
        let mut writer = ZipWriter::new(file);
        writer
            .set_raw_comment(self.archive_comment.clone().into_boxed_slice())
            .map_err(|e| e.to_string())?;
        for part in &self.parts {
            let mut options = SimpleFileOptions::default().compression_method(part.compression);
            if let Some(modified) = part.modified {
                options = options.last_modified_time(modified);
            }
            if let Some(mode) = part.unix_mode {
                options = options.unix_permissions(mode);
            }
            writer
                .start_file(&part.name, options)
                .map_err(|e| e.to_string())?;
            writer.write_all(&part.bytes).map_err(|e| e.to_string())?;
        }
        writer
            .finish()
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())
    }
}

fn is_xml_part(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".xml") || n.ends_with(".rels")
}

fn xml_text(bytes: &[u8]) -> NativeResult<String> {
    if bytes.len() > MAX_XML_BYTES {
        return Err("XML part exceeds byte limit".into());
    }
    let (data, little_endian) = if bytes.starts_with(&[0xff, 0xfe]) {
        (&bytes[2..], Some(true))
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (&bytes[2..], Some(false))
    } else if bytes.starts_with(&[b'<', 0, b'?', 0]) {
        (bytes, Some(true))
    } else if bytes.starts_with(&[0, b'<', 0, b'?']) {
        (bytes, Some(false))
    } else {
        (
            bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes),
            None,
        )
    };
    if let Some(le) = little_endian {
        if data.len() % 2 != 0 {
            return Err("Odd UTF-16 XML byte length".into());
        }
        let units: Vec<u16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|e| e.to_string())
    } else {
        String::from_utf8(data.to_vec()).map_err(|e| format!("Unsupported XML encoding: {e}"))
    }
}

fn parse_xml(text: &str) -> NativeResult<Document<'_>> {
    let doc = Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid XML (DTDs are disabled): {e}"))?;
    if doc
        .descendants()
        .any(|node| node.ancestors().take(129).count() > 128)
    {
        return Err("XML nesting exceeds depth limit".into());
    }
    Ok(doc)
}

fn is_element(node: Node<'_, '_>, namespace: &str, name: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(namespace)
        && node.tag_name().name() == name
}

fn attributes(node: Node<'_, '_>) -> Vec<NativeAttribute> {
    node.attributes()
        .map(|a| NativeAttribute {
            namespace: a.namespace().map(str::to_owned),
            name: a.name().into(),
            value: a.value().into(),
        })
        .collect()
}

fn attr(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.attribute(name).map(str::to_owned)
}

fn number(node: Node<'_, '_>, name: &str) -> NativeResult<Option<f64>> {
    node.attribute(name)
        .map(|s| match s {
            "INF" => Ok(f64::INFINITY),
            "-INF" => Ok(f64::NEG_INFINITY),
            _ => s
                .parse()
                .map_err(|_| format!("Invalid region attribute {name}={s}")),
        })
        .transpose()
}

fn all_text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|n| n.text())
        .collect()
}

fn child_text(node: Node<'_, '_>, namespace: &str, name: &str) -> Option<String> {
    node.children()
        .find(|n| is_element(*n, namespace, name))
        .map(all_text)
}

fn math(node: Node<'_, '_>, text: &str) -> NativeMath {
    fn visit(node: Node<'_, '_>, text: &str, path: &[usize], out: &mut Vec<NativeMathElement>) {
        out.push(NativeMathElement {
            path: path.into(),
            namespace: node.tag_name().namespace().map(str::to_owned),
            name: node.tag_name().name().into(),
            attributes: attributes(node),
            text: node
                .children()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect(),
            raw_xml: text[node.range()].into(),
        });
        for (i, child) in node.children().filter(Node::is_element).enumerate() {
            let mut next = path.to_vec();
            next.push(i);
            visit(child, text, &next, out);
        }
    }
    let mut elements = Vec::new();
    for (i, child) in node.children().filter(Node::is_element).enumerate() {
        visit(child, text, &[i], &mut elements);
    }
    NativeMath {
        result_ref: attr(node, "resultRef"),
        attributes: attributes(node),
        elements,
        raw_xml: text[node.range()].into(),
    }
}

fn collect_regions(
    node: Node<'_, '_>,
    part: &str,
    text: &str,
    path: &[usize],
    parent: Option<&[usize]>,
    out: &mut Vec<NativeRegion>,
) -> NativeResult<()> {
    let region = is_element(node, WORKSHEET_NS, "region");
    if region {
        let payload = node.children().find(Node::is_element);
        let math = node
            .children()
            .find(|n| is_element(*n, WORKSHEET_NS, "math"))
            .map(|n| math(n, text));
        out.push(NativeRegion {
            part: part.into(),
            path: path.into(),
            parent_path: parent.map(<[usize]>::to_vec),
            region_id: attr(node, "region-id"),
            kind: payload
                .map(|n| n.tag_name().name().into())
                .unwrap_or_else(|| "empty".into()),
            namespace: payload.and_then(|n| n.tag_name().namespace().map(str::to_owned)),
            left: number(node, "left")?,
            top: number(node, "top")?,
            width: number(node, "width")?,
            height: number(node, "height")?,
            actual_width: number(node, "actualWidth")?,
            actual_height: number(node, "actualHeight")?,
            align_x: number(node, "align-x")?,
            align_y: number(node, "align-y")?,
            attributes: attributes(node),
            math,
            raw_xml: text[node.range()].into(),
        });
    }
    for (i, child) in node.children().filter(Node::is_element).enumerate() {
        let mut next = path.to_vec();
        next.push(i);
        collect_regions(
            child,
            part,
            text,
            &next,
            if region { Some(path) } else { parent },
            out,
        )?;
    }
    Ok(())
}

/// Inspect the central directory *before* ZipArchive's name map can collapse
/// duplicate members or allocate from an untrusted member count.
fn preflight(file: &mut File) -> NativeResult<()> {
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if !(22..=MAX_ARCHIVE_BYTES).contains(&length) {
        return Err("ZIP archive size outside supported limits".into());
    }
    let tail_len = length.min(65_557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64)))
        .map_err(|e| e.to_string())?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail).map_err(|e| e.to_string())?;
    let pos = (0..=tail_len - 22)
        .rev()
        .find(|&i| {
            tail[i..].starts_with(b"PK\x05\x06")
                && i + 22 + u16le(&tail[i + 20..]) as usize == tail_len
        })
        .ok_or("ZIP end-of-directory record not found")?;
    let e = &tail[pos..];
    let count = u16le(&e[10..]) as usize;
    if u16le(&e[4..]) != 0 || u16le(&e[6..]) != 0 || u16le(&e[8..]) as usize != count {
        return Err("Multi-volume ZIP is unsupported".into());
    }
    if count == 0 || count > MAX_MEMBERS {
        return Err("ZIP member count outside supported limits".into());
    }
    let size = u32le(&e[12..]) as u64;
    let start = u32le(&e[16..]) as u64;
    if size == u32::MAX as u64 || start == u32::MAX as u64 {
        return Err("ZIP64 packages are unsupported".into());
    }
    let end = start
        .checked_add(size)
        .ok_or("ZIP directory size overflow")?;
    if end != length - tail_len as u64 + pos as u64 {
        return Err("ZIP central directory range mismatch".into());
    }
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut names = HashSet::new();
    let mut total = 0u64;
    for _ in 0..count {
        if file.stream_position().map_err(|e| e.to_string())? + 46 > end {
            return Err("Truncated ZIP central directory".into());
        }
        let mut h = [0; 46];
        file.read_exact(&mut h).map_err(|e| e.to_string())?;
        if !h.starts_with(b"PK\x01\x02") {
            return Err("Invalid ZIP central header".into());
        }
        if u16le(&h[8..]) & 1 != 0 {
            return Err("Encrypted ZIP members are unsupported".into());
        }
        let member_size = u32le(&h[24..]) as u64;
        total = total.checked_add(member_size).ok_or("ZIP size overflow")?;
        if member_size > MAX_PART_BYTES || total > MAX_TOTAL_BYTES {
            return Err("ZIP declared member or total size exceeds byte limit".into());
        }
        let name_len = u16le(&h[28..]) as usize;
        let rest = u16le(&h[30..]) as u64 + u16le(&h[32..]) as u64;
        let next = file.stream_position().map_err(|e| e.to_string())? + name_len as u64 + rest;
        if next > end {
            return Err("ZIP central member exceeds directory bounds".into());
        }
        let mut name = vec![0; name_len];
        file.read_exact(&mut name).map_err(|e| e.to_string())?;
        if name.is_empty() || !names.insert(name) {
            return Err("Empty or duplicate ZIP member name".into());
        }
        file.seek(SeekFrom::Start(next))
            .map_err(|e| e.to_string())?;
    }
    if file.stream_position().map_err(|e| e.to_string())? != end {
        return Err("ZIP central directory member count mismatch".into());
    }
    file.rewind().map_err(|e| e.to_string())
}

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Independent fixtures written for this test, not redistributed product data.
    const SHEET: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Keep whitespace, prefix spelling, opaque attributes and comment. -->
<ws:worksheet xmlns:ws="http://schemas.mathsoft.com/worksheet50"
 xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:u="urn:future">
 <ws:regions>
  <ws:region region-id="8" top="19.2" left="0" actualWidth="180" actualHeight="47.2"
   align-y="-12.3" u:future="keep">
   <ws:Area IsCollapsed="true"><ws:regions>
    <ws:region region-id="42" top="9.6" left="19.2" width="0" align-x="2.5">
     <ws:math resultRef="900" disable-calc="true"><ml:define>
      <ml:id labels="VARIABLE" xml:space="preserve">ω<ml:subscript>r</ml:subscript></ml:id>
      <ml:apply><ml:mult/><ml:real>5</ml:real><ml:id labels="UNIT">m</ml:id></ml:apply>
     </ml:define><ws:resultFormat><u:future/></ws:resultFormat></ws:math>
    </ws:region>
   </ws:regions></ws:Area>
  </ws:region>
  <ws:region region-id="10" top="0" left="200"><u:widget u:payload="opaque"/></ws:region>
 </ws:regions>
</ws:worksheet>"#;
    const CACHES: &str = r#"<resultsList xmlns="http://schemas.mathsoft.com/result10"
 xmlns:ml="http://schemas.mathsoft.com/math50">
 <resultData result-id="900" calculation-status="Pending" tag="saved-only">
  <ml:result><ml:real>5</ml:real></ml:result>
  <engineErrors><engineError error-id="3" previous-error-id="2" error-node-handle="7">
   <errorCode>undefined</errorCode><errorArgument>ω</errorArgument>
   <resource-string>정의되지 않음</resource-string>
  </engineError></engineErrors>
  <Warnings><Warning nodeId="n3" oldType="Function" oldLocation="12:2"/></Warnings>
 </resultData>
 <resultData result-id="901" calculation-status="Synchronized"><ml:symResult/></resultData>
</resultsList>"#;
    const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="unknown" Type="urn:future" Target="https://not-fetched.invalid/resource" TargetMode="External"/>
</Relationships>"#;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p = std::env::temp_dir().join(format!(
                "mathcraft-native-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(path: &Path, sheet: &[u8], extra: &[(&str, &[u8])]) {
        let mut writer = ZipWriter::new(File::create(path).unwrap());
        writer
            .set_raw_comment(b"unknown archive comment".to_vec().into_boxed_slice())
            .unwrap();
        for (name, bytes, compression) in [
            ("mathcad/worksheet.xml", sheet, CompressionMethod::Deflated),
            (
                "mathcad/result.xml",
                CACHES.as_bytes(),
                CompressionMethod::Deflated,
            ),
            (
                "mathcad/_rels/worksheet.xml.rels",
                RELS.as_bytes(),
                CompressionMethod::Stored,
            ),
            (
                "custom/private.bin",
                &[0, 255, 42, 0, 91][..],
                CompressionMethod::Stored,
            ),
            ("custom/", &[][..], CompressionMethod::Stored),
        ]
        .into_iter()
        .chain(
            extra
                .iter()
                .map(|(n, b)| (*n, *b, CompressionMethod::Deflated)),
        ) {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default().compression_method(compression),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn roundtrip_keeps_every_part_byte_and_compression() {
        let dir = TestDir::new();
        let source = dir.path("source.mcdx");
        let copy = dir.path("copy.mcdx");
        fixture(
            &source,
            SHEET.as_bytes(),
            &[("custom/future.xml", b"<future a='untouched'/>\r\n")],
        );
        let original_file = std::fs::read(&source).unwrap();
        let package = NativePackage::open(&source).unwrap();
        assert_eq!(package.worksheet_part_name(), "mathcad/worksheet.xml");
        assert_eq!(package.worksheet_xml().unwrap(), SHEET);
        package.write_new(&copy).unwrap();
        let saved = NativePackage::open(&copy).unwrap();
        assert_eq!(package.parts().len(), saved.parts().len());
        for (a, b) in package.parts().iter().zip(saved.parts()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.compression, b.compression);
            assert_eq!(a.bytes(), b.bytes());
        }
        assert_eq!(saved.archive_comment, package.archive_comment);
        assert_eq!(
            saved.original_part("mathcad/_rels/worksheet.xml.rels"),
            Some(RELS.as_bytes())
        );
        assert_eq!(std::fs::read(&source).unwrap(), original_file);
        assert!(package.write_new(&source).is_err());
        assert!(package.write_new(&copy).is_err());
        assert!(package.write_new(&dir.path("wrong.mathcraft")).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), original_file);
        std::fs::remove_file(&source).unwrap();
        assert!(package.write_new(&source).is_err());
        assert!(
            !source.exists(),
            "the original path must never be recreated"
        );
    }

    #[test]
    fn hierarchy_anchor_attributes_and_math_labels_are_not_flattened() {
        let dir = TestDir::new();
        let path = dir.path("nested.mcdx");
        fixture(&path, SHEET.as_bytes(), &[]);
        let package = NativePackage::open(&path).unwrap();
        let regions = package.regions().unwrap();
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0].kind, "Area");
        assert_eq!(regions[0].parent_path, None);
        assert_eq!(regions[1].parent_path, Some(regions[0].path.clone()));
        assert_eq!(regions[1].region_id.as_deref(), Some("42"));
        assert_eq!(regions[1].width, Some(0.0));
        assert_eq!(regions[1].height, None);
        assert_eq!(regions[0].align_y, Some(-12.3));
        assert_eq!(regions[0].actual_height, Some(47.2));
        assert_eq!(regions[1].align_x, Some(2.5));
        assert_eq!(regions[2].kind, "widget");
        assert_eq!(regions[2].namespace.as_deref(), Some("urn:future"));
        assert!(
            regions[0]
                .attributes
                .iter()
                .any(|a| a.namespace.as_deref() == Some("urn:future") && a.value == "keep")
        );
        let math = regions[1].math.as_ref().unwrap();
        assert_eq!(math.result_ref.as_deref(), Some("900"));
        let ids: Vec<_> = math.elements.iter().filter(|e| e.name == "id").collect();
        assert_eq!(ids[0].text, "ω");
        assert!(ids[0].raw_xml.contains("<ml:subscript>r</ml:subscript>"));
        assert!(
            ids[0]
                .attributes
                .iter()
                .any(|a| a.name == "labels" && a.value == "VARIABLE")
        );
        assert!(
            ids[1]
                .attributes
                .iter()
                .any(|a| a.name == "labels" && a.value == "UNIT")
        );
        assert!(math.raw_xml.contains("<u:future/>"));
    }

    #[test]
    fn saved_pending_cache_diagnostics_and_ids_remain_distinct() {
        let dir = TestDir::new();
        let path = dir.path("cache.mcdx");
        fixture(&path, SHEET.as_bytes(), &[]);
        let caches = NativePackage::open(&path).unwrap().caches().unwrap();
        assert_eq!(caches.len(), 2);
        let c = &caches[0];
        assert_eq!(c.result_id.as_deref(), Some("900"));
        assert_eq!(c.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(c.tag.as_deref(), Some("saved-only"));
        assert_eq!(c.result_kind.as_deref(), Some("result"));
        assert_eq!(c.errors[0].code.as_deref(), Some("undefined"));
        assert_eq!(c.errors[0].arguments, ["ω"]);
        assert_eq!(
            c.errors[0].resource_string.as_deref(),
            Some("정의되지 않음")
        );
        assert!(
            c.errors[0]
                .attributes
                .iter()
                .any(|a| a.name == "previous-error-id" && a.value == "2")
        );
        assert_eq!(c.warnings[0].old_type.as_deref(), Some("Function"));
        assert_eq!(c.warnings[0].old_location.as_deref(), Some("12:2"));
        assert_eq!(caches[1].result_kind.as_deref(), Some("symResult"));
    }

    #[test]
    fn utf16_view_does_not_transcode_original_xml_on_write() {
        let dir = TestDir::new();
        let source = dir.path("utf16.mcdx");
        let copy = dir.path("copy.mcdx");
        let text = SHEET.replace("encoding=\"utf-8\"", "encoding=\"utf-16\"");
        let bytes: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        fixture(&source, &bytes, &[]);
        let package = NativePackage::open(&source).unwrap();
        assert_eq!(package.worksheet_xml().unwrap(), text);
        assert_eq!(package.regions().unwrap().len(), 3);
        package.write_new(&copy).unwrap();
        assert_eq!(
            NativePackage::open(&copy)
                .unwrap()
                .original_part("mathcad/worksheet.xml"),
            Some(bytes.as_slice())
        );
    }

    #[test]
    fn rejects_dtd_even_in_an_unknown_xml_part() {
        let dir = TestDir::new();
        let path = dir.path("dtd.mcdx");
        fixture(
            &path,
            SHEET.as_bytes(),
            &[(
                "custom/future.xml",
                b"<!DOCTYPE future [<!ENTITY x 'bad'>]><future>&x;</future>",
            )],
        );
        assert!(NativePackage::open(&path).unwrap_err().contains("DTD"));
    }

    #[test]
    fn rejects_duplicate_names_before_zip_reader_collapses_them() {
        let dir = TestDir::new();
        let path = dir.path("duplicate.mcdx");
        fixture(
            &path,
            SHEET.as_bytes(),
            &[("custom/privaue.bin", b"different")],
        );
        let mut bytes = std::fs::read(&path).unwrap();
        let old = b"custom/privaue.bin";
        let new = b"custom/private.bin";
        for i in 0..bytes.len() - old.len() {
            if &bytes[i..i + old.len()] == old {
                bytes[i..i + old.len()].copy_from_slice(new);
            }
        }
        std::fs::write(&path, bytes).unwrap();
        assert!(
            NativePackage::open(&path)
                .unwrap_err()
                .contains("duplicate")
        );
    }

    #[test]
    fn rejects_declared_size_and_member_count_limits_before_decompression() {
        let dir = TestDir::new();
        let path = dir.path("limits.mcdx");
        fixture(&path, SHEET.as_bytes(), &[]);
        let original = std::fs::read(&path).unwrap();
        let mut bytes = original.clone();
        let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        bytes[central + 24..central + 28]
            .copy_from_slice(&((MAX_PART_BYTES + 1) as u32).to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(
            NativePackage::open(&path)
                .unwrap_err()
                .contains("size exceeds")
        );
        let mut bytes = original;
        let end = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        bytes[end + 8..end + 10].copy_from_slice(&4097u16.to_le_bytes());
        bytes[end + 10..end + 12].copy_from_slice(&4097u16.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(
            NativePackage::open(&path)
                .unwrap_err()
                .contains("member count")
        );
    }
}
