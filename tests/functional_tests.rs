/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use red4lib::archive::{extract_archive, Archive};

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

        let result = extract_archive(archive_path, dst_path);
        assert!(result.is_ok());
    }
}
