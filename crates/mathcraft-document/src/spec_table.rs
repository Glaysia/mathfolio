//! Inert, source-preserving specification-table edits. No expressions execute here.
use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use mathcraft_editor::{NodeKind, PortableTree};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::ops::Range;

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const ML: &str = "http://schemas.mathsoft.com/math50";
const MAX_XML: usize = 8 * 1024 * 1024;
const MAX_ROWS: usize = 1024;
const MAX_COLUMNS: usize = 256;
const MAX_CELLS: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CellSlot {
    Header,
    Unit,
    /// Zero-based data row; header and unit rows are not included.
    Body(usize),
}

/// The native Copy caller uses a mathematical matrix for body-only selections,
/// but creates a separate specification-table region when headers/units occur.
/// This is an independent transport representation, not the native clipboard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum SpecTableClipboard {
    Matrix(PortableTree),
    TableXml(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecCell {
    pub tree: PortableTree,
    /// Namespace-bound native expression, including its original literal spelling.
    pub mathml: String,
    range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecColumn {
    pub header: SpecCell,
    pub unit: Option<SpecCell>,
    pub cells: Vec<SpecCell>,
    pub definition_mathml: String,
    pub result_ref: Option<String>,
    rhs_range: Range<usize>,
    matrix_range: Option<Range<usize>>,
    math_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecTable {
    source: String,
    columns: Vec<SpecColumn>,
    rows: usize,
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_XML {
        return Err("Specification-table XML exceeds 8 MiB".into());
    }
    Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Add only inherited namespace declarations. All original bytes stay intact.
pub(crate) fn standalone(node: Node<'_, '_>, source: &str) -> Result<String, String> {
    let raw = &source[node.range()];
    let end = tag_end(raw)?;
    let at = if raw.as_bytes()[end - 1] == b'/' {
        end - 1
    } else {
        end
    };
    let mut declarations = String::new();
    let local_names = attribute_names(&raw[..end]);
    for namespace in node.namespaces() {
        let name = namespace
            .name()
            .map_or("xmlns".to_owned(), |n| format!("xmlns:{n}"));
        if !local_names.iter().any(|n| n == &name) {
            declarations.push_str(&format!(" {name}=\"{}\"", escape(namespace.uri())));
        }
    }
    let candidate = format!("{}{}{}", &raw[..at], declarations, &raw[at..]);
    parse(&candidate)?;
    Ok(candidate)
}

fn tag_end(source: &str) -> Result<usize, String> {
    let mut quote = None;
    for (offset, ch) in source.char_indices() {
        match (quote, ch) {
            (Some(q), c) if q == c => quote = None,
            (None, '\'' | '"') => quote = Some(ch),
            (None, '>') => return Ok(offset),
            _ => {}
        }
    }
    Err("Specification-table XML tag is incomplete".into())
}

fn attribute_names(opening: &str) -> Vec<&str> {
    let bytes = opening.as_bytes();
    let mut at = 1;
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    let mut names = Vec::new();
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'=' {
            at += 1;
        }
        let name = &opening[start..at];
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            break;
        }
        names.push(name);
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let Some(&quote) = bytes.get(at) else { break };
        at += 1;
        while at < bytes.len() && bytes[at] != quote {
            at += 1;
        }
        at += 1;
    }
    names
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    node.children().filter(Node::is_element).collect()
}

fn cell(node: Node<'_, '_>, source: &str) -> Result<SpecCell, String> {
    if node.tag_name().namespace() != Some(ML) {
        return Err("Specification-table cell is not native MathML".into());
    }
    let mathml = standalone(node, source)?;
    Ok(SpecCell {
        tree: tree_from_mathml(&mathml)?,
        mathml,
        range: node.range(),
    })
}

fn empty_tree() -> PortableTree {
    PortableTree {
        kind: NodeKind::Placeholder,
        children: vec![],
    }
}

fn empty_math(rows: usize) -> String {
    // Native XML always stores a column matrix, including a single slot.
    // The native calculation S-expression's scalar lowering is a separate step.
    let value = format!(
        "<ml:matrix rows=\"{rows}\" cols=\"1\">{}</ml:matrix>",
        "<ml:placeholder/>".repeat(rows)
    );
    format!(
        "<ws:math xmlns:ws=\"{WS}\" xmlns:ml=\"{ML}\"><ml:define><ml:placeholder/><ml:apply><ml:scale/>{value}<ml:placeholder/></ml:apply></ml:define></ws:math>"
    )
}

fn apply_patches(source: &str, mut patches: Vec<(Range<usize>, String)>) -> Result<String, String> {
    patches.sort_by_key(|(range, _)| (range.start, range.end));
    let mut out = String::with_capacity(source.len());
    let mut previous = 0;
    for (range, replacement) in patches {
        if range.start < previous || range.end < range.start || range.end > source.len() {
            return Err("Overlapping specification-table edits".into());
        }
        out.push_str(&source[previous..range.start]);
        out.push_str(&replacement);
        previous = range.end;
    }
    out.push_str(&source[previous..]);
    Ok(out)
}

fn expression_xml(tree: &PortableTree) -> Result<String, String> {
    let mut replacement = tree_to_mathml(tree)?;
    if !replacement.contains("xmlns:ml=") {
        let end = replacement
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .ok_or("MathML expression missing")?;
        replacement.insert_str(end, &format!(" xmlns:ml=\"{ML}\""));
    }
    let doc = parse(&replacement)?;
    if doc
        .root_element()
        .descendants()
        .any(|n| n.has_tag_name((ML, "define")) || n.has_tag_name((ML, "eval")))
    {
        return Err("Specification-table cells cannot contain worksheet statements".into());
    }
    Ok(replacement)
}

impl SpecTable {
    /// The original default constructor uses one data row and one column.
    /// Header and unit are additional independent empty mathematical slots.
    pub fn new(data_rows: usize, columns: usize) -> Result<Self, String> {
        if data_rows == 0
            || data_rows > MAX_ROWS
            || columns == 0
            || columns > MAX_COLUMNS
            || columns.saturating_mul(data_rows + 2) > MAX_CELLS
        {
            return Err("Specification-table dimensions exceed limits".into());
        }
        Self::parse(&format!(
            "<ws:region xmlns:ws=\"{WS}\" xmlns:ml=\"{ML}\" region-id=\"0\" left=\"0\" top=\"0\"><ws:spec-table>{}</ws:spec-table></ws:region>",
            empty_math(data_rows).repeat(columns)
        ))
    }
    /// Accepts a namespace-bound region wrapper, spec-table, or one context wrapper.
    /// Unknown worksheet metadata is retained; unsupported expressions fail locally.
    pub fn parse(xml: &str) -> Result<Self, String> {
        let document = parse(xml)?;
        let candidates = document
            .root_element()
            .descendants()
            .filter(|n| n.has_tag_name((WS, "spec-table")))
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err("Expected exactly one native specification table".into());
        }
        let table = candidates[0];
        let math = elements(table);
        if math.is_empty() || math.len() > MAX_COLUMNS {
            return Err("Specification-table column count is invalid".into());
        }
        let mut columns = Vec::new();
        let mut rows = 0;
        for math in math {
            if !math.has_tag_name((WS, "math")) {
                return Err("Unsupported specification-table column object".into());
            }
            let definitions = math
                .children()
                .filter(|n| n.has_tag_name((ML, "define")))
                .collect::<Vec<_>>();
            if definitions.len() != 1 {
                return Err("Specification-table column needs exactly one definition".into());
            }
            let definition = definitions[0];
            let parts = elements(definition);
            if parts.len() != 2 {
                return Err("Specification-table definition needs a header and data".into());
            }
            let header = cell(parts[0], xml)?;
            let rhs = parts[1];
            let rhs_parts = elements(rhs);
            let scaled = rhs.has_tag_name((ML, "apply"))
                && rhs_parts
                    .first()
                    .is_some_and(|n| n.has_tag_name((ML, "scale")));
            let (values, unit) = if scaled {
                if rhs_parts.len() != 3 {
                    return Err("Specification-table scale needs values and a unit".into());
                }
                (rhs_parts[1], Some(cell(rhs_parts[2], xml)?))
            } else {
                (rhs, None)
            };
            let (cells, matrix_range) = if values.has_tag_name((ML, "matrix")) {
                let count = values
                    .attribute("rows")
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| *n > 0 && *n <= MAX_ROWS)
                    .ok_or("Invalid specification-table row count")?;
                if values.attribute("cols") != Some("1") {
                    return Err("Specification-table data must have one column".into());
                }
                let nodes = elements(values);
                if nodes.len() != count {
                    return Err("Specification-table dimensions do not match its cells".into());
                }
                (
                    nodes
                        .into_iter()
                        .map(|n| cell(n, xml))
                        .collect::<Result<Vec<_>, _>>()?,
                    Some(values.range()),
                )
            } else {
                (vec![cell(values, xml)?], None)
            };
            rows = rows.max(cells.len());
            columns.push(SpecColumn {
                header,
                unit,
                cells,
                definition_mathml: standalone(definition, xml)?,
                result_ref: math.attribute("resultRef").map(str::to_owned),
                rhs_range: rhs.range(),
                matrix_range,
                math_range: math.range(),
            });
        }
        if columns.len().saturating_mul(rows + 2) > MAX_CELLS {
            return Err("Specification-table dimensions exceed cell limits".into());
        }
        Ok(Self {
            source: xml.into(),
            columns,
            rows,
        })
    }

    pub fn columns(&self) -> &[SpecColumn] {
        &self.columns
    }
    pub fn col_count(&self) -> usize {
        self.columns.len()
    }
    /// Data rows only, excluding the header and unit rows.
    pub fn row_count(&self) -> usize {
        self.rows
    }
    pub fn xml(&self) -> &str {
        &self.source
    }

    fn commit_patches(
        &mut self,
        mut patches: Vec<(Range<usize>, String)>,
        invalidate_size: bool,
    ) -> Result<bool, String> {
        if patches.is_empty() {
            return Ok(false);
        }
        if invalidate_size {
            let doc = parse(&self.source)?;
            if let Some(region) = doc
                .descendants()
                .find(|n| n.has_tag_name((WS, "spec-table")))
                .and_then(|n| n.ancestors().find(|p| p.has_tag_name((WS, "region"))))
            {
                for name in ["actualWidth", "actualHeight"] {
                    if let Some(attr) = region.attribute_node(name) {
                        patches.push((attr.range(), String::new()));
                    }
                }
            }
        }
        let source = apply_patches(&self.source, patches)?;
        let updated = Self::parse(&source)?;
        *self = updated;
        Ok(true)
    }

    /// Insert a data row before `index`; index==row_count appends. Every
    /// column is padded with empty native slots, never invented zero values.
    pub fn insert_row(&mut self, index: usize) -> Result<bool, String> {
        if index > self.rows
            || self.rows >= MAX_ROWS
            || self.col_count().saturating_mul(self.rows + 3) > MAX_CELLS
        {
            return Err("Specification-table row insertion exceeds bounds".into());
        }
        let mut patches = Vec::new();
        for column in &self.columns {
            patches.push(self.row_edit(column, Some(index), None, self.rows + 1)?);
        }
        self.commit_patches(patches, true)
    }

    /// Removing the final data row clears it, matching the native ClearContent
    /// command. The header and unit stay present; an already empty row is a no-op.
    pub fn delete_row(&mut self, index: usize) -> Result<bool, String> {
        if index >= self.rows {
            return Err("Specification-table row is outside its bounds".into());
        }
        if self.rows == 1 {
            let slots = (0..self.col_count())
                .map(|c| (c, CellSlot::Body(0)))
                .collect::<Vec<_>>();
            return self.clear_cells(&slots);
        }
        let mut patches = Vec::new();
        for column in &self.columns {
            patches.push(self.row_edit(column, None, Some(index..index + 1), self.rows - 1)?);
        }
        self.commit_patches(patches, true)
    }

    fn row_edit(
        &self,
        column: &SpecColumn,
        insert: Option<usize>,
        remove: Option<Range<usize>>,
        count: usize,
    ) -> Result<(Range<usize>, String), String> {
        let empty = format!("<ml:placeholder xmlns:ml=\"{ML}\"/>");
        if let Some(range) = &column.matrix_range {
            let raw = &self.source[range.clone()];
            let end = tag_end(raw)?;
            let close = raw.rfind("</").ok_or("Matrix closing tag missing")?;
            let opening = replace_attribute(&raw[..=end], "rows", &count.to_string())?;
            let mut local = vec![(0..end + 1, opening)];
            let mut length = column.cells.len();
            if let Some(index) = insert {
                let at = column
                    .cells
                    .get(index)
                    .map_or(close, |c| c.range.start - range.start);
                local.push((at..at, empty.clone()));
                length += 1;
            }
            if let Some(indices) = remove {
                for index in indices {
                    if let Some(cell) = column.cells.get(index) {
                        local.push((
                            cell.range.start - range.start..cell.range.end - range.start,
                            String::new(),
                        ));
                        length -= 1;
                    }
                }
            }
            if length < count {
                local.push((close..close, empty.repeat(count - length)));
            }
            Ok((range.clone(), apply_patches(raw, local)?))
        } else {
            let mut cells = vec![self.source[column.cells[0].range.clone()].to_owned()];
            cells.resize(self.rows, empty.clone());
            if let Some(index) = insert {
                cells.insert(index, empty.clone());
            }
            if let Some(indices) = remove {
                cells.drain(indices);
            }
            cells.resize(count, empty);
            let replacement = if count == 1 {
                cells.remove(0)
            } else {
                format!(
                    "<ml:matrix xmlns:ml=\"{ML}\" rows=\"{count}\" cols=\"1\">{}</ml:matrix>",
                    cells.concat()
                )
            };
            Ok((column.cells[0].range.clone(), replacement))
        }
    }

    pub fn insert_column(&mut self, index: usize) -> Result<bool, String> {
        if index > self.col_count()
            || self.col_count() >= MAX_COLUMNS
            || (self.col_count() + 1).saturating_mul(self.rows + 2) > MAX_CELLS
        {
            return Err("Specification-table column insertion exceeds bounds".into());
        }
        let at = if let Some(column) = self.columns.get(index) {
            column.math_range.start
        } else {
            let doc = parse(&self.source)?;
            let table = doc
                .descendants()
                .find(|n| n.has_tag_name((WS, "spec-table")))
                .ok_or("Native table missing")?;
            table.range().start
                + self.source[table.range()]
                    .rfind("</")
                    .ok_or("Native table closing element missing")?
        };
        self.commit_patches(vec![(at..at, empty_math(self.rows))], true)
    }

    /// The final column is cleared, including its header and scale; it is not
    /// removed and does not ask the canvas to delete the entire table region.
    pub fn delete_column(&mut self, index: usize) -> Result<bool, String> {
        let column = self
            .columns
            .get(index)
            .ok_or("Specification-table column is outside its bounds")?;
        if self.col_count() == 1 {
            let mut slots = vec![(0, CellSlot::Header), (0, CellSlot::Unit)];
            slots.extend((0..self.rows).map(|r| (0, CellSlot::Body(r))));
            return self.clear_cells(&slots);
        }
        self.commit_patches(vec![(column.math_range.clone(), String::new())], true)
    }

    /// One atomic multi-cell clear, preserving table dimensions and all
    /// unselected metadata. A duplicate selected slot is cleared once.
    pub fn clear_cells(&mut self, slots: &[(usize, CellSlot)]) -> Result<bool, String> {
        let mut seen = std::collections::HashSet::new();
        let mut patches = Vec::new();
        for &(index, slot) in slots {
            let tree = self.cell(index, slot)?;
            if !seen.insert((index, slot)) || matches!(tree.kind, NodeKind::Placeholder) {
                continue;
            }
            let column = &self.columns[index];
            let target = match slot {
                CellSlot::Header => &column.header,
                CellSlot::Unit => column.unit.as_ref().ok_or("Native unit slot missing")?,
                CellSlot::Body(row) => column.cells.get(row).ok_or("Native data slot missing")?,
            };
            patches.push((
                target.range.clone(),
                format!("<ml:placeholder xmlns:ml=\"{ML}\"/>"),
            ));
        }
        self.commit_patches(patches, true)
    }

    fn validate_selection(
        &self,
        rows: &Range<usize>,
        columns: &Range<usize>,
    ) -> Result<(), String> {
        if rows.start >= rows.end
            || rows.end > self.rows + 2
            || columns.start >= columns.end
            || columns.end > self.col_count()
        {
            return Err("Specification-table selection is outside its bounds".into());
        }
        Ok(())
    }

    /// Clear Cells uses the native indeterminate direction, which always clears
    /// expressions and never removes rows, columns or the final minimum slots.
    pub fn clear_selection(
        &mut self,
        display_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<bool, String> {
        self.validate_selection(&display_rows, &columns)?;
        let slots = display_rows
            .flat_map(|row| {
                columns.clone().map(move |column| {
                    (
                        column,
                        match row {
                            0 => CellSlot::Header,
                            1 => CellSlot::Unit,
                            row => CellSlot::Body(row - 2),
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        self.clear_cells(&slots)
    }

    /// Original clipboard caller contract: body-only copy yields a matrix.
    /// Header/unit copy yields a new table with fresh column metadata and at
    /// least one data slot. Source formatting/result references are not copied.
    pub fn copy_selection(
        &self,
        display_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<SpecTableClipboard, String> {
        self.validate_selection(&display_rows, &columns)?;
        if display_rows.start >= 2 {
            return self
                .copy_display_matrix(display_rows, columns)
                .map(SpecTableClipboard::Matrix);
        }
        let data_rows = display_rows.end.saturating_sub(2).max(1);
        let mut target = Self::new(data_rows, columns.len())?;
        let mut patches = Vec::new();
        for (index, source_column) in self.columns[columns].iter().enumerate() {
            let target_column = &target.columns[index];
            if display_rows.start == 0 {
                patches.push((
                    target_column.header.range.clone(),
                    source_column.header.mathml.clone(),
                ));
            }
            if display_rows.end >= 2
                && let Some(unit) = &source_column.unit
            {
                patches.push((
                    target_column
                        .unit
                        .as_ref()
                        .expect("new table has unit slots")
                        .range
                        .clone(),
                    unit.mathml.clone(),
                ));
            }
            for row in 0..display_rows.end.saturating_sub(2) {
                if let Some(cell) = source_column.cells.get(row) {
                    patches.push((target_column.cells[row].range.clone(), cell.mathml.clone()));
                }
            }
        }
        target.commit_patches(patches, false)?;
        Ok(SpecTableClipboard::TableXml(target.source))
    }

    /// Native Cut is Copy followed by the same structural removal as Backspace.
    /// Copy validation completes before any mutation; failure leaves source
    /// byte-identical. `bool` reports content change, including a minimum no-op.
    pub fn cut_selection(
        &mut self,
        display_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<(SpecTableClipboard, bool), String> {
        let clipboard = self.copy_selection(display_rows.clone(), columns.clone())?;
        let changed = self.remove_or_clear_selection(display_rows, columns)?;
        Ok((clipboard, changed))
    }

    /// Native Delete/Backspace on a rectangular selection: whole columns take
    /// priority, whole data rows follow, otherwise clear selected content.
    /// Unlike explicit Delete Row/Column, the final selected row/column is kept
    /// unchanged. Explicit Clear Cells should use `clear_cells` instead.
    pub fn remove_or_clear_selection(
        &mut self,
        display_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<bool, String> {
        if display_rows.start >= display_rows.end
            || display_rows.end > self.rows + 2
            || columns.start >= columns.end
            || columns.end > self.col_count()
        {
            return Err("Specification-table selection is outside its bounds".into());
        }
        if display_rows.start == 0 && display_rows.end == self.rows + 2 {
            let remove = columns.len().min(self.col_count() - 1);
            if remove == 0 {
                return Ok(false);
            }
            let patches = self.columns[columns.start..columns.start + remove]
                .iter()
                .map(|column| (column.math_range.clone(), String::new()))
                .collect();
            return self.commit_patches(patches, true);
        }
        if columns.start == 0 && columns.end == self.col_count() {
            if self.rows <= 1 || display_rows.start <= 1 {
                return Ok(false);
            }
            let remove = display_rows.len().min(self.rows - 1);
            let data_start = display_rows.start - 2;
            let mut patches = Vec::new();
            for column in &self.columns {
                patches.push(self.row_edit(
                    column,
                    None,
                    Some(data_start..data_start + remove),
                    self.rows - remove,
                )?);
            }
            return self.commit_patches(patches, true);
        }
        let mut slots = Vec::new();
        for row in display_rows {
            for column in columns.clone() {
                slots.push((
                    column,
                    match row {
                        0 => CellSlot::Header,
                        1 => CellSlot::Unit,
                        row => CellSlot::Body(row - 2),
                    },
                ));
            }
        }
        self.clear_cells(&slots)
    }

    pub fn copy_matrix(
        &self,
        data_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<PortableTree, String> {
        if data_rows.start >= data_rows.end
            || data_rows.end > self.rows
            || columns.start >= columns.end
            || columns.end > self.col_count()
        {
            return Err("Specification-table copy range is outside its bounds".into());
        }
        self.copy_display_matrix(data_rows.start + 2..data_rows.end + 2, columns)
    }

    /// Low-level mathematical matrix construction can include header/unit rows.
    /// The native Copy caller uses a new table for those rows: `copy_selection`.
    /// Returned cells are independent structural trees in row-major order.
    pub fn copy_display_matrix(
        &self,
        display_rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<PortableTree, String> {
        if display_rows.start >= display_rows.end
            || display_rows.end > self.rows + 2
            || columns.start >= columns.end
            || columns.end > self.col_count()
        {
            return Err("Specification-table copy range is outside its bounds".into());
        }
        let mut children = Vec::new();
        for row in display_rows.clone() {
            for column in columns.clone() {
                let slot = match row {
                    0 => CellSlot::Header,
                    1 => CellSlot::Unit,
                    row => CellSlot::Body(row - 2),
                };
                children.push(self.cell(column, slot)?);
            }
        }
        Ok(PortableTree {
            kind: NodeKind::Matrix {
                rows: display_rows.len(),
                cols: columns.len(),
                display: None,
            },
            children,
        })
    }

    /// Paste a structural matrix into an existing data rectangle. Out-of-bounds
    /// paste is explicit; it never silently truncates or overwrites header/unit.
    pub fn paste_matrix(
        &mut self,
        start_column: usize,
        start_data_row: usize,
        tree: &PortableTree,
    ) -> Result<bool, String> {
        tree.validate()
            .map_err(|e| format!("Invalid clipboard matrix: {e:?}"))?;
        if matches!(
            tree.kind,
            NodeKind::Matrix {
                display: Some(_),
                ..
            }
        ) {
            return Err("Source matrix display cannot be consumed by table paste".into());
        }
        let NodeKind::Matrix { rows, cols, .. } = tree.kind else {
            return Err("Specification-table paste requires a matrix".into());
        };
        if start_column
            .checked_add(cols)
            .is_none_or(|end| end > self.col_count())
            || start_data_row
                .checked_add(rows)
                .is_none_or(|end| end > self.rows)
        {
            return Err("Specification-table paste range is outside its bounds".into());
        }
        let mut patches = Vec::new();
        for col in 0..cols {
            let column = &self.columns[start_column + col];
            let mut additional = Vec::new();
            for row in 0..rows {
                let target_row = start_data_row + row;
                let new = &tree.children[row * cols + col];
                if column
                    .cells
                    .get(target_row)
                    .is_some_and(|old| old.tree == *new)
                {
                    continue;
                }
                let xml = expression_xml(new)?;
                if let Some(old) = column.cells.get(target_row) {
                    patches.push((old.range.clone(), xml));
                } else {
                    while column.cells.len() + additional.len() < target_row {
                        additional.push(format!("<ml:placeholder xmlns:ml=\"{ML}\"/>"));
                    }
                    additional.push(xml);
                }
            }
            if !additional.is_empty() {
                if let Some(range) = &column.matrix_range {
                    let raw = &self.source[range.clone()];
                    let end = tag_end(raw)?;
                    let close = raw.rfind("</").ok_or("Matrix closing tag missing")?;
                    patches.push((
                        range.start..range.start + end + 1,
                        replace_attribute(
                            &raw[..=end],
                            "rows",
                            &(column.cells.len() + additional.len()).to_string(),
                        )?,
                    ));
                    patches.push((
                        range.start + close..range.start + close,
                        additional.concat(),
                    ));
                } else {
                    // Scalar columns have only row zero. Convert its source to a
                    // matrix together with all new slots in one nonoverlapping patch.
                    let first = patches
                        .iter()
                        .position(|(r, _)| *r == column.cells[0].range)
                        .map(|at| patches.remove(at).1)
                        .unwrap_or_else(|| self.source[column.cells[0].range.clone()].into());
                    patches.push((column.cells[0].range.clone(),format!("<ml:matrix xmlns:ml=\"{ML}\" rows=\"{}\" cols=\"1\">{first}{}</ml:matrix>",1+additional.len(),additional.concat())));
                }
            }
        }
        self.commit_patches(patches, true)
    }
    /// Padded native definitions stay intact. The calculation layer handles the
    /// native distinction between trailing empty cells and internal missing data.
    pub fn definition_mathmls(&self) -> Result<Vec<String>, String> {
        Ok(self
            .columns
            .iter()
            .map(|c| c.definition_mathml.clone())
            .collect())
    }
    pub fn cell(&self, column: usize, slot: CellSlot) -> Result<PortableTree, String> {
        let column = self
            .columns
            .get(column)
            .ok_or("Specification-table column is outside its bounds")?;
        match slot {
            CellSlot::Header => Ok(column.header.tree.clone()),
            CellSlot::Unit => Ok(column
                .unit
                .as_ref()
                .map(|c| c.tree.clone())
                .unwrap_or_else(empty_tree)),
            CellSlot::Body(row) if row < self.rows => Ok(column
                .cells
                .get(row)
                .map(|c| c.tree.clone())
                .unwrap_or_else(empty_tree)),
            CellSlot::Body(_) => Err("Specification-table row is outside its bounds".into()),
        }
    }

    /// An atomic subtree edit. Invalid input leaves the entire original unchanged.
    /// Body(row_count()) appends one data row; intermediate blanks stay blanks.
    pub fn set_cell(
        &mut self,
        column: usize,
        slot: CellSlot,
        tree: &PortableTree,
    ) -> Result<(), String> {
        if matches!(slot, CellSlot::Body(row) if row > self.rows || row >= MAX_ROWS) {
            return Err("Specification-table row is outside its editable bounds".into());
        }
        let col = self
            .columns
            .get(column)
            .ok_or("Specification-table column is outside its bounds")?;
        if self
            .cell(column, slot)
            .as_ref()
            .is_ok_and(|old| old == tree)
        {
            return Ok(());
        }
        let mut replacement = tree_to_mathml(tree)?;
        if !replacement.contains("xmlns:ml=") {
            let end = replacement
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .ok_or("MathML expression missing")?;
            replacement.insert_str(end, &format!(" xmlns:ml=\"{ML}\""));
        }
        // Reject definitions/evaluation in cells rather than storing statements
        // that cannot round-trip as a native scalar expression.
        let check = parse(&replacement)?;
        if check
            .root_element()
            .descendants()
            .any(|n| n.has_tag_name((ML, "define")) || n.has_tag_name((ML, "eval")))
        {
            return Err("Specification-table cells cannot contain worksheet statements".into());
        }
        let (range, replacement) = match slot {
            CellSlot::Header => (col.header.range.clone(), replacement),
            CellSlot::Unit => {
                if let Some(unit) = &col.unit {
                    (unit.range.clone(), replacement)
                } else {
                    (
                        col.rhs_range.clone(),
                        format!(
                            "<ml:apply xmlns:ml=\"{ML}\"><ml:scale/>{}{replacement}</ml:apply>",
                            &self.source[col.rhs_range.clone()]
                        ),
                    )
                }
            }
            CellSlot::Body(row) => {
                if let Some(old) = col.cells.get(row) {
                    (old.range.clone(), replacement)
                } else {
                    let mut values = col
                        .cells
                        .iter()
                        .map(|c| self.source[c.range.clone()].to_owned())
                        .collect::<Vec<_>>();
                    while values.len() < row {
                        values.push(format!("<ml:placeholder xmlns:ml=\"{ML}\"/>"));
                    }
                    values.push(replacement);
                    let count = values.len();
                    let matrix = if let Some(range) = &col.matrix_range {
                        let raw = &self.source[range.clone()];
                        // Reuse all matrix attributes, including unsupported metadata.
                        let end = tag_end(raw)?;
                        let mut opening = raw[..=end].to_owned();
                        opening = replace_attribute(&opening, "rows", &count.to_string())?;
                        let close = &raw[raw.rfind("</").ok_or("Matrix closing tag missing")?..];
                        format!(
                            "{opening}{}{}{close}",
                            &raw[end + 1..raw.rfind("</").ok_or("Matrix closing tag missing")?],
                            values[col.cells.len()..].concat()
                        )
                    } else {
                        format!(
                            "<ml:matrix xmlns:ml=\"{ML}\" rows=\"{count}\" cols=\"1\">{}</ml:matrix>",
                            values.concat()
                        )
                    };
                    let value_range = col
                        .matrix_range
                        .clone()
                        .unwrap_or_else(|| col.cells[0].range.clone());
                    (value_range, matrix)
                }
            }
        };
        let mut source = self.source.clone();
        source.replace_range(range, &replacement);
        let updated = Self::parse(&source)?;
        *self = updated;
        Ok(())
    }
}

fn replace_attribute(opening: &str, name: &str, value: &str) -> Result<String, String> {
    // Read the original node with its inherited namespace context externally;
    // matrix row counts are ASCII and cannot introduce XML syntax.
    let bytes = opening.as_bytes();
    let mut at = 1;
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'>' {
        at += 1;
    }
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while at < bytes.len()
            && !bytes[at].is_ascii_whitespace()
            && !matches!(bytes[at], b'=' | b'>')
        {
            at += 1;
        }
        let found = &opening[start..at];
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            break;
        }
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let quote = *bytes.get(at).ok_or("Invalid matrix attribute")?;
        at += 1;
        let value_start = at;
        while at < bytes.len() && bytes[at] != quote {
            at += 1;
        }
        if found == name {
            let mut out = opening.to_owned();
            out.replace_range(value_start..at, value);
            return Ok(out);
        }
        at += 1;
    }
    Err(format!("Matrix {name} attribute missing"))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"<ws:region xmlns:ws="http://schemas.mathsoft.com/worksheet50" xmlns:m="http://schemas.mathsoft.com/math50" xmlns:u="urn:future" region-id="26" left="10" top="20" actualWidth="100" actualHeight="125" u:marker="keep"><ws:spec-table disable-calc="false"><ws:math resultRef="18"><m:define><m:id labels="VARIABLE">I</m:id><m:apply><m:scale/><m:matrix rows = '3' cols="1" u:matrix="keep"><m:real>1.00</m:real><!-- middle --><m:placeholder/><m:real>3.5</m:real></m:matrix><m:id labels="UNIT">A</m:id></m:apply></m:define><ws:formattingOverride FontSize="11"/><u:metadata exact=" keep "/></ws:math><ws:math resultRef="19"><m:define><m:id labels="VARIABLE">B</m:id><m:matrix rows="1" cols="1"><m:real>8</m:real></m:matrix></m:define></ws:math></ws:spec-table></ws:region>"#;
    fn number(value: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(value.into()),
            children: vec![],
        }
    }

    #[test]
    fn spec_table_new_default_uses_native_matrix_storage_and_independent_empty_slots() {
        let table = SpecTable::new(1, 1).unwrap();
        assert_eq!((table.row_count(), table.col_count()), (1, 1));
        for slot in [CellSlot::Header, CellSlot::Unit, CellSlot::Body(0)] {
            assert_eq!(table.cell(0, slot).unwrap(), empty_tree());
        }
        let doc = parse(table.xml()).unwrap();
        let matrix = doc
            .descendants()
            .find(|n| n.has_tag_name((ML, "matrix")))
            .unwrap();
        assert_eq!(matrix.attribute("rows"), Some("1"));
        assert_eq!(matrix.attribute("cols"), Some("1"));
        assert!(matrix.parent().unwrap().has_tag_name((ML, "apply")));
        assert!(SpecTable::new(0, 1).is_err());
        assert!(SpecTable::new(1, 0).is_err());
        assert!(SpecTable::new(1024, 256).is_err());
    }

    #[test]
    fn spec_table_native_copy_dispatches_header_and_unit_to_new_tables_body_to_matrix() {
        let source = SpecTable::parse(SOURCE).unwrap();
        for (range, rows, header, unit, body) in [
            (0..1, 1, true, false, false),
            (0..2, 1, true, true, false),
            (1..2, 1, false, true, false),
            (1..5, 3, false, true, true),
            (0..5, 3, true, true, true),
        ] {
            let payload = source.copy_selection(range, 0..2).unwrap();
            let restored: SpecTableClipboard =
                serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
            assert_eq!(restored, payload);
            let SpecTableClipboard::TableXml(xml) = payload else {
                panic!("header/unit copy must be a table")
            };
            let copy = SpecTable::parse(&xml).unwrap();
            assert_eq!((copy.row_count(), copy.col_count()), (rows, 2));
            for column in 0..2 {
                assert_eq!(
                    copy.cell(column, CellSlot::Header).unwrap(),
                    if header {
                        source.cell(column, CellSlot::Header).unwrap()
                    } else {
                        empty_tree()
                    }
                );
                assert_eq!(
                    copy.cell(column, CellSlot::Unit).unwrap(),
                    if unit {
                        source.cell(column, CellSlot::Unit).unwrap()
                    } else {
                        empty_tree()
                    }
                );
                for row in 0..rows {
                    assert_eq!(
                        copy.cell(column, CellSlot::Body(row)).unwrap(),
                        if body {
                            source.cell(column, CellSlot::Body(row)).unwrap()
                        } else {
                            empty_tree()
                        }
                    );
                }
            }
            assert!(
                copy.columns()
                    .iter()
                    .all(|column| column.result_ref.is_none())
            );
            assert!(!xml.contains("formattingOverride"));
            assert!(!xml.contains("u:metadata"));
            assert!(!xml.contains("u:marker="));
            assert!(!xml.contains("actualWidth="));
            let parsed = parse(&xml).unwrap();
            assert!(
                parsed
                    .descendants()
                    .filter(|node| node.has_tag_name((ML, "matrix")))
                    .all(|node| node.attribute("cols") == Some("1")
                        && node.attribute("rows") == Some(rows.to_string().as_str()))
            );
        }
        let SpecTableClipboard::Matrix(matrix) = source.copy_selection(3..5, 0..2).unwrap() else {
            panic!("body copy must be a matrix")
        };
        assert_eq!(
            matrix.kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
        assert_eq!(
            matrix.children,
            vec![empty_tree(), empty_tree(), number("3.5"), empty_tree()]
        );
        assert_eq!(source.xml(), SOURCE);
    }

    #[test]
    fn spec_table_cut_copies_before_native_removal_and_clear_cells_never_removes_structure() {
        let mut cut = SpecTable::parse(SOURCE).unwrap();
        let copied = cut.copy_selection(0..5, 0..2).unwrap();
        let (payload, changed) = cut.cut_selection(0..5, 0..2).unwrap();
        assert!(changed);
        assert_eq!(payload, copied);
        // This legacy fixture's surviving last column stores just one data
        // slot; native removal must not invent padding after the long column.
        assert_eq!((cut.row_count(), cut.col_count()), (1, 1));
        assert_eq!(cut.cell(0, CellSlot::Body(0)).unwrap(), number("8"));
        let before = cut.clone();
        let (_, changed) = cut.cut_selection(0..3, 0..1).unwrap();
        assert!(!changed);
        assert_eq!(cut, before);

        let mut clear = SpecTable::parse(SOURCE).unwrap();
        assert!(clear.clear_selection(0..5, 0..2).unwrap());
        assert_eq!((clear.row_count(), clear.col_count()), (3, 2));
        for column in 0..2 {
            for row in 0..5 {
                assert_eq!(
                    clear
                        .cell(
                            column,
                            match row {
                                0 => CellSlot::Header,
                                1 => CellSlot::Unit,
                                row => CellSlot::Body(row - 2),
                            }
                        )
                        .unwrap(),
                    empty_tree()
                );
            }
        }
        for original in [
            "u:marker=\"keep\"",
            "<!-- middle -->",
            "u:matrix=\"keep\"",
            "<u:metadata exact=\" keep \"/>",
            "resultRef=\"18\"",
            "<ws:formattingOverride FontSize=\"11\"/>",
        ] {
            assert!(clear.xml().contains(original), "lost {original}");
        }
        let before = clear.clone();
        assert!(!clear.clear_selection(0..5, 0..2).unwrap());
        assert_eq!(clear, before);
    }

    #[test]
    fn spec_table_clipboard_invalid_ranges_are_atomic_and_partial_clear_preserves_other_bytes() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        for (rows, columns) in [
            (0..0, 0..1),
            (0..6, 0..1),
            (0..1, 0..3),
            (Range { start: 4, end: 3 }, 0..1),
        ] {
            assert!(table.copy_selection(rows.clone(), columns.clone()).is_err());
            assert!(table.cut_selection(rows.clone(), columns.clone()).is_err());
            assert!(table.clear_selection(rows, columns).is_err());
            assert_eq!(table.xml(), SOURCE);
        }
        let other = table.columns()[1].definition_mathml.clone();
        assert!(table.clear_selection(4..5, 0..1).unwrap());
        assert_eq!(table.cell(0, CellSlot::Body(2)).unwrap(), empty_tree());
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("1.00"));
        assert!(table.xml().contains("<m:real>1.00</m:real><!-- middle -->"));
        assert_eq!(table.columns()[1].definition_mathml, other);
        let xml = format!(
            "<ws:region xmlns:ws=\"{WS}\">{}</ws:region>",
            "x".repeat(MAX_XML)
        );
        assert!(SpecTable::parse(&xml).is_err());
    }

    #[test]
    fn spec_table_row_and_column_commands_preserve_source_values_metadata_and_minimum_slots() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        assert!(table.insert_row(1).unwrap());
        assert_eq!(table.row_count(), 4);
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("1.00"));
        assert_eq!(table.cell(0, CellSlot::Body(1)).unwrap(), empty_tree());
        assert_eq!(table.cell(0, CellSlot::Body(3)).unwrap(), number("3.5"));
        assert!(table.delete_row(1).unwrap());
        assert_eq!(table.row_count(), 3);
        assert_eq!(table.cell(0, CellSlot::Body(2)).unwrap(), number("3.5"));
        assert!(table.insert_column(1).unwrap());
        assert_eq!(table.col_count(), 3);
        assert_eq!(table.cell(1, CellSlot::Header).unwrap(), empty_tree());
        assert_eq!(table.cell(2, CellSlot::Body(0)).unwrap(), number("8"));
        assert!(table.delete_column(1).unwrap());
        assert_eq!(table.col_count(), 2);
        for preserved in [
            "<!-- middle -->",
            "u:matrix=\"keep\"",
            "<u:metadata exact=\" keep \"/>",
            "<ws:formattingOverride FontSize=\"11\"/>",
        ] {
            assert!(table.xml().contains(preserved), "lost {preserved}");
        }
        assert!(!table.xml().contains("actualWidth=") && !table.xml().contains("actualHeight="));
        let before = table.clone();
        assert!(table.insert_row(20).is_err());
        assert!(table.delete_column(20).is_err());
        assert_eq!(table, before);
    }

    #[test]
    fn spec_table_final_row_and_column_delete_clear_instead_of_removing_the_region() {
        let mut table = SpecTable::new(1, 1).unwrap();
        let header = tree_from_mathml("<ml:id labels=\"VARIABLE\">A</ml:id>").unwrap();
        let unit = tree_from_mathml("<ml:id labels=\"UNIT\">m</ml:id>").unwrap();
        table.set_cell(0, CellSlot::Header, &header).unwrap();
        table.set_cell(0, CellSlot::Unit, &unit).unwrap();
        table.set_cell(0, CellSlot::Body(0), &number("3")).unwrap();
        assert!(table.delete_row(0).unwrap());
        assert!(!table.delete_row(0).unwrap());
        assert_eq!(table.cell(0, CellSlot::Header).unwrap(), header);
        assert_eq!(table.cell(0, CellSlot::Unit).unwrap(), unit);
        assert!(table.delete_column(0).unwrap());
        assert!(!table.delete_column(0).unwrap());
        assert_eq!((table.row_count(), table.col_count()), (1, 1));
        for slot in [CellSlot::Header, CellSlot::Unit, CellSlot::Body(0)] {
            assert_eq!(table.cell(0, slot).unwrap(), empty_tree());
        }
    }

    #[test]
    fn spec_table_rectangle_copy_paste_and_clear_are_structural_atomic_and_do_not_invent_zeros() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let copied = table.copy_matrix(0..2, 0..2).unwrap();
        let with_headers = table.copy_display_matrix(0..3, 0..1).unwrap();
        assert_eq!(
            with_headers.children,
            vec![
                table.cell(0, CellSlot::Header).unwrap(),
                table.cell(0, CellSlot::Unit).unwrap(),
                number("1.00")
            ]
        );
        assert_eq!(
            copied.kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
        assert_eq!(
            copied.children,
            vec![number("1.00"), number("8"), empty_tree(), empty_tree()]
        );
        let pasted = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None,
            },
            children: vec![number("4.00"), number("5"), empty_tree(), number("7")],
        };
        assert!(table.paste_matrix(0, 1, &pasted).unwrap());
        assert_eq!(table.copy_matrix(1..3, 0..2).unwrap(), pasted);
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("1.00"));
        assert_eq!(table.cell(1, CellSlot::Body(0)).unwrap(), number("8"));
        assert!(table.xml().contains("<!-- middle -->"));
        let before = table.clone();
        assert!(table.paste_matrix(0, 2, &pasted).is_err());
        assert!(
            table
                .clear_cells(&[(0, CellSlot::Header), (99, CellSlot::Header)])
                .is_err()
        );
        assert_eq!(table, before);
        assert!(
            table
                .clear_cells(&[
                    (0, CellSlot::Body(1)),
                    (0, CellSlot::Body(1)),
                    (1, CellSlot::Body(2))
                ])
                .unwrap()
        );
        assert_eq!(table.cell(0, CellSlot::Body(1)).unwrap(), empty_tree());
        assert_eq!(table.cell(1, CellSlot::Body(2)).unwrap(), empty_tree());
        assert_eq!((table.row_count(), table.col_count()), (3, 2));
    }

    #[test]
    fn spec_table_new_canvas_visual_cache_never_collides_with_native_ids() {
        use crate::{
            canvas::{CanvasContent, CanvasRegion},
            native_visual::NativeVisuals,
        };
        let table = SpecTable::new(1, 1).unwrap();
        let region = CanvasRegion {
            id: 1,
            left: 10.,
            top: 20.,
            content: CanvasContent::Opaque {
                kind: "spec-table".into(),
                xml: table.xml().into(),
            },
            source_region: None,
            rich_text: None,
            authored_text: None,
            format: Default::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        };
        let mut visuals = NativeVisuals::default();
        visuals
            .regions
            .insert("canvas:1".into(), Err("source-marker".into()));
        visuals.update_spec_table(&region).unwrap();
        assert!(visuals.region(&region).unwrap().is_ok());
        let mut native = region.clone();
        native.source_region = Some("canvas:1".into());
        assert_eq!(
            visuals.region(&native).unwrap().as_ref().unwrap_err(),
            "source-marker"
        );
    }

    #[test]
    fn spec_table_range_delete_obeys_column_priority_header_guards_and_retains_last_original_slot()
    {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        // All columns/all rows uses the full-column branch and keeps B's data.
        assert!(table.remove_or_clear_selection(0..5, 0..2).unwrap());
        assert_eq!(table.col_count(), 1);
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("8"));
        assert!(!table.remove_or_clear_selection(0..3, 0..1).unwrap());
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let before = table.clone();
        assert!(!table.remove_or_clear_selection(0..1, 0..2).unwrap());
        assert_eq!(table, before);
        // Removing all data rows retains the original final row, not an empty
        // replacement and not the first row.
        assert!(table.remove_or_clear_selection(2..5, 0..2).unwrap());
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("3.5"));
        assert!(!table.remove_or_clear_selection(2..3, 0..2).unwrap());
        let mut table = SpecTable::parse(SOURCE).unwrap();
        assert!(table.remove_or_clear_selection(2..3, 0..1).unwrap());
        assert_eq!((table.row_count(), table.col_count()), (3, 2));
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), empty_tree());
        assert_eq!(table.cell(1, CellSlot::Body(0)).unwrap(), number("8"));
        let before = table.clone();
        assert!(table.remove_or_clear_selection(0..20, 0..1).is_err());
        assert_eq!(table, before);
    }

    #[test]
    fn spec_table_parses_structural_slots_and_noop_preserves_every_original_byte() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        assert_eq!((table.col_count(), table.row_count()), (2, 3));
        assert_eq!(table.cell(0, CellSlot::Body(0)).unwrap(), number("1.00"));
        assert_eq!(table.cell(1, CellSlot::Unit).unwrap(), empty_tree());
        assert_eq!(table.cell(1, CellSlot::Body(2)).unwrap(), empty_tree());
        assert_eq!(table.columns()[0].result_ref.as_deref(), Some("18"));
        let same = table.cell(0, CellSlot::Header).unwrap();
        table.set_cell(0, CellSlot::Header, &same).unwrap();
        assert_eq!(table.xml(), SOURCE);
        for definition in table.definition_mathmls().unwrap() {
            assert!(parse(&definition).is_ok());
        }
        assert!(table.definition_mathmls().unwrap()[0].contains("1.00"));
    }

    #[test]
    fn spec_table_edits_one_subtree_and_preserves_source_format_units_and_other_columns() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let other = table.columns()[1].definition_mathml.clone();
        let expression = tree_from_mathml(
            "<ml:apply><ml:plus/><ml:real>2</ml:real><ml:real>3</ml:real></ml:apply>",
        )
        .unwrap();
        table.set_cell(0, CellSlot::Body(1), &expression).unwrap();
        assert_eq!(table.cell(0, CellSlot::Body(1)).unwrap(), expression);
        for untouched in [
            "<m:real>1.00</m:real>",
            "<!-- middle -->",
            "u:matrix=\"keep\"",
            "<u:metadata exact=\" keep \"/>",
            "<ws:formattingOverride FontSize=\"11\"/>",
            "resultRef=\"18\"",
        ] {
            assert!(table.xml().contains(untouched), "lost {untouched}");
        }
        assert_eq!(table.columns()[1].definition_mathml, other);
        table.set_cell(0, CellSlot::Unit, &empty_tree()).unwrap();
        assert_eq!(table.cell(0, CellSlot::Unit).unwrap(), empty_tree());
        assert!(table.xml().contains("<m:scale/>") && table.xml().contains("<ml:placeholder"));
    }

    #[test]
    fn spec_table_missing_unit_and_short_column_edits_expand_without_filling_internal_blanks() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let unit = tree_from_mathml("<ml:id labels=\"UNIT\">m</ml:id>").unwrap();
        table.set_cell(1, CellSlot::Unit, &unit).unwrap();
        table.set_cell(1, CellSlot::Body(2), &number("12")).unwrap();
        assert_eq!(table.cell(1, CellSlot::Body(1)).unwrap(), empty_tree());
        assert_eq!(table.cell(1, CellSlot::Body(2)).unwrap(), number("12"));
        table.set_cell(0, CellSlot::Body(3), &number("20")).unwrap();
        assert_eq!(table.row_count(), 4);
        assert!(
            table
                .xml()
                .contains("rows = '4' cols=\"1\" u:matrix=\"keep\"")
        );
        assert_eq!(table.cell(0, CellSlot::Body(3)).unwrap(), number("20"));
    }

    #[test]
    fn spec_table_rejects_invalid_xml_dimensions_statements_and_out_of_bounds_atomically() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let statement =
            tree_from_mathml("<ml:define><ml:id>A</ml:id><ml:real>1</ml:real></ml:define>")
                .unwrap();
        assert!(table.set_cell(0, CellSlot::Body(0), &statement).is_err());
        assert!(table.set_cell(2, CellSlot::Header, &number("1")).is_err());
        assert!(table.set_cell(0, CellSlot::Body(5), &number("1")).is_err());
        assert_eq!(table.xml(), SOURCE);
        assert!(SpecTable::parse(&SOURCE.replace("rows = '3'", "rows='2'")).is_err());
        assert!(SpecTable::parse("<!DOCTYPE x [<!ENTITY a 'x'>]><x>&a;</x>").is_err());
        let spaced = SOURCE.replace("xmlns:m=", "xmlns:m \t = ");
        assert!(SpecTable::parse(&spaced).is_ok());
    }

    #[test]
    fn spec_table_updated_native_visual_uses_current_expression_and_reports_bad_source() {
        use crate::{
            canvas::{CanvasContent, CanvasRegion},
            native_visual::{Block, NativeVisuals},
        };
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let expression = tree_from_mathml(
            "<ml:apply><ml:plus/><ml:real>2</ml:real><ml:real>3</ml:real></ml:apply>",
        )
        .unwrap();
        table.set_cell(0, CellSlot::Body(1), &expression).unwrap();
        let mut region = CanvasRegion {
            id: 1,
            left: 10.,
            top: 20.,
            content: CanvasContent::Opaque {
                kind: "spec-table".into(),
                xml: table.xml().into(),
            },
            source_region: Some("26".into()),
            rich_text: None,
            authored_text: None,
            format: Default::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        };
        let mut visuals = NativeVisuals::default();
        visuals.update_spec_table(&region).unwrap();
        let visual = visuals.region(&region).unwrap().as_ref().unwrap();
        let Block::Table(display) = &visual.document.as_ref().unwrap().blocks[0] else {
            panic!("table")
        };
        let edited = display
            .cells
            .iter()
            .find(|c| c.column == 0 && c.row == 3)
            .unwrap();
        assert!(matches!(&edited.blocks[0],Block::Math(m) if m.tree==expression));
        if let CanvasContent::Opaque { xml, .. } = &mut region.content {
            *xml = "<broken>".into();
        }
        assert!(visuals.update_spec_table(&region).is_err());
        assert!(visuals.region(&region).unwrap().is_err());
    }

    #[test]
    #[ignore = "Reads explicit user-selected local fixtures without executing original code"]
    fn inspect_selected_spec_table_edit_roundtrips() {
        use crate::native::NativePackage;
        let files = std::env::var("MATHCRAFT_SPEC_TABLE_FILES")
            .expect("Set explicit fixture paths separated by semicolons");
        for path in files.split(';').filter(|p| !p.is_empty()) {
            let before = std::fs::read(path).unwrap();
            let package = NativePackage::from_bytes(&before).unwrap();
            let source = package.worksheet_xml().unwrap();
            let doc = parse(&source).unwrap();
            let region = doc
                .descendants()
                .find(|n| {
                    n.has_tag_name((WS, "region"))
                        && n.children().any(|c| c.has_tag_name((WS, "spec-table")))
                })
                .unwrap();
            let raw = standalone(region, &source).unwrap();
            let mut table = SpecTable::parse(&raw).unwrap();
            assert_eq!((table.col_count(), table.row_count()), (6, 10));
            let values = table
                .columns()
                .iter()
                .flat_map(|c| &c.cells)
                .filter(|c| !matches!(c.tree.kind, NodeKind::Placeholder))
                .count();
            assert_eq!(values, 55);
            assert_eq!(table.xml(), raw);
            let SpecTableClipboard::TableXml(copied_xml) =
                table.copy_selection(0..12, 0..6).unwrap()
            else {
                panic!("native full table copy")
            };
            let copied = SpecTable::parse(&copied_xml).unwrap();
            assert_eq!((copied.row_count(), copied.col_count()), (10, 6));
            assert!(
                copied
                    .columns()
                    .iter()
                    .all(|column| column.result_ref.is_none())
            );
            for column in 0..6 {
                for row in 0..12 {
                    let slot = match row {
                        0 => CellSlot::Header,
                        1 => CellSlot::Unit,
                        row => CellSlot::Body(row - 2),
                    };
                    assert_eq!(
                        copied.cell(column, slot).unwrap(),
                        table.cell(column, slot).unwrap()
                    );
                }
            }
            let mut cleared = table.clone();
            assert!(cleared.clear_selection(0..12, 0..6).unwrap());
            assert_eq!((cleared.row_count(), cleared.col_count()), (10, 6));
            assert!(cleared.columns().iter().all(|column| {
                matches!(column.header.tree.kind, NodeKind::Placeholder)
                    && column
                        .cells
                        .iter()
                        .all(|cell| matches!(cell.tree.kind, NodeKind::Placeholder))
            }));
            let mut cut = table.clone();
            let (payload, changed) = cut.cut_selection(0..12, 0..6).unwrap();
            assert!(changed);
            assert_eq!(payload, SpecTableClipboard::TableXml(copied_xml));
            assert_eq!((cut.row_count(), cut.col_count()), (10, 1));
            assert_eq!(
                cut.cell(0, CellSlot::Header).unwrap(),
                table.cell(5, CellSlot::Header).unwrap()
            );
            let original = table.cell(0, CellSlot::Body(0)).unwrap();
            table
                .set_cell(0, CellSlot::Body(0), &number("123.00"))
                .unwrap();
            assert_eq!(
                SpecTable::parse(table.xml())
                    .unwrap()
                    .cell(0, CellSlot::Body(0))
                    .unwrap(),
                number("123.00")
            );
            table.set_cell(0, CellSlot::Body(0), &original).unwrap();
            let definitions = table.definition_mathmls().unwrap();
            table.insert_row(3).unwrap();
            table.insert_column(2).unwrap();
            assert_eq!((table.row_count(), table.col_count()), (11, 7));
            assert_eq!(table.cell(2, CellSlot::Header).unwrap(), empty_tree());
            table.delete_column(2).unwrap();
            table.delete_row(3).unwrap();
            assert_eq!(table.definition_mathmls().unwrap(), definitions);
            assert_eq!(std::fs::read(path).unwrap(), before);
            println!(
                "{path}: 6 columns, 10 rows, 55 values, typed table Copy/Cut/Clear, body edit and row/column insert-delete roundtrip, source unchanged"
            );
        }
    }
    #[test]
    fn source_display_clipboard_is_not_silently_consumed_by_table_paste() {
        let mut table = SpecTable::parse(SOURCE).unwrap();
        let mut matrix = table.copy_matrix(0..2, 0..2).unwrap();
        let NodeKind::Matrix { display, .. } = &mut matrix.kind else {
            unreachable!()
        };
        *display = Some(mathcraft_editor::SourceMatrixDisplay::default());
        let before = table.clone();
        assert!(
            table
                .paste_matrix(0, 0, &matrix)
                .unwrap_err()
                .contains("Source matrix display")
        );
        assert_eq!(table, before);
        assert!(tree_to_mathml(&matrix).unwrap().contains("<ml:display/>"));
    }
}
