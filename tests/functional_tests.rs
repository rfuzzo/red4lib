/////////////////////////////////////////////////////////////////////////////////////////
// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::create_dir_all;
    use std::path::Path;
    use std::time::Instant;
    use std::{fs, path::PathBuf};

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
    fn test_extract_archive() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let dst_path = PathBuf::from("tests").join("out");
        let data_path = PathBuf::from("tests").join("data");
        let hashes = get_red4_hashes();

        // delete folder if exists
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }

        let result =
            archive::extract_to_directory_path(&archive_path, &dst_path, true, Some(hashes));
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
        let dst_file = dst_path.join("data.archive");

        // delete folder if exists
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
        create_dir_all(&dst_path).expect("Could not create folder");

        let result = archive::create_from_directory_path(&data_path, &dst_file, None);
        assert!(result.is_ok());

        // checks
        assert!(dst_file.exists());

        // TODO binary equality
        // let existing_path = PathBuf::from("tests").join("test1.archive");
        // assert_binary_equality(&existing_path, &created_path);

        // cleanup
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
    }

    /////////////////////////////////////////////////////////////////////////////////////////
    // HELPERS
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
