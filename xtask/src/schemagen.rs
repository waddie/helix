use crate::DynError;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

/// Generate JSON schemas for Helix configuration files
pub fn generate_schemas() -> Result<(), DynError> {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("contrib")
        .join("schemas");

    fs::create_dir_all(&schema_dir)?;

    // Generate config.toml schema
    println!("Generating schema for config.toml...");
    let config_schema = generate_config_schema();
    let config_schema_json = serde_json::to_string_pretty(&config_schema)?;
    let config_schema_path = schema_dir.join("config.json");
    fs::write(&config_schema_path, config_schema_json)?;
    println!("  ✓ Written to {}", config_schema_path.display());

    // Generate languages.toml schema (default - all fields as specified)
    println!("Generating schema for languages.toml (default)...");
    let lang_schema = generate_languages_schema();
    let lang_schema_json = serde_json::to_string_pretty(&lang_schema)?;
    let lang_schema_path = schema_dir.join("languages-default.json");
    fs::write(&lang_schema_path, lang_schema_json)?;
    println!("  ✓ Written to {}", lang_schema_path.display());

    // Generate languages.toml schema (user - most fields optional except name)
    println!("Generating schema for languages.toml (user overrides)...");
    let lang_user_schema = generate_languages_user_schema();
    let lang_user_schema_json = serde_json::to_string_pretty(&lang_user_schema)?;
    let lang_user_schema_path = schema_dir.join("languages-user.json");
    fs::write(&lang_user_schema_path, lang_user_schema_json)?;
    println!("  ✓ Written to {}", lang_user_schema_path.display());

    println!("\nSchema generation complete!");
    println!("\n  Schemas location: {}", schema_dir.display());

    Ok(())
}

fn generate_config_schema() -> serde_json::Value {
    use schemars::schema_for;

    // Generate schema from the actual Rust type (ConfigRaw has the top-level structure)
    let schema = schema_for!(helix_term::config::ConfigRaw);

    // Convert to JSON value and add some metadata
    let mut schema_json = serde_json::to_value(schema).unwrap();
    if let Some(obj) = schema_json.as_object_mut() {
        obj.insert("title".to_string(), json!("Helix Editor Configuration"));
        obj.insert(
            "description".to_string(),
            json!("Configuration file for the Helix text editor (config.toml)"),
        );
    }

    // Ensure all objects have additionalProperties: false for strict validation
    ensure_no_additional_properties(&mut schema_json);

    schema_json
}

fn generate_languages_schema() -> serde_json::Value {
    use schemars::schema_for;

    // Generate schema from the actual Rust type
    let schema = schema_for!(helix_core::syntax::config::Configuration);

    // Convert to JSON value and add some metadata
    let mut schema_json = serde_json::to_value(schema).unwrap();
    if let Some(obj) = schema_json.as_object_mut() {
        obj.insert(
            "title".to_string(),
            json!("Helix Languages Configuration (Default)"),
        );
        obj.insert(
            "description".to_string(),
            json!("Complete language server and grammar configuration for Helix (languages.toml)"),
        );
    }

    // Add serde aliases as actual properties (JSON Schema doesn't have aliases)
    add_serde_aliases(&mut schema_json);

    // Ensure all objects have additionalProperties: false for strict validation
    ensure_no_additional_properties(&mut schema_json);

    schema_json
}

fn generate_languages_user_schema() -> serde_json::Value {
    use schemars::schema_for;

    // Generate schema from the actual Rust type
    let schema = schema_for!(helix_core::syntax::config::Configuration);

    // Convert to JSON value
    let mut schema_json = serde_json::to_value(schema).unwrap();

    // Modify the schema to make all fields optional except "name" in LanguageConfiguration
    if let Some(definitions) = schema_json
        .get_mut("definitions")
        .and_then(|d| d.as_object_mut())
    {
        // Find LanguageConfiguration definition
        if let Some(lang_config) = definitions
            .get_mut("LanguageConfiguration")
            .and_then(|lc| lc.as_object_mut())
        {
            // Make all fields except "name" optional by modifying the required array
            if let Some(required) = lang_config
                .get_mut("required")
                .and_then(|r| r.as_array_mut())
            {
                // Keep only "name" as required
                required.retain(|field| field.as_str() == Some("name"));
            }
        }

        // Also make language-server fields more flexible
        if let Some(ls_config) = definitions
            .get_mut("LanguageServerConfiguration")
            .and_then(|lsc| lsc.as_object_mut())
        {
            // Make all fields optional for language-server overrides
            if let Some(required) = ls_config.get_mut("required") {
                *required = json!([]);
            }
        }
    }

    // Add metadata
    if let Some(obj) = schema_json.as_object_mut() {
        obj.insert(
            "title".to_string(),
            json!("Helix Languages Configuration (User Overrides)"),
        );
        obj.insert("description".to_string(), json!("User overrides for language server and grammar configuration in Helix (languages.toml). Only the 'name' field is required; all other fields are optional and will override defaults."));
    }

    // Add serde aliases as actual properties (JSON Schema doesn't have aliases)
    add_serde_aliases(&mut schema_json);

    // Ensure all objects have additionalProperties: false for strict validation
    ensure_no_additional_properties(&mut schema_json);

    schema_json
}

/// Add serde field aliases as actual properties in the schema
/// This is needed because JSON Schema doesn't have a concept of aliases
fn add_serde_aliases(schema: &mut serde_json::Value) {
    if let Some(definitions) = schema
        .get_mut("definitions")
        .and_then(|d| d.as_object_mut())
    {
        // Add comment-token as an alias for comment-tokens in LanguageConfiguration
        if let Some(lang_config) = definitions
            .get_mut("LanguageConfiguration")
            .and_then(|lc| lc.as_object_mut())
        {
            if let Some(properties) = lang_config
                .get_mut("properties")
                .and_then(|p| p.as_object_mut())
            {
                // If comment-tokens exists, also add comment-token (the alias)
                if let Some(comment_tokens) = properties.get("comment-tokens").cloned() {
                    properties.insert("comment-token".to_string(), comment_tokens);
                }
            }
        }
    }
}

/// Recursively ensure all objects in the schema have additionalProperties: false
fn ensure_no_additional_properties(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // If this object has a "type": "object" or has "properties", ensure it has additionalProperties: false
            let has_type_object = map.get("type").and_then(|v| v.as_str()) == Some("object");
            let has_properties = map.contains_key("properties");

            if (has_type_object || has_properties) && !map.contains_key("additionalProperties") {
                // Only set additionalProperties: false if not already set
                // This allows manually set values (like true for keys) to remain
                map.insert("additionalProperties".to_string(), json!(false));
            }

            // Recursively process all values in this object
            for (_key, val) in map.iter_mut() {
                ensure_no_additional_properties(val);
            }
        }
        serde_json::Value::Array(arr) => {
            // Recursively process all items in arrays
            for item in arr.iter_mut() {
                ensure_no_additional_properties(item);
            }
        }
        _ => {}
    }
}
