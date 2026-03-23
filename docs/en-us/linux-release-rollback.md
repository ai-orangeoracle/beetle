# Linux install and versions (current status)

**English** | [中文](../zh-cn/linux-release-rollback.md)

## Who this is for

**Most users can skip this.** There is no polished one-click or single-command install for Linux yet; the musl bundles and layout notes are mainly for **integrators and operators** who need to line up release artifacts.  
A smoother path (e.g. **one-click** or **install over SSH**) is planned; user-facing docs will be updated when that exists.

## If you are installing manually

Each tarball includes `README.txt` and a sample `beetle.service`. If you must deploy by hand, a common pattern is: extract under `/opt/beetle/releases/<version>/`, point a `current` symlink at the version to run, and use `BEETLE_STATE_ROOT` or defaults for state (see `src/platform/state_root.rs`). Follow the in-bundle notes—**this is not** the long-term end-user story.

## Relation to CI

Releases ship checksums and build provenance; installation and rollback on the device are your responsibility and are not simulated in CI.
