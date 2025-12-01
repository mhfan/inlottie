
//use std::io::Result;
#[cfg(feature = "rive-rs")]
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[cfg(feature = "rive-rs")] #[test] fn parse_rive_file() -> Result<()> {
    use std::{fs, path::Path, io::BufReader, collections::HashMap};
    use inlottie::rive_nvg::schema::RiveFile;

    let assets_dir = "data";
    //let assets_dir = "rive-rs/submodules/rive-cpp/tests/unit_tests/assets";

    fn visit_assets_dir<F>(dir: &Path, callback: &mut F) -> Result<()>
        where F: FnMut(&Path) -> Result<()> {
        if !dir.is_dir() { return callback(dir) }
        fs::read_dir(dir)?.flatten().try_for_each(|entry| {
            let path = entry.path();
            if  path.is_dir() { visit_assets_dir(&path, callback)
            } else { callback(&path) }
        })
    }

    //for path in glob::glob(&format!("{assets_dir}/**/*.riv"))? { }
    visit_assets_dir(Path::new(assets_dir), &mut |path| {
        if path.extension().is_none_or(|s| s != "riv") { return Ok(()) }
        println!("\nParsing Rive file: {}", path.display());
        let rive_file = RiveFile::read(&mut
            BufReader::new(fs::File::open(path)?))?;

        println!("  Version: v{}.{}", rive_file.header.majorv.0,
                                      rive_file.header.minorv.0);
        println!("  File ID: {}", rive_file.header.fileid.0);
        println!("  Objects: {}", rive_file.ocoll.len());

        let mut type_counts = HashMap::new();
        for obj in &rive_file.ocoll {
            let obj_len = obj.props.len() as u32;
            let entry = type_counts.entry(obj.type_id)
                .or_insert((0u32, obj_len, 0u32));  entry.0 += 1;
            if  obj_len < entry.1 { entry.1 = obj_len; }
            if  entry.2 < obj_len { entry.2 = obj_len; }
        }

        println!("\nDiscovered object types:");
        for (&type_id, &count) in type_counts.iter() {
            println!("  Type {:6}: {:4} objects, {:2} ~ {:2} properties",
                type_id.0, count.0, count.1, count.2);
        }   println!();     Ok(())
    })
}
