use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

const ML: &str = "http://schemas.mathsoft.com/math50";

fn parse(source: &str) -> Result<PortableTree, String> {
    tree_from_mathml(&format!("<context xmlns:ml='{ML}'>{source}</context>"))
}

#[test]
fn row_and_column_definition_targets_roundtrip_native_operators_and_metadata() {
    for (operator, kind) in [("matrow", NodeKind::Row), ("matcol", NodeKind::Column)] {
        for (tag, definition) in [
            ("define", BinaryOp::Define),
            ("globalDefine", BinaryOp::GlobalDefine),
        ] {
            let source = format!(
                "<ml:{tag}><ml:apply><ml:{operator}/><ml:id labels='VARIABLE' label-is-contextual='false'><Span xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation'>M<Subscript xmlns='clr-namespace:Ptc.Wpf;assembly=Ptc.Core'>dc</Subscript></Span></ml:id><ml:id labels='SYSTEM'>ORIGIN</ml:id></ml:apply><ml:eval><ml:matrix rows='2' cols='1'><ml:real>4</ml:real><ml:real>5</ml:real></ml:matrix><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></ml:{tag}>"
            );
            let tree = parse(&source).unwrap();
            assert_eq!(tree.kind, NodeKind::Binary(definition));
            assert_eq!(tree.children[0].kind, kind);
            let native = tree_to_mathml(&tree).unwrap();
            assert!(native.contains(&format!("<ml:{operator}/>")));
            assert!(native.contains("labels=\"VARIABLE\""));
            assert_eq!(tree_from_mathml(&native).unwrap(), tree);
        }
    }
}

#[test]
fn unsupported_slice_targets_preserve_the_raw_boundary_in_both_directions() {
    for operator in ["matrow", "matcol"] {
        for tag in ["define", "globalDefine"] {
            for target in [
                "<ml:real>3</ml:real>",
                "<ml:apply><ml:plus/><ml:id>M</ml:id><ml:id>N</ml:id></ml:apply>",
            ] {
                assert!(parse(&format!("<ml:{tag}><ml:apply><ml:{operator}/>{target}<ml:real>0</ml:real></ml:apply><ml:real>4</ml:real></ml:{tag}>")).is_err());
            }
            for slice in [
                format!("<ml:apply><ml:{operator}/><ml:id>M</ml:id></ml:apply>"),
                format!(
                    "<ml:apply><ml:{operator} unknown='true'/><ml:id>M</ml:id><ml:real>0</ml:real></ml:apply>"
                ),
                format!(
                    "<ml:apply><foreign:{operator} xmlns:foreign='urn:foreign'/><ml:id>M</ml:id><ml:real>0</ml:real></ml:apply>"
                ),
            ] {
                assert!(
                    parse(&format!("<ml:{tag}>{slice}<ml:real>4</ml:real></ml:{tag}>")).is_err()
                );
            }
        }
    }
    let valid = parse("<ml:define><ml:apply><ml:matrow/><ml:id>M</ml:id><ml:real>0</ml:real></ml:apply><ml:real>4</ml:real></ml:define>").unwrap();
    let mut invalid = valid.clone();
    invalid.children[0].children[0] = PortableTree {
        kind: NodeKind::Number("3".into()),
        children: vec![],
    };
    assert!(tree_to_mathml(&invalid).is_err());
    assert!(
        tree_to_mathml(&PortableTree {
            kind: NodeKind::Vectorize,
            children: vec![valid]
        })
        .is_err()
    );
}

#[test]
fn slice_names_are_declared_without_forcing_variable_labels_on_their_indices() {
    for operator in ["matrow", "matcol"] {
        for tag in ["define", "globalDefine"] {
            let rows = vec![
                (1, 0., 0., "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),
                (2, 0., 40., format!("<ml:{tag}><ml:apply><ml:{operator}/><ml:id>m</ml:id><ml:apply><ml:plus/><ml:id>ORIGIN</ml:id><ml:id>s</ml:id></ml:apply></ml:apply><ml:real>4</ml:real></ml:{tag}>")),
            ];
            let xml = worksheet_xml(&rows).unwrap();
            let doc = roxmltree::Document::parse(&xml).unwrap();
            let identifiers: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name((ML, "id")))
                .collect();
            assert_eq!(identifiers[1].attribute("labels"), Some("VARIABLE"));
            assert_eq!(identifiers[2].attribute("labels"), Some("SYSTEM"));
            assert_eq!(identifiers[3].attribute("labels"), Some("UNIT"));
            assert_eq!(
                identifiers[0].attribute("labels"),
                Some(if tag == "globalDefine" {
                    "VARIABLE"
                } else {
                    "UNIT"
                })
            );
        }
    }
}

#[test]
fn program_slice_assignments_roundtrip_and_keep_loop_and_context_restrictions() {
    for operator in ["matrow", "matcol", "indexer"] {
        let assignment = format!(
            "<ml:localDefine><ml:apply><ml:{operator}/><ml:id>m</ml:id><ml:id>ORIGIN</ml:id></ml:apply><ml:real>4</ml:real></ml:localDefine>"
        );
        let source = format!("<ml:program>{assignment}<ml:id>m</ml:id></ml:program>");
        let tree = parse(&source).unwrap();
        let native = tree_to_mathml(&tree).unwrap();
        assert_eq!(tree_from_mathml(&native).unwrap(), tree);
        // The reader also accepts standalone structural clipboard fragments;
        // the writer requires an actual program host for local assignments.
        assert_eq!(parse(&assignment).unwrap(), tree.children[0]);
        assert!(tree_to_mathml(&tree.children[0]).is_err());
        let worksheet = worksheet_xml(&[(
            1,
            0.,
            0.,
            format!("<ml:define><ml:id>result</ml:id>{source}</ml:define>"),
        )])
        .unwrap();
        let doc = roxmltree::Document::parse(&worksheet).unwrap();
        let identifiers: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name((ML, "id")))
            .collect();
        assert_eq!(identifiers[1].text(), Some("m"));
        assert_eq!(identifiers[1].attribute("labels"), Some("VARIABLE"));
        assert_eq!(identifiers[2].attribute("labels"), Some("SYSTEM"));
        assert_eq!(identifiers[3].attribute("labels"), Some("VARIABLE"));
        let mut invalid = tree.clone();
        invalid.children[0].children[0].children[0] = PortableTree {
            kind: NodeKind::Number("3".into()),
            children: vec![],
        };
        assert!(invalid.validate().is_err());
        assert!(tree_to_mathml(&invalid).is_err());
        let mut loop_tree = tree;
        loop_tree.children[0].kind = NodeKind::For;
        loop_tree.children[0].children.push(PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        });
        assert!(loop_tree.validate().is_err());
    }
}

#[test]
fn program_slice_local_labels_do_not_escape_their_program_scope() {
    let source = "<ml:define><ml:id>answer</ml:id><ml:apply><ml:plus/><ml:program><ml:localDefine><ml:apply><ml:matcol/><ml:id>m</ml:id><ml:real>0</ml:real></ml:apply><ml:real>4</ml:real></ml:localDefine><ml:id>m</ml:id></ml:program><ml:id>m</ml:id></ml:apply></ml:define>";
    let xml = worksheet_xml(&[(1, 0., 0., source.into())]).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let identifiers: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((ML, "id")) && n.text() == Some("m"))
        .collect();
    assert_eq!(identifiers.len(), 3);
    assert_eq!(identifiers[0].attribute("labels"), Some("VARIABLE"));
    assert_eq!(identifiers[1].attribute("labels"), Some("VARIABLE"));
    assert_eq!(identifiers[2].attribute("labels"), Some("UNIT"));
}
