#![warn(clippy::all, rust_2018_idioms)]

mod cr2w;
mod io;

pub mod archive;
pub mod kraken;

use std::{collections::HashMap, hash::Hasher, path::Path};

use sha1::{Digest, Sha1};
use strum_macros::{Display, EnumIter};

/////////////////////////////////////////////////////////////////////////////////////////
// HELPERS
/////////////////////////////////////////////////////////////////////////////////////////

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

pub fn sha1_hash_file(file_buffer: &Vec<u8>) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(file_buffer);
    let result = hasher.finalize();
    result.into()
}

/// Get vanilla resource path hashes https://www.cyberpunk.net/en/modding-support
pub fn get_red4_hashes() -> HashMap<u64, String> {
    let kark_data = include_bytes!("../WolvenKit/WolvenKit.Common/Resources/usedhashes.kark");

    // parse KARK header: 4-byte magic, 4-byte decompressed size
    let magic = u32::from_le_bytes(kark_data[0..4].try_into().unwrap());
    assert_eq!(magic, kraken::MAGIC, "Invalid KARK magic");
    let decompressed_size = u32::from_le_bytes(kark_data[4..8].try_into().unwrap()) as usize;

    // decompress
    let compressed = kark_data[8..].to_vec();
    let mut decompressed = Vec::new();
    kraken::decompress(compressed, &mut decompressed, decompressed_size);

    // parse: one resource path per line, compute FNV1a64 hash
    let text = String::from_utf8_lossy(&decompressed);
    let mut map: HashMap<u64, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let hash = fnv1a64_hash_string(&line.to_string());
        map.insert(hash, line.to_owned());
    }

    map
}

/////////////////////////////////////////////////////////////////////////////////////////
// TESTS
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

/////////////////////////////////////////////////////////////////////////////////////////
// RED4 LIB
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
