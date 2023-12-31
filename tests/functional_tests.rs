/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::create_dir_all;
    use std::io::{self, Read};
    use std::path::Path;
    use std::time::Instant;
    use std::{fs, path::PathBuf};

    use red4lib::archive::*;
    use red4lib::io::FromReader;
    use red4lib::*;

    #[test]
    fn time_csv() {
        let start = Instant::now();
        let hashes = get_red4_hashes();
        assert!(!hashes.is_empty());
        let end = Instant::now();
        let duration = end - start;
        println!("Execution time csv: {:?}", duration);
    }

    #[test]
    fn read_srxl() {
        let file_path = PathBuf::from("tests").join("srxl.bin");
        let mut file = fs::File::open(file_path).expect("Could not open file");
        let mut buffer: Vec<u8> = vec![];
        file.read_to_end(&mut buffer).expect("Could not read file");

        let mut cursor = io::Cursor::new(&buffer);

        let _srxl = LxrsFooter::from_reader(&mut cursor).unwrap();
    }

    #[test]
    fn read_archive() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let result = Archive::from_file(&archive_path);
        assert!(result.is_ok());
    }

    #[test]
    fn read_archive2() {
        let archive_path = PathBuf::from("tests").join("nci.archive");
        let result = Archive::from_file(&archive_path);
        assert!(result.is_ok());
    }

    #[test]
    fn read_custom_data() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let archive = Archive::from_file(&archive_path).expect("Could not parse archive");
        let mut file_names = archive
            .file_names
            .values()
            .map(|f| f.to_owned())
            .collect::<Vec<_>>();
        file_names.sort();

        let expected: Vec<String> = vec!["base\\cycleweapons\\localization\\en-us.json".to_owned()];
        assert_eq!(expected, file_names);
    }

    #[test]
    fn test_extract_archive() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let dst_path = PathBuf::from("tests").join("out");
        let data_path = PathBuf::from("tests").join("data");
        let hashes = get_red4_hashes();

        // delete folder if exists
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }

        let result = extract_archive(&archive_path, &dst_path, &hashes);
        assert!(result.is_ok());

        // check
        let binding = get_files_in_folder_recursive(&data_path);
        let mut expected_files = binding
            .iter()
            .filter(|f| !f.ends_with(".DS_Store"))
            .collect::<Vec<_>>();
        let mut expected = expected_files
            .iter()
            .map(|f| {
                // Convert the absolute path to a relative path
                if let Ok(relative_path) = f.strip_prefix(&data_path) {
                    relative_path.to_owned()
                } else {
                    panic!("Could not construct relative path")
                }
            })
            .map(|f| f.to_string_lossy().to_ascii_lowercase())
            .map(|f| f.replace('\\', "/"))
            .collect::<Vec<_>>();

        let binding = get_files_in_folder_recursive(&dst_path);
        let mut found_files = binding
            .iter()
            .filter(|f| !f.ends_with(".DS_Store"))
            .collect::<Vec<_>>();
        let mut found = found_files
            .iter()
            .map(|f| {
                // Convert the absolute path to a relative path
                if let Ok(relative_path) = f.strip_prefix(&dst_path) {
                    relative_path.to_owned()
                } else {
                    panic!("Could not construct relative path")
                }
            })
            .map(|f| f.to_string_lossy().to_ascii_lowercase())
            .map(|f| f.replace('\\', "/"))
            .collect::<Vec<_>>();

        expected.sort();
        found.sort();
        expected_files.sort();
        found_files.sort();

        assert_eq!(expected.len(), found.len());
        assert_eq!(expected, found);

        for (i, e) in expected_files.into_iter().enumerate() {
            let f = found_files.get(i).unwrap();
            assert_binary_equality(e, f);
        }

        // cleanup
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
    }

    #[test]
    fn test_pack_archive() {
        // pack test data
        let data_path = PathBuf::from("tests").join("data");
        let dst_path = PathBuf::from("tests").join("out2");
        let hash_map = get_red4_hashes();

        // delete folder if exists
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
        create_dir_all(&dst_path).expect("Could not create folder");

        let result = write_archive(&data_path, &dst_path, None, hash_map);
        assert!(result.is_ok());

        // checks
        let created_path = dst_path.join("data.archive");
        assert!(created_path.exists());

        // TODO binary equality
        // let existing_path = PathBuf::from("tests").join("test1.archive");
        // assert_binary_equality(&existing_path, &created_path);

        // cleanup
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
    }

    /////////////////////////////////////////////////////////////////////////////////////////
    /// HELPERS
    /////////////////////////////////////////////////////////////////////////////////////////

    fn assert_binary_equality(e: &PathBuf, f: &PathBuf) {
        // compare bytes
        let mut fe = fs::File::open(e).expect("Could not open file");
        let mut fe_buffer = Vec::new();
        std::io::Read::read_to_end(&mut fe, &mut fe_buffer).expect("Could not open file");
        let fe_hash = format!("{:02X?}", sha1_hash_file(&fe_buffer));

        let mut ff = fs::File::open(f).expect("Could not open file");
        let mut ff_buffer = Vec::new();
        std::io::Read::read_to_end(&mut ff, &mut ff_buffer).expect("Could not open file");
        let ff_hash = format!("{:02X?}", sha1_hash_file(&ff_buffer));

        // hash for nicer error msg
        assert_eq!(fe_hash, ff_hash);
    }

    fn get_files_in_folder_recursive<P: AsRef<Path>>(folder_path: &P) -> Vec<PathBuf> {
        // Read the directory
        if let Ok(entries) = fs::read_dir(folder_path) {
            let mut files = Vec::new();

            // Iterate over directory entries
            for entry in entries.flatten() {
                let path = entry.path();

                // Check if the entry is a file
                if path.is_file() {
                    files.push(path.to_path_buf());
                } else if path.is_dir() {
                    // Recursively get files in subdirectories
                    let subfolder_files = get_files_in_folder_recursive(&path);
                    files.extend(subfolder_files);
                }
            }

            return files;
        }

        // Return an empty vector if there's an error
        Vec::new()
    }
}
