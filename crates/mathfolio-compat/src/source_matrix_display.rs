//! Shared strict Math50 source-display admission; raw calculation ignores metadata.
//!
//! Equation 06001243 reads first display, offset before size; 0600117a writes
//! display before ALL column-major source cells. See the59-body frozen packet.
//! This initial subset rejects overhang instead of simulating setter shiftback.
use crate::{ML, Node};

// Preserve the existing RAW compatibility matrix cell limit. This is not the
// editor's4096-cell limit, a viewport limit, or a recovered original threshold.
const MAX_RAW_CELLS: usize = 16_384;

use mathfolio_editor::SourceMatrixDisplay;

pub(crate) struct SourceMatrixCells<'nodes, 'a, 'input> {
    pub(crate) cells: &'nodes [Node<'a, 'input>],
}

fn validate_shape(shape: [usize; 2]) -> Result<(), String> {
    if shape.contains(&0)
        || shape[0]
            .checked_mul(shape[1])
            .is_none_or(|n| n > MAX_RAW_CELLS)
    {
        return Err("Invalid matrix size".into());
    }
    Ok(())
}
fn pair(value: &str) -> Result<[u32; 2], String> {
    let (a, b) = value
        .split_once(',')
        .ok_or("Invalid source matrix display pair")?;
    if a.is_empty() || b.is_empty() || !a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit()) {
        return Err("Invalid source matrix display pair".into());
    }
    Ok([
        a.parse()
            .map_err(|_| "Source matrix display integer overflow")?,
        b.parse()
            .map_err(|_| "Source matrix display integer overflow")?,
    ])
}

pub(crate) fn read_display(
    node: Node<'_, '_>,
    shape: [usize; 2],
) -> Result<SourceMatrixDisplay, String> {
    if !node.has_tag_name((ML, "display")) {
        return Err("Foreign source matrix display namespace".into());
    }
    if node
        .attributes()
        .any(|a| a.namespace().is_some() || !matches!(a.name(), "offset" | "size"))
    {
        return Err("Unsupported source matrix display attribute".into());
    }
    if node
        .children()
        .any(|n| !n.is_text() || !n.text().unwrap_or("").trim().is_empty())
    {
        return Err("Structured source matrix display is unsupported".into());
    }
    validate_shape(shape)?;
    let display = SourceMatrixDisplay::from_parts(
        node.attribute("offset").map(pair).transpose()?,
        node.attribute("size").map(pair).transpose()?,
    )?;
    display.window(shape)?;
    Ok(display)
}

/// Borrow the complete source cell list without cloning XML or allocating a
/// second Vec. Metadata never slices cells or participates in numeric values.
pub(crate) fn parse<'nodes, 'a, 'input>(
    matrix: Node<'a, 'input>,
    rows: usize,
    cols: usize,
    children: &'nodes [Node<'a, 'input>],
) -> Result<SourceMatrixCells<'nodes, 'a, 'input>, String> {
    validate_shape([rows, cols])?;
    let first_is_display = children
        .first()
        .is_some_and(|n| n.tag_name().name() == "display");
    let cells = if first_is_display {
        // This entire profile was unsupported previously. Be strict about
        // structural material; do not let children() hide comments/PI/text.
        if matrix.children().any(|n| {
            n.is_comment()
                || n.is_pi()
                || (n.is_text() && !n.text().unwrap_or("").trim().is_empty())
        }) {
            return Err("Unsupported structural source matrix content".into());
        }
        read_display(children[0], [rows, cols])?;
        &children[1..]
    } else {
        children
    };
    if cells.iter().any(|n| n.tag_name().name() == "display") {
        return Err("Source matrix display must be unique and first".into());
    }
    if rows.checked_mul(cols) != Some(cells.len()) {
        return Err("Invalid matrix source cell count".into());
    }
    Ok(SourceMatrixCells { cells })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod common_path_tests;
