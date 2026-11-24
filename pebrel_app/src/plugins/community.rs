//! Fetch the Winc community catalog and stage pinned manifests for local review.

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use super::install::{Library, Request};
use super::package::{MAX_MANIFEST_BYTES, Manifest, identifier, label};
use super::{Error, Result};

pub(crate) const REPOSITORY: &str = "https://github.com/suzikuo/pebrel-winc-plugins";
const RAW: &str = "https://raw.githubusercontent.com/suzikuo/pebrel-winc-plugins";
const EXECUTABLE_SLOT: &str = "{select-executable}";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema: u16,
    plugins: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub id: String,
    pub version: String,
    pub name: String,
    pub name_zh: String,
    pub description: String,
    pub description_zh: String,
    revision: String,
    #[serde(default)]
    pub windows_only: bool,
    #[serde(default)]
    pub choose_executable: bool,
}

impl Entry {
    pub(crate) fn supported(&self) -> bool {
        !self.windows_only || cfg!(windows)
    }

    pub(crate) fn source_url(&self) -> String {
        format!("{REPOSITORY}/tree/{}/plugins/{}", self.revision, self.id)
    }

    fn package_url(&self) -> String {
        format!("{RAW}/{}/plugins/{}/plugin.toml", self.revision, self.id)
    }
}

pub(crate) fn fetch_catalog() -> Result<Vec<Entry>> {
    let bytes = fetch(&format!("{RAW}/main/catalog.json"), 64 * 1024)?;
    parse_catalog(&bytes)
}

fn parse_catalog(bytes: &[u8]) -> Result<Vec<Entry>> {
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(catalog_error)?;
    if catalog.schema != 1 || catalog.plugins.len() > 64 {
        return Err(catalog_error("unsupported catalog schema or size"));
    }
    for (index, entry) in catalog.plugins.iter().enumerate() {
        if !identifier(&entry.id)
            || !label(&entry.version, 64)
            || !label(&entry.name, 128)
            || !label(&entry.name_zh, 128)
            || !label(&entry.description, 512)
            || !label(&entry.description_zh, 512)
            || entry.revision.len() != 40
            || !entry.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            || (entry.choose_executable && !entry.windows_only)
            || catalog.plugins[..index].iter().any(|other| other.id == entry.id)
        {
            return Err(catalog_error("invalid or duplicate plugin entry"));
        }
    }
    Ok(catalog.plugins)
}

pub(crate) fn download(
    entry: &Entry,
    executable: Option<&Path>,
) -> Result<(tempfile::TempDir, Request)> {
    if !entry.supported() {
        return Err(catalog_error("plugin is unavailable on this platform"));
    }
    let bytes = fetch(&entry.package_url(), MAX_MANIFEST_BYTES)?;
    stage(entry, &bytes, executable)
}

fn stage(
    entry: &Entry,
    bytes: &[u8],
    executable: Option<&Path>,
) -> Result<(tempfile::TempDir, Request)> {
    let source = std::str::from_utf8(bytes).map_err(package_error)?;
    let mut manifest: Manifest = toml::from_str(source).map_err(package_error)?;
    if manifest.id != entry.id || manifest.version != entry.version || manifest.name != entry.name {
        return Err(package_error("manifest metadata does not match the catalog"));
    }
    if manifest.entry.is_some() || !manifest.themes.is_empty() {
        return Err(package_error("the catalog supports standalone manifests"));
    }
    if manifest
        .directory_launchers
        .iter()
        .any(|launcher| launcher.executable != EXECUTABLE_SLOT && !launcher.opens_explorer())
    {
        return Err(package_error("community launchers require a locally selected executable"));
    }
    let mut slots = manifest
        .directory_launchers
        .iter_mut()
        .filter(|launcher| launcher.executable == EXECUTABLE_SLOT);
    let slot = slots.next();
    if slots.next().is_some() || slot.is_some() != entry.choose_executable {
        return Err(package_error("unexpected executable selection slot"));
    }
    if let Some(launcher) = slot {
        let path = executable
            .filter(|path| path.is_file())
            .and_then(Path::to_str)
            .ok_or_else(|| package_error("select a local Windows executable"))?;
        launcher.executable = path.into();
    }
    // Keep the usual parser and permission review authoritative after resolving local paths.
    let package =
        tempfile::Builder::new().prefix("pebrel-community-").tempdir().map_err(package_error)?;
    let manifest = toml::to_string(&manifest).map_err(package_error)?;
    fs::write(package.path().join("plugin.toml"), manifest).map_err(package_error)?;
    let request = Library::inspect(package.path())?;
    Ok((package, request))
}

fn fetch(url: &str, limit: usize) -> Result<Vec<u8>> {
    let agent = ureq::config::Config::builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .max_redirects(0)
        .https_only(true)
        .build()
        .new_agent();
    let mut response = agent.get(url).call().map_err(network_error)?;
    if response.status() != 200 {
        return Err(network_error(response.status()));
    }
    response.body_mut().with_config().limit(limit as u64).read_to_vec().map_err(network_error)
}

fn network_error(error: impl std::fmt::Display) -> Error {
    Error::new("community_download_failed", error)
}

fn catalog_error(error: impl std::fmt::Display) -> Error {
    Error::new("invalid_community_catalog", error)
}

fn package_error(error: impl std::fmt::Display) -> Error {
    Error::new("invalid_community_package", error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::Grant;
    use serde_json::{Value, json};

    const SCRATCH: &[u8] = include_bytes!("../../examples/plugins/scratch-terminal/plugin.toml");

    fn entry() -> Value {
        json!({
            "id": "local.scratch-terminal", "version": "1.0.0", "name": "Scratch Terminal",
            "name_zh": "临时终端", "description": "Open a terminal.", "description_zh": "打开终端。",
            "revision": "0123456789abcdef0123456789abcdef01234567"
        })
    }

    fn catalog(entries: &[Value]) -> Result<Vec<Entry>> {
        parse_catalog(&serde_json::to_vec(&json!({"schema": 1, "plugins": entries})).unwrap())
    }

    #[test]
    fn pinned_manifest_uses_the_existing_review_and_install_rules() {
        let entry = catalog(&[entry()]).unwrap().remove(0);
        assert!(entry.package_url().ends_with(
            "/0123456789abcdef0123456789abcdef01234567/plugins/local.scratch-terminal/plugin.toml"
        ));
        let (package, request) = stage(&entry, SCRATCH, None).unwrap();
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(root.path());
        assert!(library.list().unwrap().plugins.is_empty());
        assert_eq!(request.requested, [Grant::WorkspaceUi]);
        assert_eq!(library.install(package.path(), &[]).unwrap_err().code, "grant_required");
        let installed = library.install_reviewed(package.path(), &request).unwrap();
        assert!(installed.workspace.unwrap().scratch_terminal);
        assert!(stage(&entry, b"name = 'other'", None).is_err());
    }

    #[test]
    fn catalog_rejects_duplicate_ids_and_unpinned_paths() {
        assert!(catalog(&[entry(), entry()]).is_err());
        for (key, value) in [("revision", "main"), ("id", "../scratch"), ("id", "a%2fb")] {
            let mut bad = entry();
            bad[key] = json!(value);
            assert!(catalog(&[bad]).is_err());
        }
    }
}
