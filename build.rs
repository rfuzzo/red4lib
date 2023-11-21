use cmake::Config;
use std::{fs::File, path::PathBuf};

// print build script logs
macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo:warning={}", format!($($tokens)*))
    }
}

fn main() {
    let dst = Config::new("kraken").build_target("kraken_static").build();

    // info
    let profile = std::env::var("PROFILE").unwrap();
    p!("PROFILE : {}", profile);
    if profile == "debug" {
        p!("DST: {}", dst.display());
    }

    // link
    println!(
        "cargo:rustc-link-search=native={}/build/bin/CMake/{}",
        dst.display(),
        profile
    );
    println!("cargo:rustc-link-lib=static=kraken_static");

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
