#![warn(clippy::all, rust_2018_idioms)]

pub mod archive;
pub mod cr2w;
pub mod io;
pub mod kraken;

use std::collections::HashMap;
use std::fs::{self};
use std::hash::Hasher;
use std::path::{Path, PathBuf};

use strum_macros::{Display, EnumIter};

/////////////////////////////////////////////////////////////////////////////////////////
/// RED4 LIB
/////////////////////////////////////////////////////////////////////////////////////////

#[allow(non_camel_case_types)]
#[derive(Debug, EnumIter, Display)]
enum ERedExtension {
    unknown,

    acousticdata,
    actionanimdb,
    aiarch,
    animgraph,
    anims,
    app,
    archetypes,
    areas,
    audio_metadata,
    audiovehcurveset,
    behavior,
    bikecurveset,
    bk2,
    bnk,
    camcurveset,
    ccstate,
    cfoliage,
    charcustpreset,
    chromaset,
    cminimap,
    community,
    conversations,
    cooked_mlsetup,
    cookedanims,
    cookedapp,
    cookedprefab,
    credits,
    csv,
    cubemap,
    curveresset,
    curveset,
    dat,
    devices,
    dlc_manifest,
    dtex,
    effect,
    ent,
    env,
    envparam,
    envprobe,
    es,
    facialcustom,
    facialsetup,
    fb2tl,
    fnt,
    folbrush,
    foldest,
    fp,
    game,
    gamedef,
    garmentlayerparams,
    genericanimdb,
    geometry_cache,
    gidata,
    gradient,
    hitrepresentation,
    hp,
    ies,
    inkanim,
    inkatlas,
    inkcharcustomization,
    inkenginesettings,
    inkfontfamily,
    inkfullscreencomposition,
    inkgamesettings,
    inkhud,
    inklayers,
    inkmenu,
    inkshapecollection,
    inkstyle,
    inktypography,
    inkwidget,
    interaction,
    journal,
    journaldesc,
    json,
    lane_connections,
    lane_polygons,
    lane_spots,
    lights,
    lipmap,
    location,
    locopaths,
    loot,
    mappins,
    matlib,
    mesh,
    mi,
    mlmask,
    mlsetup,
    mltemplate,
    morphtarget,
    mt,
    null_areas,
    opusinfo,
    opuspak,
    particle,
    phys,
    physicalscene,
    physmatlib,
    poimappins,
    psrep,
    quest,
    questphase,
    redphysics,
    regionset,
    remt,
    reps,
    reslist,
    rig,
    scene,
    scenerid,
    scenesversions,
    smartobject,
    smartobjects,
    sp,
    spatial_representation,
    streamingblock,
    streamingquerydata,
    streamingsector,
    streamingsector_inplace,
    streamingworld,
    terrainsetup,
    texarray,
    traffic_collisions,
    traffic_persistent,
    vehcommoncurveset,
    vehcurveset,
    voicetags,
    w2mesh,
    w2mi,
    wem,
    workspot,
    worldlist,
    xbm,
    xcube,

    wdyn,
}
#[warn(non_camel_case_types)]
/////////////////////////////////////////////////////////////////////////////////////////
/// HELPERS
/////////////////////////////////////////////////////////////////////////////////////////

/// Get top-level files of a folder with given extension
pub fn get_files(folder_path: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !folder_path.exists() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(ext) = entry.path().extension() {
                        if ext == extension {
                            files.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    files
}

/// Calculate FNV1a64 hash of a String
pub fn fnv1a64_hash_string(str: &String) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    hasher.write(str.as_bytes());
    hasher.finish()
}

/// Calculate FNV1a64 hash of a PathBuf
pub fn fnv1a64_hash_path(path: &Path) -> u64 {
    let path_string = path.to_string_lossy();
    let mut hasher = fnv::FnvHasher::default();
    hasher.write(path_string.as_bytes());
    hasher.finish()
}

/// Get vanilla resource path hashes https://www.cyberpunk.net/en/modding-support
pub fn get_red4_hashes() -> HashMap<u64, String> {
    let csv_data = include_bytes!("metadata-resources.csv");
    parse_csv_data(csv_data)
}

/// Reads the metadata-resources.csv (csv of hashes and strings) from https://www.cyberpunk.net/en/modding-support
fn parse_csv_data(csv_data: &[u8]) -> HashMap<u64, String> {
    let mut reader = csv::ReaderBuilder::new().from_reader(csv_data);
    let mut csv_map: HashMap<u64, String> = HashMap::new();

    for result in reader.records() {
        match result {
            Ok(record) => {
                // Assuming the CSV has two columns: String and u64
                if let (Some(path), Some(hash_str)) = (record.get(0), record.get(1)) {
                    if let Ok(hash) = hash_str.parse::<u64>() {
                        csv_map.insert(hash, path.to_string());
                    } else {
                        eprintln!("Error parsing u64 value: {}", hash_str);
                    }
                } else {
                    eprintln!("Malformed CSV record: {:?}", record);
                }
            }
            Err(err) => eprintln!("Error reading CSV record: {}", err),
        }
    }

    csv_map
}

/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    #[test]
    fn load_order() {
        let mut input = [
            "#.archive",
            "_.archive",
            "aa.archive",
            "zz.archive",
            "AA.archive",
            "ZZ.archive",
        ];
        let correct = [
            "#.archive",
            "AA.archive",
            "ZZ.archive",
            "_.archive",
            "aa.archive",
            "zz.archive",
        ];

        input.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        //input.sort();
        assert_eq!(correct, input);
    }
}
