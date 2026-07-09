//! Export/import a portable tar.gz bundle of everything the old desktop app stored
//! locally (`donna.sqlite`, the knowledge-base tree, keychain secrets) so it can be
//! migrated onto donna-server in one shot.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::error::Result;

/// Write `db_path`, `kb_dir` (stored under the `knowledge-base/` prefix regardless of
/// its source folder name), and `secrets` (as `secrets.json`) into a tar.gz at `out`.
pub fn write_bundle(
    out: &Path,
    db_path: &Path,
    kb_dir: &Path,
    secrets: &BTreeMap<String, String>,
) -> Result<()> {
    let file = File::create(out)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_path_with_name(db_path, "donna.sqlite")?;

    if kb_dir.is_dir() {
        tar.append_dir_all("knowledge-base", kb_dir)?;
    }

    let secrets_json = serde_json::to_vec_pretty(secrets)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(secrets_json.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    tar.append_data(&mut header, "secrets.json", secrets_json.as_slice())?;

    tar.into_inner()?.finish()?;
    Ok(())
}

/// Unpack a bundle produced by [`write_bundle`] into `data_dir`, so `donna.sqlite`,
/// `knowledge-base/`, and `secrets.json` land directly under it.
pub fn import_bundle(bundle: &Path, data_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let file = File::open(bundle)?;
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    // ponytail: tar's default unpack() already refuses `..` path traversal.
    archive.unpack(data_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "donna-{name}-{}-{}",
            std::process::id(),
            crate::db::unique_test_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn bundle_roundtrip() {
        let src = tempdir("bundle-src");
        let dst = tempdir("bundle-dst");
        std::fs::write(src.join("donna.sqlite"), b"fake-db").unwrap();
        std::fs::create_dir_all(src.join("kb/People")).unwrap();
        std::fs::write(src.join("kb/People/alex.md"), b"# Alex").unwrap();
        let secrets = BTreeMap::from([("api_key:openai".to_string(), "sk-1".to_string())]);
        let out = src.join("bundle.tar.gz");
        write_bundle(&out, &src.join("donna.sqlite"), &src.join("kb"), &secrets).unwrap();
        import_bundle(&out, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("donna.sqlite")).unwrap(), b"fake-db");
        assert_eq!(
            std::fs::read(dst.join("knowledge-base/People/alex.md")).unwrap(),
            b"# Alex"
        );
        let m: BTreeMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(dst.join("secrets.json")).unwrap()).unwrap();
        assert_eq!(m["api_key:openai"], "sk-1");
    }
}
