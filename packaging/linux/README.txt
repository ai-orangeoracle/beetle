Beetle Linux bundle (musl)
==========================

Audience
--------
This tarball is for integrators and manual trials. **End-user one-click / SSH install is not the story yet**—that will follow in a separate product flow. Optional context: docs/en-us/linux-release-rollback.md (or docs/zh-cn/linux-release-rollback.md).

Binary
------
- `beetle`: statically linked (musl). If you deploy by hand, a common layout is `/opt/beetle/releases/<version>/` plus a `current` symlink; see the markdown above for current status only.

Config API (optional)
---------------------
- Set `BEETLE_CONFIG_HTTP_LISTEN` to e.g. `127.0.0.1:8080` to enable the config HTTP server on Linux.
- Default state root: `/var/lib/beetle` or `/data/beetle`, or override with `BEETLE_STATE_ROOT`.

Hardware JSON
-------------
- Copy `hardware.json.example` to your config path (e.g. under the state root `config/hardware.json`) and adjust `backlight_path` / `backlight_max` for your board.
- SG2002 / LicheeRV Nano style boards often use `backlight0` and `backlight_max` 100 instead of the Luckfox example in this file.

systemd
-------
- Edit `beetle.service`: set `User=`/`Group=` and tighten `ReadWritePaths=` for your deployment.
- Install: copy unit to `/etc/systemd/system/`, `systemctl daemon-reload`, `systemctl enable --now beetle`.

Permissions
-----------
- Prefer a dedicated user; config/secrets `0600` or `0640`; state directory `0700` per project baseline.
