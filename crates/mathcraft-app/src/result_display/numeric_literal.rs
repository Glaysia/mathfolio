//! Inert numeric coefficients must retain the meaning of native metadata.
use roxmltree::Node;

pub(super) fn coefficient(node: Node<'_, '_>, imaginary: bool) -> Option<f64> {
    if node.children().any(|n| !n.is_text())
        || node.attributes().any(|a| {
            a.namespace().is_some()
                || if imaginary {
                    a.name() != "symbol" || !matches!(a.value(), "i" | "j")
                } else {
                    a.name() != "base"
                }
        })
    {
        return None;
    }
    let text = node.text()?;
    let value: f64 = match node.attribute("base") {
        None | Some("10") => text.trim().parse().ok()?,
        Some(base @ ("2" | "8" | "16")) => {
            let base = match base {
                "2" => 2,
                "8" => 8,
                "16" => 16,
                _ => unreachable!(),
            };
            mathcraft_compat::radix_integral::decode_unsigned_integral(text, base)
                .ok()?
                .exact_f64()
        }
        Some(_) => return None,
    };
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use crate::result_display::from_native_result;
    use mathcraft_document::canvas::MathFormat;
    const ML: &str = "http://schemas.mathsoft.com/math50";
    fn display(body: &str) -> Option<String> {
        from_native_result(
            &format!("<ml:result xmlns:ml='{ML}'>{body}</ml:result>"),
            &MathFormat::default(),
        )
        .map(|result| result.cells.iter().map(|cell| cell.plain()).collect())
    }
    #[test]
    fn cached_radix_coefficients_use_the_shared_exact_decoder_recursively() {
        for (digits, base, expected) in [("00101", 2, "5"), ("0017", 8, "15"), ("000aB", 16, "171")]
        {
            let real = format!("<ml:real base='{base}'>{digits}</ml:real>");
            assert_eq!(display(&real).as_deref(), Some(expected));
            let complex = format!("<ml:complex>{real}<ml:imag>2</ml:imag></ml:complex>");
            assert_eq!(display(&complex), Some(format!("{expected} + 2i")));
            let matrix =
                format!("<ml:matrix rows='1' cols='2'>{real}<ml:real>3</ml:real></ml:matrix>");
            assert_eq!(display(&matrix), Some(format!("{expected}3")));
        }
        assert_eq!(
            display("<ml:real base='10'> 1.25 </ml:real>").as_deref(),
            Some("1.25")
        );
    }
    #[test]
    fn unsupported_numeric_metadata_and_digits_never_display_as_decimal() {
        for literal in [
            "<ml:real base='2'>2</ml:real>",
            "<ml:real base='3'>10</ml:real>",
            "<ml:real base='02'>10</ml:real>",
            "<ml:real base='2'>1.1</ml:real>",
            "<ml:real base='16'>20000000000001</ml:real>",
            "<ml:real future='x'>10</ml:real>",
            "<ml:real xmlns:f='urn:fake' f:base='2'>10</ml:real>",
            "<ml:real>1<!--x-->0</ml:real>",
        ] {
            assert!(display(literal).is_none(), "{literal}");
            assert!(
                display(&format!(
                    "<ml:complex>{literal}<ml:imag>2</ml:imag></ml:complex>"
                ))
                .is_none(),
                "{literal}"
            );
            assert!(
                display(&format!(
                    "<ml:matrix rows='1' cols='1'>{literal}</ml:matrix>"
                ))
                .is_none(),
                "{literal}"
            );
        }
        for imag in [
            "<ml:imag base='2'>10</ml:imag>",
            "<ml:imag symbol='q'>2</ml:imag>",
            "<ml:imag future='x'>2</ml:imag>",
        ] {
            assert!(
                display(&format!(
                    "<ml:complex><ml:real>1</ml:real>{imag}</ml:complex>"
                ))
                .is_none(),
                "{imag}"
            );
        }
    }
}
