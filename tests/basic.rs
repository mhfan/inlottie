
use std::fs::File;
use std::error::Error as StdErr;
use inlottie::schema::Animation;
use serde_json::Deserializer as json_des;
use serde_path_to_error::deserialize as deserial_err;

// XXX: get resources from https://github.com/zimond/lottie-rs/tree/main/fixtures

#[test] pub fn parse_ui_samples() -> Result<(), Box<dyn StdErr>> {  let mut cnt = 0u32;
    for path in glob::glob("fixtures/ui/**/*.json")?.filter_map(Result::ok) {
                //.chain(glob::glob("fixtures/unit/simple/*.json")?)
        let _: Animation = deserial_err(&mut json_des::from_reader(
            File::open(&path)?)).map_err(|err| {
                eprintln!("Failed parsing {}", path.display()); err })?;    cnt += 1;
    }   println!("Succeed to parse {cnt} lottie json files!");  Ok(())
}

#[test] pub fn parse_segments() -> Result<(), Box<dyn StdErr>> {
    fn segparse<'de, T: serde::de::Deserialize<'de>>(sfn: &str) -> Result<T, Box<dyn StdErr>> {
        let path = format!("fixtures/segments/{}.json", sfn);
        Ok(deserial_err(&mut json_des::from_reader(File::open(&path)?))
            .map_err(|err| { eprintln!("Failed parsing {path}"); err })?)
    }   use inlottie::schema::{GradientFill, Stroke, TextRange, Transform};

    segparse::<Animation>("animated_position_legacy")?;
    segparse::<Transform>("transform_complex")?;
    segparse::<GradientFill>("gradient_fill")?;
    segparse::<TextRange>("text_range")?;
    segparse::<Stroke>("stroke")?;

    Ok(())
}
