# Linux 安装与版本（当前状态）

[English](../en-us/linux-release-rollback.md) | **中文** | [文档索引](../README.md)

## 面向谁

普通用户**不必**读本文。当前 Linux 侧没有「一键烧录 / 一条命令装好」的体验；musl 压缩包与目录约定主要给**集成、运维或自己玩板子的人**对齐发布物用。  
后续会单独做**一键安装**或 **SSH 上直接下载安装**等更顺的方案，届时再更新对外文档。

## 若你已经在用手动包

发布 tarball 里自带 `README.txt`、`beetle.service` 示例。若必须手工部署，常见做法是：解压到 `/opt/beetle/releases/<version>/`，用符号链接 `current` 指向当前要跑的版本，状态目录由 `BEETLE_STATE_ROOT` 或默认路径决定（实现见 `src/platform/state_root.rs`）。细节以包内说明为准；**这不是**推荐给终端用户的最终流程。

## 与 CI

GitHub Release 上提供校验和与构建出处证明；设备上的安装与回滚由你在目标机上完成，不在 CI 里模拟。
