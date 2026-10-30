//! Operation-local raw-serializer traces. No trace is retained by either cache.
use super::*;
use std::{ops::Range, sync::Arc};
const MAX_BYTES: usize = 1024 * 1024;
const SCRATCH_BYTES: usize =
    std::mem::size_of::<TraceCursor>() + std::mem::size_of::<MathmlTraces>();
pub(in crate::editor_bridge) struct IdentifierSpan {
    pub path: Box<[usize]>,
    pub range: Range<usize>,
}
pub(in crate::editor_bridge) struct IdentifierSpans {
    pub spans: Vec<IdentifierSpan>,
    pub bytes: usize,
}
pub(in crate::editor_bridge) struct TraceCursor {
    path: [usize; 64],
    path_len: usize,
    data: Option<IdentifierSpans>,
    budget: usize,
}
impl TraceCursor {
    pub fn disabled() -> Self {
        Self {
            path: [0; 64],
            path_len: 0,
            data: None,
            budget: 0,
        }
    }
    pub fn new(budget: usize) -> Self {
        if budget == 0 {
            return Self::disabled();
        }
        Self {
            path: [0; 64],
            path_len: 0,
            data: Some(IdentifierSpans {
                spans: Vec::new(),
                bytes: 0,
            }),
            budget,
        }
    }
    pub fn push(&mut self, index: usize) {
        if self.data.is_some() {
            self.path[self.path_len] = index;
            self.path_len += 1;
        }
    }
    pub fn pop(&mut self) {
        if self.data.is_some() {
            self.path_len -= 1;
        }
    }
    pub fn disable(&mut self) {
        self.data = None;
        self.path_len = 0;
    }
    pub fn record(&mut self, range: Range<usize>) {
        let Some(data) = &mut self.data else {
            return;
        };
        let capacity = if data.spans.len() == data.spans.capacity() {
            data.spans.capacity().saturating_mul(2).max(1)
        } else {
            data.spans.capacity()
        };
        let extra = (capacity - data.spans.capacity()) * std::mem::size_of::<IdentifierSpan>()
            + self.path_len * std::mem::size_of::<usize>();
        if data.bytes.saturating_add(extra) > self.budget {
            self.disable();
            return;
        }
        if data.spans.len() == data.spans.capacity() {
            data.spans.reserve_exact(capacity - data.spans.len());
        }
        data.bytes += extra;
        data.spans.push(IdentifierSpan {
            path: self.path[..self.path_len].into(),
            range,
        });
    }
    pub fn finish(self) -> Option<IdentifierSpans> {
        self.data
    }
}
/// Opaque strong owner ties every recorded AST path/range to one immutable
/// proof, never a pointer-only source cache identity.
pub struct MathmlTrace {
    pub(super) owner: Arc<TypedMathml>,
    pub(super) identifiers: IdentifierSpans,
}
/// New trace metadata, including the lookup Vec and path scratch, is bounded
/// to1MiB for this preparation operation. Exhaustion chooses the full writer.
pub struct MathmlTraces {
    entries: Vec<(u64, MathmlTrace)>,
    bytes: usize,
    slots: usize,
}
// Repeated scalar measurements did not establish a consistent whole-frame
// gain. Trace only the dense matrix bodies with a measured net benefit; this
// is a conservative cost gate, never an acceptance or semantics restriction.
fn trace_worthwhile(node: &PortableTree) -> bool {
    matches!(node.kind,NodeKind::Matrix{rows,cols, .. } if rows.saturating_mul(cols)>=256)
        || node.children.iter().any(trace_worthwhile)
}
impl MathmlTraces {
    pub fn new(regions: usize) -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
            slots: regions
                .min((MAX_BYTES - SCRATCH_BYTES) / std::mem::size_of::<(u64, MathmlTrace)>()),
        }
    }
    pub fn new_math(&mut self, id: u64, tree: &PortableTree) -> Result<Arc<TypedMathml>, String> {
        let eligible = validate_mathml_tree(tree)?;
        let trace_eligible = eligible && trace_worthwhile(tree);
        // Source-cache hits, small fragments and unsupported proofs never
        // allocate this index; all still use their full validated serializer.
        if trace_eligible && self.entries.capacity() == 0 && self.slots > 0 {
            self.entries.reserve_exact(self.slots);
            self.bytes = self.entries.capacity() * std::mem::size_of::<(u64, MathmlTrace)>();
        }
        let budget = if self.entries.len() < self.entries.capacity() {
            MAX_BYTES.saturating_sub(SCRATCH_BYTES + self.bytes)
        } else {
            0
        };
        let (source, identifiers) = if trace_eligible && budget > 0 {
            serialization_writer::serialize_traced(tree, budget)?
        } else {
            (
                if eligible {
                    serialization_writer::serialize(tree, true, 0, None)?
                } else {
                    serialize(tree, true, 0)?
                },
                None,
            )
        };
        let proof = Arc::new(TypedMathml {
            source,
            tree: tree.clone(),
            eligible,
        });
        if let Some(identifiers) = identifiers {
            self.bytes += identifiers.bytes;
            self.entries.push((
                id,
                MathmlTrace {
                    owner: proof.clone(),
                    identifiers,
                },
            ));
        }
        debug_assert!(self.bytes + SCRATCH_BYTES <= MAX_BYTES);
        Ok(proof)
    }
    pub(in crate::editor_bridge) fn sort(&mut self) {
        self.entries.sort_unstable_by_key(|(id, _)| *id);
    }
    pub(in crate::editor_bridge) fn get(
        &self,
        id: u64,
        proof: &Arc<TypedMathml>,
    ) -> Option<&MathmlTrace> {
        let i = self.entries.binary_search_by_key(&id, |(id, _)| *id).ok()?;
        let trace = &self.entries[i].1;
        Arc::ptr_eq(&trace.owner, proof).then_some(trace)
    }
    #[cfg(test)]
    pub(in crate::editor_bridge) fn bytes(&self) -> usize {
        self.bytes + SCRATCH_BYTES
    }
}
