
use std::{fs, io::Error};
use inlottie::schema::{GradientFill, Stroke, Transform, Animation};

#[test] pub fn test_transform_complex() -> Result<(), Error> {
    let file = fs::File::open("fixtures/unit/segments/transform_complex.json")?;
    let d = &mut serde_json::Deserializer::from_reader(file);
    let _: Transform = serde_path_to_error::deserialize(d).unwrap(); Ok(())
}

#[test] pub fn test_stroke() -> Result<(), Error> {
    let file = fs::File::open("fixtures/unit/segments/stroke.json")?;
    let d = &mut serde_json::Deserializer::from_reader(file);
    let _: Stroke = serde_path_to_error::deserialize(d).unwrap(); Ok(())
}

#[test] pub fn test_gradient_fill() -> Result<(), Error> {
    let file = fs::File::open("fixtures/unit/segments/gradient_fill.json")?;
    let d = &mut serde_json::Deserializer::from_reader(file);
    let _: GradientFill = serde_path_to_error::deserialize(d).unwrap(); Ok(())
}

#[test] fn test_bouncy_ball_example() -> Result<(), Error> {
    let file = fs::File::open("fixtures/ui/lottie-ios-samples/Nonanimating/FirstText.json")?;
    let d = &mut serde_json::Deserializer::from_reader(file);
    let _: Animation = serde_path_to_error::deserialize(d).unwrap(); Ok(())
}
