//! Self-update from GitHub releases, through fastframe-update.
//!
//! The previous release's helper installs the first release built on the
//! crate, so what it relies on stays the same: `--version` prints
//! `<command> <version>`, the new app accepts `--update-receipt` and
//! `--update-error` (stripped by [`fastframe_update::intercept`] before the
//! argument parser), and the handoff and receipt files keep their format.

pub use fastframe_update::{
    CHECK_INTERVAL, DownloadState, Installation, Kind, Prepared, Release, Source, Unsupported,
    Updater,
};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig};

pub const CONFIG: UpdateConfig = UpdateConfig {
    macos: MacConfig {
        bundle_ids: &["rocks.spotifast.Spotifast"],
        executable_names: &["Spotifast"],
        legacy_bundle_names: &[],
    },
    // Releases publish an Ed25519 signature for checksums.txt. Require it
    // before downloading an update, using the release workflow's public key.
    // This fork signs with its own key, so an upstream build can never be
    // installed over it by the updater, and the other way round.
    publisher_key: Some(include_str!("../assets/update-public-key.hex")),
    // Fork versions carry a suffix (0.14.0-spicy.1), which semver reads as a
    // pre-release. The channel considers every release of this repository
    // and takes the highest, so fork builds update from fork releases only.
    prereleases: fastframe_update::Prereleases::WhenRunningPrerelease,
    // Upstream AppImages replace themselves (appimage: true, #737). The fork
    // builds its AppImages after the release, without the native-packages
    // name, the .zsync file or the update information. Thus the fork keeps
    // the default: an AppImage tells the user to use the download page.
    appimage: false,
    ..UpdateConfig::new(
        "TheBestIT/spotifast-spicylyrics",
        "Spotifast",
        "spotifast",
        env!("CARGO_PKG_VERSION"),
    )
};

/// An updater on Spotifast's HTTP client, through the configured proxy.
pub fn updater(proxy: &crate::settings::ProxyConfig) -> anyhow::Result<Updater> {
    let builder = crate::http::blocking_builder(proxy).map_err(anyhow::Error::msg)?;
    Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn updates_require_the_release_signing_key() {
        assert!(
            CONFIG.publisher_key.is_some(),
            "updates must require a publisher signature"
        );
        CONFIG.validate().unwrap();
    }

    /// The fork AppImages do not carry the native-packages name that the
    /// updater downloads (packages.yml). Thus an AppImage must not try to
    /// replace itself.
    #[test]
    fn fork_appimages_do_not_update_themselves() {
        const { assert!(!CONFIG.appimage) };
    }
}
