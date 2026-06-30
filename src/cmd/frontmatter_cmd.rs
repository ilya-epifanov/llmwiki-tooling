use std::path::Path;

use crate::error::WikiError;
use crate::wiki::Wiki;

/// Run `frontmatter get <file> [field]`: extract frontmatter as JSON.
pub fn get(wiki: &Wiki, file: &Path, field: Option<&str>) -> Result<(), WikiError> {
    let file_path = wiki.abs_path(file);

    let fm_result = wiki.frontmatter(&file_path)?;

    let fm = match fm_result {
        Ok(Some(fm)) => fm,
        Ok(None) => {
            eprintln!("no frontmatter in {}", file_path.display());
            return Ok(());
        }
        Err(e) => {
            eprintln!("frontmatter error in {}: {e}", file_path.display());
            return Ok(());
        }
    };

    match field {
        Some(name) => {
            if let Some(val) = fm.get(name) {
                let json: serde_json::Value =
                    serde_json::to_value(val).unwrap_or(serde_json::Value::Null);
                println!("{json}");
            } else {
                eprintln!("field '{name}' not found");
            }
        }
        None => {
            let json =
                serde_json::to_value(fm.data()).expect("valid YAML frontmatter serializes to JSON");
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
    }

    Ok(())
}

/// Run `frontmatter set <file> <field> <value>`: modify a frontmatter field.
pub fn set(wiki: &mut Wiki, file: &Path, field: &str, value: &str) -> Result<(), WikiError> {
    let file_path = wiki.abs_path(file);

    // Read phase
    let cached = wiki.file(&file_path)?;
    let source = cached.source();

    let fm = match cached.frontmatter() {
        Ok(Some(fm)) => fm,
        Ok(None) => {
            eprintln!("no frontmatter in {}", file_path.display());
            return Ok(());
        }
        Err(e) => {
            eprintln!("frontmatter error in {}: {e}", file_path.display());
            return Ok(());
        }
    };

    let mut yaml_value: serde_yml::Value =
        serde_yml::from_str(&fm.raw_yaml).map_err(|e| WikiError::Frontmatter {
            path: file_path.clone(),
            source: crate::error::FrontmatterError::Yaml {
                source: Box::new(e),
                context: field.to_owned(),
            },
        })?;

    let parsed_value: serde_yml::Value =
        serde_yml::from_str(value).unwrap_or(serde_yml::Value::String(value.to_owned()));

    if let serde_yml::Value::Mapping(ref mut map) = yaml_value {
        map.insert(field, parsed_value);
    }

    let new_yaml = serde_yml::to_string(&yaml_value).map_err(|e| WikiError::Frontmatter {
        path: file_path.clone(),
        source: crate::error::FrontmatterError::Yaml {
            source: Box::new(e),
            context: field.to_owned(),
        },
    })?;

    let body = &source[fm.byte_range.end..];
    let new_source = format!("---\n{new_yaml}---\n{body}");

    // Write phase
    wiki.write_file(&file_path, &new_source)?;

    println!("updated '{}' in {}", field, file_path.display());
    Ok(())
}
