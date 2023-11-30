use cmake::Config;
use std::{fs::File, path::PathBuf};

// print build script logs
macro_rules! p {
  ($($tokens: tt)*) => {
      println!("cargo:warning={}", format!($($tokens)*))
  }
}

const LIB_NAME: &str = "kraken";
const TARGET_NAME: &str = "kraken_static";

fn main() {
    // cmake config
    let mut cfg = Config::new(LIB_NAME);
    //cfg.profile("RelWithDebInfo");
    //cfg.profile("Debug");
    let dst = cfg.build_target(TARGET_NAME).build();

    // logging
    let cmake_profile: String = cfg.get_profile().to_owned();
    let rust_profile = std::env::var("PROFILE").unwrap();
    p!("CMAKE_PROFILE : {}", cmake_profile);
    p!("RUST_PROFILE : {}", rust_profile);
    p!("DST: {}", dst.display());

    // link
    let mut link_path = format!("{}/build/bin/CMake", dst.display());
    let mut additional_args = "".to_owned();
    if cfg!(windows) {
        link_path = format!("{}/{}", link_path, cmake_profile);
    } else if cfg!(unix) {
        additional_args = "-l".to_owned();
    }

    // link rustc
    println!("cargo:rustc-link-search=native={}", link_path);
    println!(
        "cargo:rustc-link-lib{}=static={}",
        additional_args, TARGET_NAME
    );

    // extract resources
    let file_path = PathBuf::from("src/metadata-resources.csv");
    if file_path.exists() {
        p!("file exists: {}", file_path.display());
    } else {
        p!("extracting file: {}", file_path.display());
        let f = File::open("src/metadata-resources.zip").expect("failed to open resource file");
        let mut zip = zip::ZipArchive::new(f).expect("fialed to open zip file");
        zip.extract("src/").expect("failed to extract zip file");
    }
}
