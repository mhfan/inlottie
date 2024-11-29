
use std::fs::File;
use std::error::Error as StdErr;
use inlottie::schema::Animation;
use serde_json::Deserializer as json_des;
use serde_path_to_error::deserialize as deserial_err;

// XXX: get resources from https://github.com/zimond/lottie-rs/tree/main/fixtures

#[test] pub fn parse_ui_samples() -> Result<(), Box<dyn StdErr>> {  let mut cnt = 0u32;
    for path in glob::glob("lottie-rs/fixtures/ui/**/*.json")?.filter_map(Result::ok) {
                //.chain(glob::glob("fixtures/unit/simple/*.json")?)
        if path.ends_with("issue_1460.json") { continue } // ignore malformed file
        println!("Parsing {} ...", path.display());     cnt += 1;
        let _: Animation = deserial_err(&mut json_des::from_reader(File::open(&path)?))?;
    }   println!("Succeed to parse {cnt} lottie json files!");  Ok(())
}

#[test] pub fn parse_segments() -> Result<(), Box<dyn StdErr>> {
    fn segparse<'de, T: serde::de::Deserialize<'de>>(sfn: &str) -> Result<T, Box<dyn StdErr>> {
        let path = format!("lottie-rs/fixtures/segments/{}.json", sfn);
        Ok(deserial_err(&mut json_des::from_reader(File::open(&path)?))
            .inspect_err(|_| eprintln!("Failed parsing {path}"))?)
    }   use inlottie::schema::{FillStrokeGrad, TextRange, Transform};

    segparse::<Animation>("animated_position_legacy")?;
    segparse::<Transform>("transform_complex")?;
    segparse::<FillStrokeGrad>("gradient_fill")?;
    segparse::<FillStrokeGrad>("stroke")?;
    segparse::<TextRange>("text_range")?;

    Ok(())
}
