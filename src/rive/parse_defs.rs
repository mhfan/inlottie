/****************************************************************
 * $ID: parse_defs.rs  	        Sun 30 Nov 2025 10:20:37+0800   *
 *                                                              *
 * Maintainer: MeiHui FAN <mhfan@ustc.edu>                       *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

#![allow(unused)]
use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::{Path, PathBuf},
    io::{Error, ErrorKind::InvalidInput, Result, BufWriter, Write}};

const DEFAULT_DEFS_DIR: &str = "rive-rs/submodules/rive-cpp/dev/defs";
const DEFAULT_OUTPUT: &str = "target/rive_defs.rs";

/// Key represented by both its integer ID and string name in a JSON definition.
#[derive(Debug, Deserialize)] struct Key { int: u32, string: String, }

/// Property definition deserialized from JSON.
#[derive(Debug, Deserialize)] struct PropertyDef {
    #[serde(rename = "type")] type_name: String,
    #[serde(rename = "typeRuntime", default)] type_runtime: Option<String>,
    #[serde(default)] runtime: Option<bool>,
    #[serde(default)] key: Option<Key>,
    #[serde(default)] description: Option<String>,
}

/// Object definition deserialized from JSON.
#[derive(Debug, Deserialize)] struct ObjectDef {
    name: String, key: Key,
    #[serde(default)] extends: Option<String>,
    #[serde(default)] properties: HashMap<String, PropertyDef>,
    #[serde(default)] runtime: Option<bool>,
}

/// Property metadata collected for code generation.
#[derive(Debug, Clone)] struct PropertyInfo {
    name: String,
    type_name: String,
    type_runtime: Option<String>,
    property_id: Option<u32>,
    property_key: Option<String>,
    object_name: String, // Used to produce a unique property constant name.
}

/// Object metadata collected for code generation.
#[derive(Debug)] struct ObjectInfo {
    name: String,
    type_id: u32,
    type_key: String,
    extends: Option<String>,
    properties: Vec<PropertyInfo>,
}

fn main() -> Result<()> {   // cargo r --bin parse_rive_defs
    let mut args = env::args_os().skip(1);
    let defs_dir = args.next().map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DEFS_DIR));
    let output = args.next().map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    if args.next().is_some() {
        return Err(Error::new(InvalidInput,
            "usage: parse_rive_defs [defs-directory] [output-file]"))
    }

    generate(&defs_dir, &output)
}

pub fn generate(defs_dir: &Path, output: &Path) -> Result<()> {
    if !defs_dir.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound,
            format!("definitions directory does not exist: {}", defs_dir.display())))
    }   println!("Scanning definitions directory: {}", defs_dir.display());

    let mut objects = Vec::new();
    let mut unique_types = HashMap::new();
    let (mut type_count, mut property_count) = (0, 0);

    visit_defs_dir(defs_dir, &mut |fpath| {
        if fpath.extension().is_none_or(|ext| ext != "json") { return Ok(()) }
        let content = fs::read_to_string(fpath)?;
        let def = serde_json::from_str::<ObjectDef>(&content)?;
        type_count += 1;

        // Include properties that are not exposed by the runtime.
        let mut properties: Vec<_> = def.properties.iter()
            .map(|(prop_name, prop_def)| {
            property_count += 1;

            // Track every declared and runtime backing type.
            unique_types.insert(prop_def.type_name.clone(), ());
            if let Some(type_runtime) = &prop_def.type_runtime {
                unique_types.insert(type_runtime.clone(), ());
            }

            PropertyInfo {
                name: prop_name.clone(),
                type_name: prop_def.type_name.clone(),
                type_runtime: prop_def.type_runtime.clone(),
                property_id:  prop_def.key.as_ref().map(|k| k.int),
                property_key: prop_def.key.as_ref().map(|k| k.string.clone()),
                object_name: def.name.clone(),
            }
        }).collect();
        properties.sort_by(|left, right|
            left.property_id.cmp(&right.property_id).then_with(|| left.name.cmp(&right.name)));

        objects.push(ObjectInfo {
            name: def.name,
            type_id:  def.key.int,
            type_key: def.key.string,
            extends:  def.extends, properties,
        }); Ok(())
    })?;

    objects.sort_by_key(|obj| obj.type_id);

    println!("Found {} object types, {} properties, and {} unique property types:\n",
        type_count, property_count, unique_types.len());
    for type_name in unique_types.keys() { println!("- {}", type_name); }

    generate_rs_file(&objects, output)?;    Ok(())
}

fn screaming_snake(name: &str) -> String {
    let mut result = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && index != 0
                && !result.ends_with('_') { result.push('_') }
            result.push(ch.to_ascii_uppercase());
        } else if !result.ends_with('_') { result.push('_') }
    }
    result.trim_matches('_').to_owned()
}

fn snake_case(name: &str) -> String {
    let ident = screaming_snake(name).to_ascii_lowercase();
    match ident.as_str() {
        "box" | "const" | "crate" | "enum" | "extern" | "fn" | "impl" | "in" |
        "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref" |
        "self" | "self_type" | "static" | "struct" | "super" | "trait" | "type" |
        "unsafe" | "use" | "where" | "while" | "async" | "await" | "dyn" =>
            format!("{ident}_"),
        _ => ident,
    }
}

fn pascal_case(name: &str) -> String {
    screaming_snake(name).split('_').filter(|part| !part.is_empty()).map(|part| {
        let mut chars = part.chars();
        chars.next().map(|first|
            first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase())
            .unwrap_or_default()
    }).collect()
}

fn accessor(type_name: &str, type_runtime: Option<&str>) -> (&'static str, &'static str) {
    let semantic = type_name.to_ascii_lowercase();
    let runtime = type_runtime.unwrap_or(type_name).to_ascii_lowercase();
    match runtime.as_str() {
        "bytes" | "string" => ("bytes", "&[u8]"),
        "float" | "double" => ("float", "f32"),
        "color" => ("color", "u32"),
        _ if semantic == "bool" => ("boolean", "bool"),
        _ => ("varuint", "u32"),
    }
}

fn collect_properties<'a>(object: &'a ObjectInfo,
    objects: &HashMap<&str, &'a ObjectInfo>, properties: &mut Vec<&'a PropertyInfo>) {
    if let Some(base) = object.extends.as_deref()
        .and_then(|path| Path::new(path).file_stem())
        .and_then(|key| key.to_str())
        .and_then(|key| objects.get(key)) {
        collect_properties(base, objects, properties);
    }
    properties.extend(object.properties.iter());
}

// Generate Rust object IDs, property IDs, typed accessors, and backing-type lookup.
fn generate_rs_file(objects: &[ObjectInfo], output: &Path) -> Result<()> {
    let objects_by_key: HashMap<_, _> =
        objects.iter().map(|object| (object.type_key.as_str(), object)).collect();
    let mut  name_counts = HashMap::new();
    for obj in objects {
        for prop in &obj.properties {
            *name_counts.entry(prop.name.as_str()).or_insert(0) += 1;
        }
    }

    let mut properties: Vec<_> = objects.iter() .flat_map(|obj| {
        obj.properties.iter().filter_map(|prop| {
            prop.property_id.map(|id| (id,
                prop.name.as_str(),
                 obj.name.as_str(),
                prop.type_name.as_str(),
                prop.type_runtime.as_deref()
            ))
        })
    }).collect();

    properties.sort_by_key(|(id, _, _, _, _)| *id);

    // Generate unique property constant names.
    let mut generated_constants = HashMap::new();
    let mut id_to_const_name  = HashMap::new();

    for &(id, name, object_name, _, _) in &properties {
        let base_name = name.replace(":", "").replace("/", "_").to_uppercase();

        // Prefix duplicate property names with their object name.
        let const_name = if *name_counts.get(name).unwrap() > 1 {
            let obj_prefix = object_name.replace(":", "")
                .replace("/", "_").to_uppercase();
            format!("{}_{}", obj_prefix, base_name)
        } else { base_name };

        // Add a numeric suffix if the object prefix is still not unique.
        let (mut final_name, mut counter) = (const_name.clone(), 1);
        while generated_constants.contains_key(&final_name) {
            final_name = format!("{}_{}", const_name, counter);
            counter += 1;
        }

        generated_constants.insert(final_name.clone(), id);
        if id_to_const_name.insert(id, final_name).is_some() {
            eprintln!("Warning: property ID {id} maps to multiple constant names");
        }
    }

    println!("\nFound {} properties and generated {} property constants",
        properties.len(), generated_constants.len());

    let mut writer = BufWriter::new(fs::File::create(output)?);

    writeln!(writer, "// @generated by src/rive/parse_defs.rs; DO NOT EDIT.\n\
        // Contains {} object types and {} property mappings (Rive).\n",
        objects.len(), properties.len())?;

    writeln!(writer, "pub mod object_ids {{")?;
    for obj in objects {
        let const_name = screaming_snake(&obj.name);
        writeln!(writer, "    pub const {}: u32 = {};", const_name, obj.type_id)?;
    }   writeln!(writer, "}}\n")?;

    writeln!(writer, "pub mod property_ids {{")?;
    for &(prop_id, _, _, _, _) in &properties {
        let const_name = id_to_const_name.get(&prop_id).unwrap();
        writeln!(writer, "    pub const {}: u32 = {};", const_name, prop_id)?;
    }   writeln!(writer, "}}\n")?;

    writeln!(writer, "pub mod objects {{
    use super::{{DecodeError, Object, Result, object_ids, property_ids}};\n")?;
    for obj in objects {
        let type_name = pascal_case(&obj.name);
        let object_id = screaming_snake(&obj.name);
        writeln!(writer, "    #[derive(Debug, Clone, Copy)]
    pub struct {type_name}<'a>(&'a Object);

    impl<'a> TryFrom<&'a Object> for {type_name}<'a> {{
        type Error = DecodeError;
        fn try_from(object: &'a Object) -> Result<Self> {{
            if object.type_id.0 == object_ids::{object_id} {{ Ok(Self(object)) }}
            else {{ Err(DecodeError::ObjectTypeMismatch {{
                expected: object_ids::{object_id}, actual: object.type_id.0 }}) }}
        }}
    }}

    impl {type_name}<'_> {{
        pub fn object(&self) -> &Object {{ self.0 }}")?;
        let mut inherited = Vec::new();
        collect_properties(obj, &objects_by_key, &mut inherited);
        let mut accessors = std::collections::BTreeMap::new();
        for prop in inherited { accessors.insert(snake_case(&prop.name), prop); }
        for (method, prop) in accessors {
            let Some(prop_id) = prop.property_id else { continue };
            let prop_const = id_to_const_name.get(&prop_id).unwrap();
            let (accessor, return_type) =
                accessor(&prop.type_name, prop.type_runtime.as_deref());
            writeln!(writer,
                "        pub fn {method}(&self) -> Result<Option<{return_type}>> {{ \
                 self.0.{accessor}(property_ids::{prop_const}) }}")?;
        }
        writeln!(writer, "    }}\n")?;
    }
    writeln!(writer, "}}\n")?;

    writeln!(writer, "#[derive(Debug, Clone, Copy)]
pub enum TypedObject<'a> {{")?;
    for obj in objects {
        let type_name = pascal_case(&obj.name);
        writeln!(writer, "    {type_name}(objects::{type_name}<'a>),")?;
    }
    writeln!(writer, "}}

impl<'a> TryFrom<&'a Object> for TypedObject<'a> {{
    type Error = DecodeError;
    fn try_from(object: &'a Object) -> Result<Self> {{
        Ok(match object.type_id.0 {{")?;
    for obj in objects {
        let type_name = pascal_case(&obj.name);
        let object_id = screaming_snake(&obj.name);
        writeln!(writer, "            object_ids::{object_id} =>\
            Self::{type_name}(objects::{type_name}::try_from(object)?),")?;
    }
    writeln!(writer, "            type_id => return Err(DecodeError::UnknownObjectType(type_id)),
        }})
    }}
}}\n")?;

    writeln!(writer, "// Return the serialization type of a known core property.
pub fn core_prop_type(id: VarUInt) -> Option<FieldType> {{
    Some(match id.0 {{")?;

    for &(id, _, _, type_name, type_runtime) in &properties {
        // Prefer the runtime type when selecting the serialized backing type.
        let field_type = match type_runtime.unwrap_or(type_name)
            .to_lowercase().as_str() {
            "uint" | "int" | "bool" => "FieldType::UIntBool",
            "bytes" | "string" => "FieldType::String",
            "float" | "double" => "FieldType::Float",
            "color" => "FieldType::Color",
            _ => "FieldType::UIntBool", // Default backing type.
        };

        writeln!(writer, "        {} => {},", id, field_type)?;
    }   writeln!(writer, "        _ => return None,
    }})
}}")?;

    println!("Generated: {}", output.display());
    writer.flush()
}

fn visit_defs_dir<F>(dir: &Path, callback: &mut F) -> Result<()>
    where F: FnMut(&Path) -> Result<()> {
    if !dir.is_dir() { return callback(dir) }
    fs::read_dir(dir)?.try_for_each(|entry| {
        let path = entry?.path();
        if  path.is_dir() { visit_defs_dir(&path, callback)
        } else { callback(&path) }
    })
}
