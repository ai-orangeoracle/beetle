# Linux release layout and rollback

**English** | [中文](../zh-cn/linux-release-rollback.md)

This document describes the recommended on-disk layout, version switching, and rollback procedure for **beetle** on low-end Linux SBCs. Directory and permission baselines align with the project’s migration plan (operational section); internal-only paths are not linked here.

## 1. Recommended layout

| Path | Purpose |
|------|---------|
| `/opt/beetle/releases/<version>/` | Contents of one release bundle (`beetle`, `beetle.service`, `README.txt`, etc.) |
| `/opt/beetle/current` | **Symbolic link** to the version that should run |
| `/var/lib/beetle` or `/data/beetle` | Runtime state and config (via `BEETLE_STATE_ROOT` or defaults; see `src/platform/state_root.rs`) |

`<version>` matches the bundle name, e.g. `v0.1.0` (aligned with the Git tag and `package.version` in `Cargo.toml`).

## 2. First install

1. Verify `SHA256SUMS` from the release page.
2. Create: `sudo mkdir -p /opt/beetle/releases/vX.Y.Z`.
3. Extract `beetle-vX.Y.Z-linux-<arch>-musl.tar.gz` into that directory (top-level folder name matches the archive basename).
4. Set permissions: `sudo chmod 755 /opt/beetle/releases/vX.Y.Z/beetle`; use a dedicated user and `0600`/`0700` for config/state as appropriate.
5. Point the current link: `sudo ln -sfn /opt/beetle/releases/vX.Y.Z /opt/beetle/current`.
6. Optional systemd: copy `beetle.service` to `/etc/systemd/system/`, edit `User`/`Group`/`ReadWritePaths`, then `systemctl daemon-reload && systemctl enable --now beetle`.

## 3. Upgrade

1. Extract the new release under `/opt/beetle/releases/<new>`.
2. Switch the link: `sudo ln -sfn /opt/beetle/releases/<new> /opt/beetle/current`.
3. Restart: `sudo systemctl restart beetle` (or SysV: `./beetle.init restart`).

## 4. Roll back

1. Ensure the previous release directory still exists (e.g. `/opt/beetle/releases/v0.1.0`).
2. Run: `sudo ln -sfn /opt/beetle/releases/<previous> /opt/beetle/current`.
3. `sudo systemctl restart beetle`.
4. Verify: process is up; if the config HTTP API is enabled, `GET http://<listen>/api/health`; logs look healthy.

## 5. Failure drills (recommended before production)

- Point `current` at a wrong path and confirm failure is observable; restore the symlink and confirm recovery.
- `kill -9` the main process and confirm systemd restarts it (if `Restart=` is set).

## 6. Relation to CI

Rollback is performed **on the device**. The GitHub Actions release workflow ships tarballs, checksums, and build provenance; it does not simulate symlinks on hardware.
