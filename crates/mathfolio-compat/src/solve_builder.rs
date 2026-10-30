//! Safe construction of the supported native worksheet50 solve-block subset.
//!
//! Callers provide structural `PortableTree` expressions. This module owns all
//! worksheet tags, category metadata and solver identifiers; it has no raw XML
//! insertion API. See docs/analysis/0028-solve-block-builder.md.

use crate::editor_bridge::tree_to_mathml;
use mathfolio_editor::{BinaryOp, NodeKind, PortableTree};
use std::collections::HashSet;

const RESULT_REF: &str = "solve-result";
const MAX_NAME_BYTES: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveMethod {
    Find,
    Minerr,
}

impl SolveMethod {
    fn keyword(self) -> &'static str {
        match self {
            Self::Find => "Find",
            Self::Minerr => "Minerr",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveBuildError {
    detail: String,
}

impl SolveBuildError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for SolveBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for SolveBuildError {}

#[derive(Clone, Debug)]
struct GuessVariable {
    name: String,
    value: PortableTree,
}

#[derive(Clone, Debug)]
struct EqualityConstraint {
    left: PortableTree,
    right: PortableTree,
}

#[derive(Clone, Debug)]
pub struct SolveBlockBuilder {
    method: SolveMethod,
    guesses: Vec<GuessVariable>,
    equalities: Vec<EqualityConstraint>,
    variables: Vec<String>,
    output: Option<String>,
}

impl SolveBlockBuilder {
    pub fn new(method: SolveMethod) -> Self {
        Self {
            method,
            guesses: Vec::new(),
            equalities: Vec::new(),
            variables: Vec::new(),
            output: None,
        }
    }

    pub fn guess(
        &mut self,
        name: impl Into<String>,
        value: PortableTree,
    ) -> Result<&mut Self, SolveBuildError> {
        let name = name.into();
        validate_name(&name)?;
        validate_expression(&value)?;
        if self.guesses.iter().any(|guess| guess.name == name) {
            return Err(SolveBuildError::new("Duplicate solve-block guess name"));
        }
        if self.guesses.len() >= 16 {
            return Err(SolveBuildError::new(
                "Solve-block guess count exceeds the supported limit",
            ));
        }
        self.guesses.push(GuessVariable { name, value });
        Ok(self)
    }

    pub fn equality(
        &mut self,
        left: PortableTree,
        right: PortableTree,
    ) -> Result<&mut Self, SolveBuildError> {
        validate_expression(&left)?;
        validate_expression(&right)?;
        if self.equalities.len() >= 64 {
            return Err(SolveBuildError::new(
                "Solve-block equality count exceeds the supported limit",
            ));
        }
        self.equalities.push(EqualityConstraint { left, right });
        Ok(self)
    }

    pub fn solve_for<I, S>(&mut self, variables: I) -> Result<&mut Self, SolveBuildError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let variables = variables.into_iter().map(Into::into).collect::<Vec<_>>();
        if variables.is_empty() || variables.len() > 16 {
            return Err(SolveBuildError::new(
                "Solve variable count must be between 1 and 16",
            ));
        }
        let mut seen = HashSet::new();
        for variable in &variables {
            validate_name(variable)?;
            if !seen.insert(variable.clone()) {
                return Err(SolveBuildError::new("Duplicate solve variable"));
            }
        }
        self.variables = variables;
        Ok(self)
    }

    pub fn export_as(&mut self, output: impl Into<String>) -> Result<&mut Self, SolveBuildError> {
        let output = output.into();
        validate_name(&output)?;
        self.output = Some(output);
        Ok(self)
    }

    pub fn build(self) -> Result<SolveBlockModel, SolveBuildError> {
        if self.variables.is_empty() {
            return Err(SolveBuildError::new("Solve variables were not selected"));
        }
        let guess_names = self
            .guesses
            .iter()
            .map(|guess| guess.name.as_str())
            .collect::<HashSet<_>>();
        if self.guesses.len() != self.variables.len()
            || self
                .variables
                .iter()
                .any(|variable| !guess_names.contains(variable.as_str()))
        {
            return Err(SolveBuildError::new(
                "Every selected solve variable requires exactly one local guess",
            ));
        }
        match self.method {
            SolveMethod::Find if self.equalities.len() != self.variables.len() => {
                return Err(SolveBuildError::new(
                    "Find requires one equality per solve variable in this subset",
                ));
            }
            SolveMethod::Minerr if self.equalities.len() < self.variables.len() => {
                return Err(SolveBuildError::new(
                    "Minerr requires at least as many equalities as solve variables",
                ));
            }
            _ => {}
        }
        if self
            .output
            .as_ref()
            .is_some_and(|output| guess_names.contains(output.as_str()))
        {
            return Err(SolveBuildError::new(
                "Solver output must not replace a local guess variable",
            ));
        }
        // Revalidate before sealing the model. Its fields remain private, so a
        // valid model cannot later acquire an unchecked tree.
        for guess in &self.guesses {
            validate_expression(&guess.value)?;
        }
        for equality in &self.equalities {
            validate_expression(&equality.left)?;
            validate_expression(&equality.right)?;
        }
        Ok(SolveBlockModel {
            method: self.method,
            guesses: self.guesses,
            equalities: self.equalities,
            variables: self.variables,
            output: self.output,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SolveBlockModel {
    method: SolveMethod,
    guesses: Vec<GuessVariable>,
    equalities: Vec<EqualityConstraint>,
    variables: Vec<String>,
    output: Option<String>,
}

impl SolveBlockModel {
    pub fn method(&self) -> SolveMethod {
        self.method
    }

    pub fn variables(&self) -> &[String] {
        &self.variables
    }

    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    pub fn result_ref(&self) -> &'static str {
        RESULT_REF
    }

    pub fn to_solveblock_xml(&self) -> Result<String, SolveBuildError> {
        let mut regions = String::new();
        let mut top = 0_u64;
        for (index, guess) in self.guesses.iter().enumerate() {
            let definition = definition_tree(&guess.name, guess.value.clone());
            let math = tree_to_mathml(&definition).map_err(SolveBuildError::new)?;
            push_region(
                &mut regions,
                &format!("guess-{index}"),
                top,
                "guess-value",
                None,
                &math,
            );
            top += 100;
        }
        for (index, equality) in self.equalities.iter().enumerate() {
            let left = expression_xml(&equality.left)?;
            let right = expression_xml(&equality.right)?;
            let math = format!(
                "<ml:apply xmlns:ml=\"{}\"><ml:equal/>{left}{right}</ml:apply>",
                crate::ML
            );
            push_region(
                &mut regions,
                &format!("constraint-{index}"),
                top,
                "constraint",
                None,
                &math,
            );
            top += 100;
        }
        let call = solver_call(self.method, &self.variables);
        let evaluation = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                call,
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        };
        let statement = self
            .output
            .as_ref()
            .map(|output| definition_tree(output, evaluation.clone()))
            .unwrap_or(evaluation);
        let solver = tree_to_mathml(&statement).map_err(SolveBuildError::new)?;
        push_region(
            &mut regions,
            "solver",
            top,
            "solver",
            Some(RESULT_REF),
            &solver,
        );
        let xml = format!(
            "<solveblock xmlns=\"{}\" xmlns:ml=\"{}\"><regions>{regions}</regions></solveblock>",
            crate::WS,
            crate::ML
        );
        let document = roxmltree::Document::parse(&xml)
            .map_err(|error| SolveBuildError::new(format!("Generated solve-block XML: {error}")))?;
        crate::solve::parse_block(document.root_element(), None).map_err(|fault| {
            SolveBuildError::new(format!(
                "Unsupported generated solve block: {}",
                fault.detail
            ))
        })?;
        Ok(xml)
    }

    pub fn to_worksheet_xml(
        &self,
        region_id: u64,
        left: f64,
        top: f64,
    ) -> Result<String, SolveBuildError> {
        if !left.is_finite() || !top.is_finite() {
            return Err(SolveBuildError::new(
                "Solve-block worksheet coordinates must be finite",
            ));
        }
        let solveblock = self.to_solveblock_xml()?;
        let xml = format!(
            "<worksheet xmlns=\"{}\" xmlns:ml=\"{}\"><regions><region region-id=\"{region_id}\" left=\"{left}\" top=\"{top}\">{solveblock}</region></regions></worksheet>",
            crate::WS,
            crate::ML
        );
        roxmltree::Document::parse(&xml)
            .map_err(|error| SolveBuildError::new(format!("Generated worksheet XML: {error}")))?;
        Ok(xml)
    }
}

fn validate_name(name: &str) -> Result<(), SolveBuildError> {
    if name.trim().is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
        return Err(SolveBuildError::new("Invalid solve-block identifier"));
    }
    variable_tree(name)
        .validate()
        .map_err(|error| SolveBuildError::new(format!("Invalid solve-block identifier: {error}")))
}

fn validate_expression(expression: &PortableTree) -> Result<(), SolveBuildError> {
    expression
        .validate()
        .map_err(|error| SolveBuildError::new(format!("Invalid solve expression: {error}")))?;
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if matches!(
            node.kind,
            NodeKind::Placeholder
                | NodeKind::Binary(BinaryOp::Define)
                | NodeKind::Binary(BinaryOp::Evaluate)
        ) {
            return Err(SolveBuildError::new(
                "Solve expressions must be complete values, not statements or placeholders",
            ));
        }
        pending.extend(&node.children);
    }
    expression_xml(expression).map(|_| ())
}

fn expression_xml(expression: &PortableTree) -> Result<String, SolveBuildError> {
    let xml = tree_to_mathml(expression).map_err(SolveBuildError::new)?;
    let document = roxmltree::Document::parse(&xml)
        .map_err(|error| SolveBuildError::new(format!("Serialized solve expression: {error}")))?;
    crate::parse_expression(document.root_element(), 0).map_err(|fault| {
        SolveBuildError::new(format!("Unsupported solve expression: {}", fault.detail))
    })?;
    Ok(xml)
}

fn variable_tree(name: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::AnnotatedIdentifier {
            name: name.into(),
            labels: "VARIABLE".into(),
        },
        children: vec![],
    }
}

fn definition_tree(name: &str, value: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(BinaryOp::Define),
        children: vec![variable_tree(name), value],
    }
}

fn solver_call(method: SolveMethod, variables: &[String]) -> PortableTree {
    let mut children = vec![PortableTree {
        kind: NodeKind::AnnotatedIdentifier {
            name: method.keyword().into(),
            labels: "KEYWORD".into(),
        },
        children: vec![],
    }];
    children.extend(variables.iter().map(|variable| variable_tree(variable)));
    PortableTree {
        kind: NodeKind::Call {
            argument_count: variables.len(),
        },
        children,
    }
}

fn push_region(
    output: &mut String,
    id: &str,
    top: u64,
    category: &str,
    result_ref: Option<&str>,
    math: &str,
) {
    let result_ref = result_ref
        .map(|value| format!(" resultRef=\"{value}\""))
        .unwrap_or_default();
    output.push_str(&format!(
        "<region region-id=\"{id}\" left=\"0\" top=\"{top}\" solve-block-category=\"{category}\"><math{result_ref}>{math}</math></region>"
    ));
}
