# Third-party software

Slotshift's application source and original icon are MIT-licensed. It uses the following UI foundation rather than a browser-based runtime:

- GPUI Kit 0.7.1, GPUI Base, GPUI Component, and GPUI Kit Assets: Apache-2.0. https://github.com/longbridge/gpui-kit
- Matching GPUI pre-release snapshot 0.3.8 and platform crates: licenses are recorded in Cargo metadata and the bundled crate notices. https://github.com/zed-industries/zed

The precise dependency versions are pinned in `Cargo.lock`. Other dependencies retain their own licenses and copyrights.

Windows release archives include `THIRD_PARTY_LICENSES.txt`, generated from the resolved Windows dependency graph and the license/notice files supplied with those crates. The collector includes package names, versions, declared licenses, upstream locations, and available license text. It does not copy font binaries, account data, or local source paths into the release.

To regenerate the catalog, run `python scripts/collect-licenses.py` after dependencies have been fetched. System components, the separately installed Codex CLI, and Windows Terminal are not redistributed with Slotshift.