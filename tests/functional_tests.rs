/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::create_dir_all;
    use std::path::Path;
    use std::{fs, path::PathBuf};

    use red4lib::archive::write_archive;
    use red4lib::{
        archive::{extract_archive, Archive},
        get_red4_hashes,
    };

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

    #[test]
    fn read_archive() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
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
        let expected_files = get_files_in_folder_recursive(&data_path);
        let expected = expected_files
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
            .collect::<Vec<_>>();

        let found_files = get_files_in_folder_recursive(&dst_path);
        let found = found_files
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
            .collect::<Vec<_>>();

        assert_eq!(expected.len(), found.len());
        assert_eq!(expected, found);

        for (i, e) in expected_files.into_iter().enumerate() {
            let f = found_files.get(i).unwrap();
            // compare bytes
            let mut fe = fs::File::open(&e).expect("Could not open file");
            let mut fe_buffer = Vec::new();
            std::io::Read::read_to_end(&mut fe, &mut fe_buffer).expect("Could not open file");

            let mut ff = fs::File::open(f).expect("Could not open file");
            let mut ff_buffer = Vec::new();
            std::io::Read::read_to_end(&mut ff, &mut ff_buffer).expect("Could not open file");

            assert_eq!(fe_buffer, ff_buffer);
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
        let expected_path = dst_path.join("data.archive");
        assert!(expected_path.exists());

        // TODO binary equality

        // cleanup
        if dst_path.exists() {
            assert!(fs::remove_dir_all(&dst_path).is_ok());
        }
    }
}
