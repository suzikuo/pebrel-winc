# Pebrel Winc 2.5.3

## English

### Added

- Added five independent sidebar spacing settings under **Settings → Appearance → Sidebar tab spacing**. Each gap can use its default or a value from 0 to 48 px.
- Added an optional Scratch Terminal plugin. Press `Ctrl+Alt+J` to open the panel and run multiple independent shells per tab. Dock the panel or float it, drag and resize it while it is visible, adjust its font size independently, and close one or all scratch shells.
- Added reviewed workspace plugin contributions for sidebar styling and tab context-menu commands.

### Fixed

- Fixed close confirmations so moving a tab or split pane while its dialog is open keeps the confirmation tied to the original target.
- Fixed automatic Codex recovery for saved tabs without a conversation ID. These tabs now open the native conversation picker.

### Improved

- Improved sidebar groups with taller headings, shorter tab rows, and group actions in the context menu. The new-terminal button is hidden while a group is collapsed.
- Improved Appearance settings with expandable navigation and added a glass effect to the sidebar when a dark theme extends its wallpaper into the window chrome.

To enable Scratch Terminal, install the [example plugin folder](https://github.com/suzikuo/pebrel-winc/tree/winc-v2.5.3/pebrel_app/examples/plugins/scratch-terminal) through **Settings → Plugins → Install from folder** and approve workspace access.

## 简体中文

### 新增

- **设置 → 外观 → 侧栏标签间距**新增五项独立设置。每项可沿用默认值，也可设为 0～48 px。
- 新增可选的临时终端插件。按 `Ctrl+Alt+J` 打开，可在每个标签中运行多个独立 Shell。面板支持停靠或浮动、实时拖动和缩放、独立字号，以及关闭单个或全部临时 Shell。
- 新增经用户授权的工作区插件功能，可定制侧栏样式，并向标签右键菜单添加命令。

### 修复

- 关闭确认框打开后，即使移动标签或分屏，确认操作也会作用于原来的目标。
- 修复 Codex 自动恢复时缺少对话 ID 的问题。对应标签现在会打开原生会话选择器。

### 改进

- 加高侧栏分组标题，缩短标签行，将分组操作放入右键菜单，并在组折叠时隐藏新建终端按钮。
- 外观设置使用可展开的导航。深色主题将壁纸延伸到窗口边栏时，侧栏会显示玻璃效果。

要启用临时终端，请打开 **设置 → 插件 → 从文件夹安装**，选中[示例插件目录](https://github.com/suzikuo/pebrel-winc/tree/winc-v2.5.3/pebrel_app/examples/plugins/scratch-terminal)，并批准工作区访问权限。

## Contributors

- [suzikuo](https://github.com/suzikuo)
- The upstream Pebrel developers and contributors / Pebrel 上游开发者与贡献者 · [Upstream commits / 上游提交](https://github.com/Kuddev/pebrel/compare/f939fddc...76f74be1)
