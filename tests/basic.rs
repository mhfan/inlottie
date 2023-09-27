
use std::fs::File;
use inlottie::schema::{GradientFill, Stroke, Transform, Animation};

#[test] pub fn parse_simple() -> Result<(), Box<dyn std::error::Error>> {
    for path in glob::glob("fixtures/unit/simple/*.json")?.filter_map(Result::ok) {
        eprintln!("Parsing {}", path.display()); //let path = path?;
        let _: Animation = serde_path_to_error::deserialize(
            &mut serde_json::Deserializer::from_reader(File::open(path)?))?;
    }
    for path in glob::glob("fixtures/ui/**/*.json")?.filter_map(Result::ok) {
        eprintln!("Parsing {}", path.display()); //let path = path?;
        let _: Animation = serde_path_to_error::deserialize(
            &mut serde_json::Deserializer::from_reader(File::open(path)?))?;
    }   Ok(())
}

#[test] pub fn parse_segments() -> Result<(), Box<dyn std::error::Error>> {
    let path_base = "fixtures/unit/segments";

    let file_path = format!("{}/transform_complex.json", path_base);
    eprintln!("Parsing {file_path}");
    let _: Transform = serde_path_to_error::deserialize(
        &mut serde_json::Deserializer::from_reader(File::open(file_path)?))?;

    let file_path = format!("{}/gradient_fill.json", path_base);
    eprintln!("Parsing {file_path}");
    let _: GradientFill = serde_path_to_error::deserialize(
        &mut serde_json::Deserializer::from_reader(File::open(file_path)?))?;

    let file_path = format!("{}/stroke.json", path_base);
    eprintln!("Parsing {file_path}");
    let _: Stroke = serde_path_to_error::deserialize(
        &mut serde_json::Deserializer::from_reader(File::open(file_path)?))?;

    Ok(())
}
