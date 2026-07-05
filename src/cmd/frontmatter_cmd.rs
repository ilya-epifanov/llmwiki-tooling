use std::path::Path;

use crate::edit_plan::{EditPlan, EditPlanMode};
use crate::error::{FrontmatterError, WikiError};
use crate::wiki::Wiki;

fn frontmatter_error(path: &Path, source: FrontmatterError) -> WikiError {
    WikiError::Frontmatter {
        path: path.to_path_buf(),
        source,
    }
}

/// Run `frontmatter get <file> [field]`: extract frontmatter as JSON.
pub fn get(wiki: &Wiki, file: &Path, field: Option<&str>) -> Result<(), WikiError> {
    let file_path = wiki.abs_path(file);
    let fm = wiki
        .file(&file_path)?
        .frontmatter()
        .map_err(|e| frontmatter_error(&file_path, e))?
        .ok_or_else(|| frontmatter_error(&file_path, FrontmatterError::NoFrontmatter))?;

    match field {
        Some(name) => {
            let val = fm.get(name).ok_or_else(|| {
                frontmatter_error(
                    &file_path,
                    FrontmatterError::MissingField {
                        field: name.to_owned(),
                    },
                )
            })?;
            let json: serde_json::Value =
                serde_json::to_value(val).unwrap_or(serde_json::Value::Null);
            println!("{json}");
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
    let (source, edit) = {
        let cached = wiki.file(&file_path)?;
        let source = cached.source().to_owned();
        let edit = cached
            .set_frontmatter_field_edit(field, value)
            .map_err(|e| frontmatter_error(&file_path, e))?;
        (source, edit)
    };

    let mut plan = EditPlan::new();
    plan.add_edits(file_path.clone(), &source, vec![edit])?;
    plan.execute(wiki, EditPlanMode::Apply)?;
    Ok(())
}
