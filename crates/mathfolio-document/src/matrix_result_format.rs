//! Closed matrix-result presentation reader; does not read or execute results.
//! Draft for a separate document module. Source matrix ml:display is unrelated.
use crate::canvas::{MatrixResultViewport, MatrixViewOffset, MatrixViewSize};
use crate::native::NativePackage;
use roxmltree::Node;

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const PRESENTATION: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";

fn pair(value: &str) -> Result<(u32, u32), String> {
    let (a, b) = value
        .split_once(',')
        .ok_or("Matrix format needs a row,column pair")?;
    fn integer(value: &str) -> Result<u32, String> {
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Invalid matrix format coordinate".into());
        }
        value
            .parse()
            .map_err(|_| "Matrix format coordinate overflow".into())
    }
    Ok((integer(a)?, integer(b)?))
}

fn boolean(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Invalid matrix format boolean".into()),
    }
}

fn read_matrix(node: Node<'_, '_>) -> Result<MatrixResultViewport, String> {
    if !node.has_tag_name((WS, "matrix")) {
        return Err("Expected worksheet matrix result format".into());
    }
    if node.attributes().any(|attribute| {
        attribute.namespace().is_some()
            || !matches!(
                attribute.name(),
                "size" | "offset" | "show-indices" | "expand-nested-arrays"
            )
    }) || node.children().any(|child| {
        !child.is_comment() && !(child.is_text() && child.text().unwrap_or("").trim().is_empty())
    }) {
        return Err("Unsupported matrix result format remains preserved in native source".into());
    }
    let size = node
        .attribute("size")
        .map(|value| -> Result<MatrixViewSize, String> {
            let (rows, cols) = pair(value)?;
            if rows == 0
                || cols == 0
                || rows > 65_536
                || cols > 65_536
                || rows.checked_mul(cols).is_none_or(|n| n > 65_536)
            {
                return Err("Matrix result view exceeds supported positive cell budget".into());
            }
            Ok(MatrixViewSize { rows, cols })
        })
        .transpose()?;
    let offset = node
        .attribute("offset")
        .map(|value| -> Result<MatrixViewOffset, String> {
            let (row, col) = pair(value)?;
            if row > i32::MAX as u32 || col > i32::MAX as u32 {
                return Err("Matrix result offset exceeds supported bounds".into());
            }
            Ok(MatrixViewOffset { row, col })
        })
        .transpose()?;
    let show_indices = node.attribute("show-indices").map(boolean).transpose()?;
    // This known property remains in raw native XML. Flat result preparation
    // independently rejects nested values, so it is never interpreted as a
    // request to flatten/drop their contents here.
    node.attribute("expand-nested-arrays")
        .map(boolean)
        .transpose()?;
    Ok(MatrixResultViewport {
        size,
        offset,
        show_indices,
    })
}

fn matrix_in(parent: Node<'_, '_>) -> Result<MatrixResultViewport, String> {
    if parent.children().any(|n| {
        n.is_element() && n.tag_name().name() == "matrix" && n.tag_name().namespace() != Some(WS)
    }) {
        return Err("Unsupported namespace on matrix result format".into());
    }
    let mut candidates = parent.children().filter(|n| n.has_tag_name((WS, "matrix")));
    let Some(matrix) = candidates.next() else {
        return Ok(MatrixResultViewport::default());
    };
    if candidates.next().is_some() {
        return Err("Ambiguous duplicate matrix result formats".into());
    }
    read_matrix(matrix)
}

/// Only a direct worksheet math/resultFormat/matrix carrier is accepted.
pub fn read_local(math: Node<'_, '_>) -> Result<MatrixResultViewport, String> {
    if !math.has_tag_name((WS, "math")) {
        return Err("Matrix result format needs an owning worksheet Math".into());
    }
    if math.children().any(|n| {
        n.is_element()
            && n.tag_name().name() == "resultFormat"
            && n.tag_name().namespace() != Some(WS)
    }) {
        return Err("Unsupported namespace on resultFormat".into());
    }
    let mut formats = math
        .children()
        .filter(|n| n.has_tag_name((WS, "resultFormat")));
    let Some(format) = formats.next() else {
        return Ok(MatrixResultViewport::default());
    };
    if formats.next().is_some() {
        return Err("Ambiguous duplicate resultFormat".into());
    }
    matrix_in(format)
}

/// Actual presentation metadata, without substituting Help's user default for
/// absent source bytes. An absent effective size is deliberately unresolved.
pub fn read_defaults(package: &NativePackage) -> Result<MatrixResultViewport, String> {
    let Some(xml) = package.original_xml_part("mathcad/settings/presentation.xml")? else {
        return Ok(MatrixResultViewport::default());
    };
    if xml.len() > 8 * 1024 * 1024 {
        return Err("Matrix presentation XML exceeds byte budget".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let root = doc.root_element();
    if !root.has_tag_name((PRESENTATION, "presentation")) {
        return Err("Unsupported matrix presentation root".into());
    }
    let mut renderings = root
        .children()
        .filter(|n| n.has_tag_name((PRESENTATION, "mathRendering")));
    let Some(rendering) = renderings.next() else {
        return Ok(MatrixResultViewport::default());
    };
    if renderings.next().is_some() {
        return Err("Ambiguous mathRendering defaults".into());
    }
    let mut results = rendering
        .children()
        .filter(|n| n.has_tag_name((WS, "results")));
    let Some(results) = results.next() else {
        return Ok(MatrixResultViewport::default());
    };
    if rendering
        .children()
        .filter(|n| n.has_tag_name((WS, "results")))
        .count()
        != 1
    {
        return Err("Ambiguous result format defaults".into());
    }
    matrix_in(results)
}

/// Resolve only properties positively present in the given carriers. No size,
/// ORIGIN or persisted false override is fabricated.
pub fn inherit(
    local: &MatrixResultViewport,
    defaults: &MatrixResultViewport,
) -> MatrixResultViewport {
    MatrixResultViewport {
        size: local.size.or(defaults.size),
        offset: local.offset.or(defaults.offset),
        show_indices: local.show_indices.or(defaults.show_indices),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(body: &str) -> Result<MatrixResultViewport, String> {
        let xml = format!("<math xmlns='{WS}'>{body}</math>");
        let doc = roxmltree::Document::parse(&xml).unwrap();
        read_local(doc.root_element())
    }
    #[test]
    fn missing_properties_stay_absent_and_false_stays_explicit() {
        assert_eq!(read("").unwrap(), MatrixResultViewport::default());
        let local = read(
            "<resultFormat><general precision='3'/><matrix show-indices='false'/></resultFormat>",
        )
        .unwrap();
        assert_eq!(local.size, None);
        assert_eq!(local.offset, None);
        assert_eq!(local.show_indices, Some(false));
        let defaults = MatrixResultViewport {
            size: Some(MatrixViewSize { rows: 12, cols: 12 }),
            offset: None,
            show_indices: Some(true),
        };
        assert_eq!(inherit(&local, &defaults).show_indices, Some(false));
        assert_eq!(inherit(&local, &defaults).size, defaults.size);
        assert_eq!(inherit(&local, &defaults).offset, None);
    }
    #[test]
    fn row_column_order_and_beyond_twelve_are_retained() {
        let view = read("<resultFormat><matrix size='13,17' offset='7,2' show-indices='true' expand-nested-arrays='false'/></resultFormat>").unwrap();
        assert_eq!(view.size, Some(MatrixViewSize { rows: 13, cols: 17 }));
        assert_eq!(view.offset, Some(MatrixViewOffset { row: 7, col: 2 }));
        assert_eq!(view.show_indices, Some(true));
    }
    #[test]
    fn ambiguous_unknown_nested_and_overflow_do_not_create_an_editable_view() {
        for matrix in [
            "<matrix size='0,0'/>",
            "<matrix size='65536,2'/>",
            "<matrix size='+1,2'/>",
            "<matrix size='01,2'/>",
            "<matrix offset=' 7 , 2 '/>",
            "<matrix offset='2147483648,0'/>",
            "<matrix offset='0,1,2'/>",
            "<matrix show-indices='False'/>",
            "<matrix show-indices='1'/>",
            "<matrix future='preserve'/>",
            "<matrix xmlns:f='urn:f' f:size='1,1'/>",
            "<matrix><nestedMatrixFormats/></matrix>",
            "<matrix/><matrix/>",
        ] {
            assert!(
                read(&format!("<resultFormat>{matrix}</resultFormat>")).is_err(),
                "{matrix}"
            );
        }
        assert!(read("<resultFormat/><resultFormat/>").is_err());
        assert!(
            read("<f:resultFormat xmlns:f='urn:future'><f:matrix size='2,3'/></f:resultFormat>")
                .is_err()
        );
        assert!(
            read("<resultFormat><f:matrix xmlns:f='urn:future' size='2,3'/></resultFormat>")
                .is_err()
        );
    }
    #[test]
    fn source_matrix_display_is_not_a_result_view() {
        let value = read("<ml:matrix xmlns:ml='http://schemas.mathsoft.com/math50' rows='1' cols='1'><ml:display rows='1' cols='1'/></ml:matrix>").unwrap();
        assert_eq!(value, MatrixResultViewport::default());
    }

    #[test]
    fn tracked_lcc_presentation_supplies_twelve_without_fabricating_local_overrides() {
        use std::io::{Cursor, Write};
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in [
            (
                "mathcad/worksheet.xml",
                b"<worksheet xmlns='http://schemas.mathsoft.com/worksheet50'/>".as_slice(),
            ),
            (
                "mathcad/settings/presentation.xml",
                include_bytes!("../../../tests/reference/rich-text/lcc-standard-presentation.xml")
                    .as_slice(),
            ),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        let package = NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap();
        assert_lcc_defaults(&package);
    }

    #[test]
    #[ignore = "requires the private original LCC package in local REA artifacts"]
    fn actual_lcc_package_supplies_twelve_without_fabricating_local_overrides() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx");
        assert_lcc_defaults(&NativePackage::open(&path).unwrap());
    }

    fn assert_lcc_defaults(package: &NativePackage) {
        let defaults = read_defaults(package).unwrap();
        assert_eq!(defaults.size, Some(MatrixViewSize { rows: 12, cols: 12 }));
        assert_eq!(defaults.offset, Some(MatrixViewOffset { row: 0, col: 0 }));
        assert_eq!(defaults.show_indices, Some(false));
        assert_eq!(read("").unwrap(), MatrixResultViewport::default());
        assert_eq!(inherit(&read("").unwrap(), &defaults), defaults);
    }
}
