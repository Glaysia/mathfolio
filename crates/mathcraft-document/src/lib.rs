use mathcraft_core::{Environment, LineValue, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

pub mod canvas;
pub mod export;
mod mcdx;
pub mod native;
pub use mcdx::import_mcdx;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub input: String,
    #[serde(default)]
    pub note: bool,
    #[serde(default)]
    pub imported: Option<Imported>,
}
impl Row {
    pub fn math(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            note: false,
            imported: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Imported {
    pub region_id: String,
    pub left: f64,
    pub top: f64,
    pub unsupported: Option<String>,
    pub cached_display: Option<String>,
    pub cached_value: Option<num_complex::Complex64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Worksheet {
    pub version: u32,
    pub title: String,
    pub rows: Vec<Row>,
}
impl Default for Worksheet {
    fn default() -> Self {
        Self::demo()
    }
}
impl Worksheet {
    pub fn demo() -> Self {
        Self {version:1,title:"Resonant circuit · engineering worksheet".into(),rows:vec![
            Row {input:"An independent Rust calculation prototype. Edit any expression to recalculate below.".into(),note:true,imported:None},
            Row::math("f := 360 kHz -> kHz"),Row::math("omega := 2*pi*f -> rad/s"),
            Row::math("L := 15 uH -> uH"),Row::math("C := 1/(omega^2*L) -> nF"),
            Row::math("R := 50 ohm -> ohm"),Row::math("Z := R + i*(omega*L - 1/(omega*C)) -> ohm"),
            Row::math("abs(Z) -> ohm"),Row::math("period := 1/f -> us"),
            Row {input:"Functions, complex values and dimensional checks".into(),note:true,imported:None},
            Row::math("response(x) := sin(x)*exp(-0.15*x)"),Row::math("response(pi/2)"),
            Row::math("distance := 3 m + 20 cm -> cm"),
        ]}
    }
    pub fn empty() -> Self {
        Self {
            version: 1,
            title: "Untitled worksheet".into(),
            rows: vec![Row::math("")],
        }
    }
    pub fn calculate(&self) -> (Environment, Vec<Result<Option<LineValue>>>) {
        let mut env = Environment::default();
        let results = self
            .rows
            .iter()
            .map(|row| {
                if row.note {
                    return Ok(None);
                }
                if let Some(reason) = row.imported.as_ref().and_then(|i| i.unsupported.as_ref()) {
                    return Err(format!("Unsupported import: {reason}"));
                }
                env.line(&row.input)
            })
            .collect();
        (env, results)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if path.extension().and_then(|s| s.to_str()) != Some("mathcraft") {
            return Err(
                "Use the .mathcraft extension; original Mathcad documents are never overwritten"
                    .into(),
            );
        }
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        // Write a sibling temporary file first. Preserve the old file on serialization failure.
        let temp = path.with_extension("mathcraft.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    }
    pub fn load(path: &Path) -> Result<Self> {
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("mcdx"))
        {
            return import_mcdx(path);
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
            return Err("Document exceeds 8 MiB".into());
        }
        let doc: Self = serde_json::from_reader(file).map_err(|e| e.to_string())?;
        doc.validate()?;
        Ok(doc)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(format!("Unsupported document version {}", self.version));
        }
        if self.rows.len() > 1000 {
            return Err("At most 1000 rows are supported".into());
        }
        if self.title.len() > 1024 || self.rows.iter().any(|r| r.input.len() > 4096) {
            return Err("Document text exceeds prototype limits".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_calculates_and_serialization_preserves_results() {
        let doc = Worksheet::demo();
        assert!(doc.calculate().1.iter().all(|r| r.is_ok()));
        let json = serde_json::to_string(&doc).unwrap();
        let restored: Worksheet = serde_json::from_str(&json).unwrap();
        let results = restored.calculate().1;
        assert!(results.iter().all(|r| r.is_ok()));
        assert_eq!(
            results[12].as_ref().unwrap().as_ref().unwrap().text,
            "320 cm"
        );
    }
    #[test]
    fn future_version_is_rejected() {
        let mut d = Worksheet::empty();
        d.version = 2;
        assert!(d.validate().is_err());
    }
    #[test]
    fn save_open_and_replace_preserve_document() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mathcraft-test-{}-{nonce}.mathcraft",
            std::process::id()
        ));
        let mut doc = Worksheet::demo();
        doc.save(&path).unwrap();
        assert_eq!(Worksheet::load(&path).unwrap().rows.len(), doc.rows.len());
        doc.title = "Updated".into();
        doc.save(&path).unwrap();
        assert_eq!(Worksheet::load(&path).unwrap().title, "Updated");
        std::fs::remove_file(path).unwrap();
    }
}
