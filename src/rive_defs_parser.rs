/****************************************************************
 * $ID: rive_defs_parser.rs  	Sun 30 Nov 2025 10:20:37+0800   *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

#![allow(unused)]
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path, io::{Result, BufWriter, Write}};

/// JSON文件中的键结构，包含整数ID和字符串表示
#[derive(Debug, Deserialize)] struct Key { int: u32, string: String, }

/// 属性定义结构，从JSON文件中反序列化
#[derive(Debug, Deserialize)] struct PropertyDef {
    #[serde(rename = "type")] type_name: String,
    #[serde(rename = "typeRuntime", default)] type_runtime: Option<String>,
    #[serde(default)] runtime: Option<bool>,
    #[serde(default)] key: Option<Key>,
    #[serde(default)] description: Option<String>,
}

/// 对象定义结构，从JSON文件中反序列化
#[derive(Debug, Deserialize)] struct ObjectDef {
    name: String, key: Key,
    #[serde(default)] extends: Option<String>,
    #[serde(default)] properties: HashMap<String, PropertyDef>,
    #[serde(default)] runtime: Option<bool>,
}

/// 收集的属性信息，用于后续代码生成
#[derive(Debug, Clone)] struct PropertyInfo {
    name: String,
    type_name: String,
    type_runtime: Option<String>,
    property_id: Option<u32>,
    property_key: Option<String>,
    object_name: String, // 对象名称，用于生成唯一的属性常量名
}

/// 收集的对象信息，用于后续代码生成
#[derive(Debug)] struct ObjectInfo {
    name: String,
    type_id: u32,
    type_key: String,
    extends: Option<String>,
    properties: Vec<PropertyInfo>,
}

fn main() -> Result<()> {
    let defs_dir = Path::new("rive-rs/submodules/rive-cpp/dev/defs");
    if !defs_dir.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound,
            format!("目录不存在: {}", defs_dir.display())))
    }

    println!("开始解析 rive-cpp 定义文件...");
    println!("遍历目录: {}", defs_dir.display());

    let mut objects = Vec::new();
    let mut type_count = 0;
    let mut property_count = 0;
    let mut unique_types = HashMap::new();

    visit_defs_dir(defs_dir, &mut |fpath| {
        if fpath.extension().is_none_or(|ext| ext != "json") { return Ok(()) }
        let content = fs::read_to_string(fpath)?;
        let def = serde_json::from_str::<ObjectDef>(&content)?;
        type_count += 1;

        // 收集属性信息（包括非运行时属性）
        let properties = def.properties.iter()
            .map(|(prop_name, prop_def)| {
            property_count += 1;

            // 记录类型信息
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

        objects.push(ObjectInfo {
            name: def.name,
            type_id:  def.key.int,
            type_key: def.key.string,
            extends:  def.extends, properties,
        }); Ok(())
    })?;

    objects.sort_by_key(|obj| obj.type_id);

    println!("找到 {} 个对象类型，{} 个属性，{} 种唯一属性类型",
        type_count, property_count, unique_types.len());

    println!("\n所有唯一的属性类型：");
    for type_name in unique_types.keys() { println!("- {}", type_name); }

    generate_rs_file(&objects)?;
    println!("\n已成功生成 rive_defs.rs 文件");   Ok(())
}

// 生成.rs文件，包含类型ID和属性ID的映射
fn generate_rs_file(objects: &[ObjectInfo]) -> Result<()> {
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

    // 生成唯一的属性常量名称
    let mut generated_constants = HashMap::new();
    let mut id_to_const_name  = HashMap::new();

    for &(id, name, object_name, _, _) in &properties {
        let base_name = name.replace(":", "").replace("/", "_").to_uppercase();

        // 如果属性名称重复，添加对象前缀
        let const_name = if *name_counts.get(name).unwrap() > 1 {
            let obj_prefix = object_name.replace(":", "")
                .replace("/", "_").to_uppercase();
            format!("{}_{}", obj_prefix, base_name)
        } else { base_name };

        // 确保生成的常量名称唯一
        let mut counter = 1;
        let mut final_name = const_name.clone();
        while generated_constants.contains_key(&final_name) {
            final_name = format!("{}_{}", const_name, counter);
            counter += 1;
        }

        generated_constants.insert(final_name.clone(), id);
        if id_to_const_name.insert(id, final_name).is_some() {
            eprintln!("警告: 属性ID {} 对应多个常量名称", id);
        }
    }

    println!("总共发现 {} 个属性，已生成 {} 个属性常量映射",
        properties.len(), generated_constants.len());

    let file = fs::File::create("target/rive_defs.rs")?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "// 本文件包含 {} 个对象类型和 {} 个属性映射\n",
        objects.len(), properties.len())?;
    writeln!(writer, "/****************************************************************")?;
    writeln!(writer, " * $ID: rive_defs.rs                                            ")?;
    writeln!(writer, " *                                                              ")?;
    writeln!(writer, " * 自动生成的文件 - 包含Rive对象类型和属性ID的映射                 ")?;
    writeln!(writer, " ****************************************************************/\n")?;

    /* writeln!(writer, "// 对象类型ID常量\npub mod object_ids {{")?;
    for obj in objects {
        let const_name = obj.name.replace(":", "")
            .replace("/", "_").to_uppercase();
        writeln!(writer, "    pub const {}: u32 = {};", const_name, obj.type_id)?;
    }   writeln!(writer, "}}\n")?;

    writeln!(writer, "// 属性ID常量\npub mod property_ids {{")?;
    for (const_name, prop_id) in &generated_constants {
        writeln!(writer, "    pub const {}: u32 = {};", const_name, prop_id)?;
    }   writeln!(writer, "}}\n")?; */

    writeln!(writer, "// 创建包含所有已知属性类型的TOC映射
pub fn create_core_toc() -> HashMap<VarUInt, FieldType> {{")?;
    writeln!(writer, "    let mut toc = HashMap::with_capacity({});\n",
        properties.len())?;

    for &(id, _, _, type_name, type_runtime) in &properties {
        let const_name = id_to_const_name.get(&id).unwrap();

        // 确定字段类型，优先使用type_runtime
        let field_type = match type_runtime.unwrap_or(type_name)
            .to_lowercase().as_str() {
            "uint" | "int" | "bool" => "FieldType::UIntBool",
            "bytes" | "string" => "FieldType::String",
            "float" | "double" => "FieldType::Float",
            "color" => "FieldType::Color",
            _ => "FieldType::UIntBool", // 默认类型
        };

        writeln!(writer, "    toc.insert(VarUInt({}), {});", id, field_type)?;
    }   writeln!(writer, "\n    toc
}}\n")?;         writer.flush()
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
