use mathcraft_core::Environment;
use mathcraft_document::Worksheet;
use mathcraft_document::native::NativePackage;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("native-inspect") => {
            let path = args
                .get(1)
                .ok_or("Usage: mathcraft-cli native-inspect FILE.mcdx")?;
            let package = NativePackage::open(Path::new(path))?;
            let parts: Vec<_> = package.parts().iter().map(|p| serde_json::json!({
                "name":p.name,"bytes":p.bytes().len(),"compression":format!("{:?}",p.compression)
            })).collect();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "parts":parts,"regions":package.regions()?,"caches":package.caches()?,
                "note":"Original parts are preserved; cached results are not freshly recalculated."
            })).map_err(|e|e.to_string())?);
        }
        Some("native-copy") => {
            let source = args
                .get(1)
                .ok_or("Usage: mathcraft-cli native-copy INPUT.mcdx NEW.mcdx")?;
            let destination = args.get(2).ok_or("Missing NEW.mcdx")?;
            let original = NativePackage::open(Path::new(source))?;
            original.write_new(Path::new(destination))?;
            let copy = NativePackage::open(Path::new(destination))?;
            if original.parts().len() != copy.parts().len()
                || original.parts().iter().zip(copy.parts()).any(|(a, b)| {
                    a.name != b.name || a.compression != b.compression || a.bytes() != b.bytes()
                })
            {
                return Err("Written package failed content/compression verification".into());
            }
            println!(
                "Preserved {} parts; original part bytes and compression verified. Original application reopen remains unverified.",
                copy.parts().len()
            );
        }
        Some("native-run") => {
            let path = args
                .get(1)
                .ok_or("Usage: mathcraft-cli native-run FILE.mcdx")?;
            let package = NativePackage::open(Path::new(path))?;
            let presentation = package
                .original_part("mathcad/settings/presentation.xml")
                .ok_or("Missing presentation settings")?;
            let presentation = std::str::from_utf8(presentation).map_err(|e| e.to_string())?;
            let grid = mathcraft_compat::PlacementGrid::from_presentation(
                presentation.trim_start_matches('\u{feff}'),
            )?;
            let rows = mathcraft_compat::evaluate_worksheet_with_grid(
                &package.worksheet_xml()?,
                Some(grid),
            )?;
            let errors = rows.iter().filter(|r| r.error.is_some()).count();
            println!("{}",serde_json::to_string_pretty(&serde_json::json!({
                "rows":rows,"errors":errors,"note":"Fresh independent Rust execution of a limited MathML subset; saved caches are not used."
            })).map_err(|e|e.to_string())?);
            if errors > 0 {
                return Err(format!("{errors} calculation/unsupported errors"));
            }
        }
        Some("conformance") => {
            let path = args
                .get(1)
                .ok_or("Usage: mathcraft-cli conformance REFERENCE.json")?;
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() > 32 * 1024 * 1024 {
                return Err("Reference exceeds 32 MiB".into());
            }
            let corpus = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let report = mathcraft_compat::reference::check_reference(&corpus)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            if !report.all_match() {
                return Err("Reference differences remain; see JSON report".into());
            }
        }
        Some("eval") => {
            let mut env = Environment::default();
            for line in args[1..].join(" ").split(';') {
                if let Some(value) = env.line(line)? {
                    println!("{}", value.text);
                }
            }
        }
        Some("run") => {
            let path = args
                .get(1)
                .ok_or("Usage: mathcraft-cli run FILE [--json]")?;
            let doc = Worksheet::load(Path::new(path))?;
            let (env, results) = doc.calculate();
            let mut errors = 0;
            let mut matched = 0;
            let mut compared = 0;
            let rows:Vec<_>=doc.rows.iter().zip(results).map(|(row,result)| {
                let mut comparison=None;
                if let (Some(expected),Ok(Some(actual)))=(row.imported.as_ref().and_then(|i|i.cached_value),&result)
                    && let Some(quantity)=actual.value {
                        let displayed=if let Some((_,label))=row.input.split_once("->") {
                            env.evaluate(label).ok().filter(|unit|quantity.same_dimension(unit)).map(|unit|quantity.value/unit.value)
                        }else{Some(quantity.value)};
                        if let Some(displayed)=displayed {
                            compared+=1;
                            let ok=(displayed-expected).norm() <= 1e-12_f64.max(expected.norm()*1e-8);
                            if ok {matched+=1;}
                            comparison=Some(ok);
                        }
                    }
                let (text,error)=match result {Ok(Some(v))=>(Some(v.text),None),Ok(None)=>(None,None),Err(e)=>{errors+=1;(None,Some(e))}};
                serde_json::json!({"input":row.input,"note":row.note,"result":text,"error":error,"cached_match":comparison})
            }).collect();
            if args.iter().any(|a| a == "--json") {
                println!("{}",serde_json::to_string_pretty(&serde_json::json!({"title":doc.title,"rows":rows,"errors":errors,"cached_compared":compared,"cached_matched":matched,"comparison_note":"Comparison against saved display values, not a fresh Mathcad recalculation."})).map_err(|e|e.to_string())?);
            } else {
                println!("{} — {} rows", doc.title, doc.rows.len());
                for row in rows {
                    println!(
                        "{}\n  {}",
                        row["input"].as_str().unwrap_or(""),
                        row["error"]
                            .as_str()
                            .or_else(|| row["result"].as_str())
                            .unwrap_or("")
                    );
                }
                println!("Errors: {errors}; saved cached values matched: {matched}/{compared}");
            }
        }
        Some("import") => {
            let source = args
                .get(1)
                .ok_or("Usage: mathcraft-cli import FILE.mcdx OUTPUT.mathcraft")?;
            let dest = args.get(2).ok_or("Missing output filename")?;
            let doc = Worksheet::load(Path::new(source))?;
            doc.save(Path::new(dest))?;
            println!("Imported {} rows", doc.rows.len());
        }
        Some("demo") => Worksheet::demo().save(Path::new(
            args.get(1)
                .ok_or("Usage: mathcraft-cli demo OUTPUT.mathcraft")?,
        ))?,
        Some("--version") => println!("Mathcraft {}", env!("CARGO_PKG_VERSION")),
        _ => println!(
            "Mathcraft {}\n\n  native-inspect FILE.mcdx\n  native-copy INPUT.mcdx NEW.mcdx\n  native-run FILE.mcdx\n  conformance REFERENCE.json\n  eval \"x := 3 m; x -> cm\"\n  run FILE.mathcraft [--json]\n  run FILE.mcdx [--json]\n  import FILE.mcdx OUTPUT.mathcraft",
            env!("CARGO_PKG_VERSION")
        ),
    }
    Ok(())
}
