use dprint_cli_core::types::ErrBox;
use std::cmp::Ordering;
use std::path::PathBuf;

use super::{get_plugin_dir, BinaryManifestItem, GlobalBinaryLocation, PluginsManifest};
use crate::configuration::ConfigFileBinary;
use crate::environment::Environment;
use crate::types::{CommandName, NameSelector, VersionSelector};
use crate::utils;

pub fn get_installed_binary_if_associated_config_file_binary<'a>(
    manifest: &'a PluginsManifest,
    config_binary: &ConfigFileBinary,
) -> Option<&'a BinaryManifestItem> {
    // the url needs to be associated to an identifier for this to return anything
    if let Some(identifier) = manifest.get_identifier_from_url(&config_binary.path) {
        // return the url version if installed
        if let Some(binary) = manifest.get_binary(&identifier) {
            return Some(binary);
        }

        // else check for the latest matching version in the manifest
        if let Some(version_selector) = &config_binary.version {
            let name_selector = identifier.get_binary_name().to_selector();
            let binary = get_latest_binary_matching_name_and_version(&manifest, &name_selector, version_selector);
            if let Some(binary) = binary {
                return Some(binary);
            }
        }
    }

    None
}

pub fn get_latest_binary_matching_name_and_version<'a>(
    manifest: &'a PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &VersionSelector,
) -> Option<&'a BinaryManifestItem> {
    let binaries = manifest.get_binaries_matching_name_and_version(&name_selector, version_selector);
    get_latest_binary(&binaries)
}

pub fn get_binary_with_name_and_version<'a>(
    plugin_manifest: &'a PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &VersionSelector,
) -> Result<&'a BinaryManifestItem, ErrBox> {
    let binaries = plugin_manifest.get_binaries_matching_name_and_version(name_selector, version_selector);

    if binaries.len() == 0 {
        let binaries = plugin_manifest.get_binaries_matching_name(name_selector);
        if binaries.is_empty() {
            err!("Could not find any installed binaries named '{}'", name_selector)
        } else {
            err!(
                "Could not find binary '{}' that matched version '{}'\n\nInstalled versions:\n  {}",
                name_selector,
                version_selector,
                display_binaries_versions(binaries).join("\n "),
            )
        }
    } else if !get_have_same_owner(&binaries) {
        return err!(
            "There were multiple binaries with the specified name '{}' that matched version '{}'. Please include the owner to uninstall.\n\nInstalled versions:\n  {}",
            name_selector,
            version_selector,
            display_binaries_versions(binaries).join("\n  "),
        );
    } else {
        Ok(get_latest_binary(&binaries).unwrap())
    }
}

pub fn display_binaries_versions(binaries: Vec<&BinaryManifestItem>) -> Vec<String> {
    if binaries.is_empty() {
        return Vec::new();
    }

    let mut binaries = binaries;
    binaries.sort();
    let have_same_owner = get_have_same_owner(&binaries);
    let lines = binaries
        .into_iter()
        .map(|b| {
            if have_same_owner {
                b.version.to_string()
            } else {
                format!("{} {}", b.name, b.version)
            }
        })
        .collect::<Vec<_>>();

    return lines;
}

pub fn get_have_same_owner(binaries: &Vec<&BinaryManifestItem>) -> bool {
    if binaries.is_empty() {
        true
    } else {
        let first_owner = &binaries[0].name.owner;
        binaries.iter().all(|b| &b.name.owner == first_owner)
    }
}

pub fn get_latest_binary<'a>(binaries: &Vec<&'a BinaryManifestItem>) -> Option<&'a BinaryManifestItem> {
    let mut latest_binary: Option<&'a BinaryManifestItem> = None;

    for binary in binaries.iter() {
        if let Some(latest_binary_val) = &latest_binary {
            if latest_binary_val.cmp(binary) == Ordering::Less {
                latest_binary = Some(binary);
            }
        } else {
            latest_binary = Some(binary);
        }
    }

    latest_binary
}

pub fn get_global_binary_file_name(
    environment: &impl Environment,
    plugin_manifest: &PluginsManifest,
    command_name: &CommandName,
) -> Result<PathBuf, ErrBox> {
    match plugin_manifest.get_global_binary_location(command_name) {
        Some(location) => match location {
            GlobalBinaryLocation::Path => {
                if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name)? {
                    Ok(path_executable_path)
                } else {
                    err!("Binary '{}' is configured to use the executable on the path, but only the bvm version exists on the path. Run `bvm use {0} <some other version>` to select a version to run.", command_name)
                }
            }
            GlobalBinaryLocation::Bvm(identifier) => {
                if let Some(item) = plugin_manifest.get_binary(&identifier) {
                    let plugin_cache_dir = get_plugin_dir(environment, &item.name, &item.version)?;
                    let command = item
                        .commands
                        .iter()
                        .filter(|c| &c.name == command_name)
                        .next()
                        .expect("Expected to have command.");
                    Ok(plugin_cache_dir.join(&command.path))
                } else {
                    err!("Should have found executable path for global binary. Report this as a bug and update the version used by running `bvm use {} <some other version>`", command_name)
                }
            }
        },
        None => {
            // use the executable on the path
            if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name)? {
                Ok(path_executable_path)
            } else {
                let binaries = plugin_manifest.get_binaries_with_command(command_name);
                if binaries.is_empty() {
                    err!("Could not find binary on the path for command '{}'", command_name)
                } else {
                    err!(
                        "No binary is set on the path for command '{}'. Run `bvm use {0} <version>` to set a global version.\n\nInstalled versions:\n  {}",
                        command_name,
                        display_binaries_versions(binaries).join("\n "),
                    )
                }
            }
        }
    }
}