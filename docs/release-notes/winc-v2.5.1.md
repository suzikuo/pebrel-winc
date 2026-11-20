# Pebrel Winc 2.5.1

## English

### Added

- Added approved plugin shortcuts that open the focused Windows or WSL terminal's directory.
- Added named groups for organizing tabs in the left sidebar, with custom colors and drag-and-drop between groups.
- Added a Windows menu for launching, packaging, and publishing the fork.

### Fixed

- Fixed session restore when launching Debug or Release from the local menu.
- Fixed Release ZIP packaging when Cargo reuses the CLI after a GUI-only change.

### Improved

- Improved the directory launcher buttons by matching the size and spacing of the built-in toolbar icons.
- Improved installation isolation with a separate Windows installer identity and settings directory.
- Improved update delivery for the fork with a dedicated GitHub release feed. Installed copies can install updates in the app, and portable copies can download the updated ZIP.

## 简体中文

### 新增

- 新增经用户授权的插件快捷按钮，可打开当前 Windows 或 WSL 终端的工作目录。
- 新增左侧栏标签分组，支持自定义颜色和拖拽移动标签。
- 新增 Windows 菜单，可启动、打包和发布自编译版。

### 修复

- 修复从本地菜单启动 Debug 或 Release 版时跳过会话恢复的问题。
- 修复只修改界面时，Cargo 复用 CLI 导致 Release ZIP 打包误报的问题。

### 改进

- 目录启动按钮采用原生工具栏图标的尺寸和间距。
- 为自编译版使用独立的 Windows 安装标识和配置目录。
- 从自编译仓库的 GitHub Releases 检查更新。安装版可在应用内安装更新，便携版可下载新版 ZIP。

## Contributors

- [suzikuo](https://github.com/suzikuo)
