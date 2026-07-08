# ntools

本地优先的桌面剪贴板历史与内容分析工具。`ntools` 记录你复制过的文本，提供搜索、预览、复制回剪贴板和基础内容解析能力，目标是在 macOS 和 Windows 上提供轻量、键盘友好、尊重隐私的剪贴板效率体验。

Local-first desktop clipboard history and content analysis. `ntools` records copied text, lets you search and reuse clipboard history, and provides lightweight local analysis for common text formats. The goal is a fast, keyboard-friendly, privacy-conscious clipboard utility for macOS and Windows.

> 当前项目处于早期开发阶段，功能会持续迭代。README 中的“已实现”和“路线图”会尽量保持清晰边界。
>
> This project is in early development. The sections below separate implemented features from planned work as clearly as possible.

## 功能亮点 / Highlights

- 剪贴板历史：后台监听复制内容，保存文本历史到本地。
- Clipboard history: monitors copied text in the background and stores text history locally.
- 快速搜索：按复制内容过滤历史记录。
- Fast search: filter history items by copied content.
- 历史复用：选中历史项后可复制回系统剪贴板。
- Reuse history: copy any saved item back to the system clipboard.
- 基础管理：支持删除单条记录、清空历史记录。
- Basic management: delete individual items or clear all history.
- 内容分析：识别时间戳、JSON、URL、英文单词，并展示文本统计。
- Content analysis: detects timestamps, JSON, URLs, English words, and text statistics.
- 本地设置：支持保存快捷键、历史条数、解析规则和翻译源配置。
- Local settings: stores hotkey, history limit, parser, and translation provider preferences.
- 系统托盘：关闭窗口后常驻后台，可从托盘重新打开。
- System tray: stays available in the background and can be reopened from the tray.

## 当前状态 / Current Status

已实现 / Implemented:

- Go + Wails v3 桌面应用骨架。
- Go + Wails v3 desktop application shell.
- Vue 3 + TypeScript 前端界面。
- Vue 3 + TypeScript frontend.
- macOS / Windows 复制快捷键监听入口，并提供轮询 fallback。
- macOS / Windows copy shortcut listener entry points with polling fallback.
- 本地 JSON 历史存储、去重、最大条数限制和时间排序。
- Local JSON history storage, deduplication, max item limit, and time-based sorting.
- 历史列表、搜索、详情预览、复制、删除、清空。
- History list, search, detail preview, copy, delete, and clear actions.
- 设置面板与本地配置文件读写。
- Settings panel with local config read/write.
- 时间戳、JSON、URL、英文单词、文本统计的前端本地分析。
- Frontend local analysis for timestamps, JSON, URLs, English words, and text statistics.

开发中 / In progress:

- 自定义快捷键保存后动态注册。
- Dynamic registration after saving custom hotkeys.
- 粘贴最近历史、选中并粘贴等键盘优先工作流。
- Keyboard-first workflows such as paste latest and select-and-paste.
- 置顶历史、清理未置顶历史、忽略下一次复制。
- Pinning items, clearing unpinned items, and ignoring the next copy.
- 更多解析规则：日期、Base64、颜色值、UUID、翻译源等。
- More parsers: datetime, Base64, color values, UUID, translation providers, and more.
- 隐私过滤：密码管理器、临时剪贴板、远程通用剪贴板等忽略规则。
- Privacy filters for password managers, transient clipboard types, universal clipboard, and similar sources.
- 正式发布包、签名、公证和安装器流程。
- Release packaging, signing, notarization, and installers.

## 截图 / Screenshots

截图暂未提交。欢迎在发布第一个预览版本后补充主界面、设置面板和内容分析示例。

Screenshots are not committed yet. Good future additions include the main panel, settings panel, and content analysis examples.

## 技术栈 / Tech Stack

- Backend: Go, Wails v3
- Frontend: Vue 3, TypeScript, Vite
- Desktop: WebView-based Wails application
- Storage: Local JSON files under the user config directory
- Build: Wails task system, platform-specific Taskfiles

## 快速开始 / Quick Start

### 环境要求 / Requirements

- Go 1.25 或更高版本 / Go 1.25 or newer
- Node.js 与 npm / Node.js and npm
- Wails v3 CLI / Wails v3 CLI
- Task runner support from Wails tasks / Wails task runner support

安装 Wails v3 CLI 的方式可能会随 Wails alpha 版本变化，请参考 Wails v3 官方文档。

Wails v3 is still alpha, so CLI installation details may change. Please refer to the official Wails v3 documentation.

### 安装依赖 / Install Dependencies

```bash
cd frontend
npm install
cd ..
```

### 开发运行 / Run In Development

```bash
wails3 task dev
```

等价的底层命令：

Equivalent underlying command:

```bash
wails3 dev -config ./build/config.yml
```

默认 Vite 端口为 `9245`，可通过 `WAILS_VITE_PORT` 覆盖。

The default Vite port is `9245`; override it with `WAILS_VITE_PORT`.

### 构建 / Build

```bash
wails3 task build
```

### 打包 / Package

```bash
wails3 task package
```

macOS 会生成 `.app` bundle；Windows 任务支持 NSIS / MSIX 打包配置。

On macOS this creates an `.app` bundle. Windows tasks include NSIS / MSIX packaging configuration.

### 测试 / Test

后端测试：

Backend tests:

```bash
go test ./...
```

前端单元测试：

Frontend unit tests:

```bash
cd frontend
npm run test:unit
```

前端构建检查：

Frontend build check:

```bash
cd frontend
npm run build
```

## 使用方式 / Usage

1. 启动 `ntools`。
2. 正常复制文本。
3. 在主窗口中搜索、查看和复用剪贴板历史。
4. 选中历史项查看详情和本地分析结果。
5. 需要时打开设置面板调整历史数量、快捷键和解析规则。

1. Start `ntools`.
2. Copy text as usual.
3. Search, inspect, and reuse clipboard history in the main window.
4. Select an item to view its details and local analysis results.
5. Open settings when you need to adjust history limits, hotkeys, or parser preferences.

## 数据与隐私 / Data And Privacy

`ntools` 的当前实现默认将历史和配置保存在本机，不依赖云端服务。

The current implementation stores history and settings locally by default and does not depend on cloud services.

常见路径由系统用户目录决定：

Common paths are derived from the current user's system directories:

- 配置文件 / Config file: `ntools/config.json`
- 历史文件 / History file: `ntools/clipboard-history.json`

实际完整路径由 Go 的 `os.UserConfigDir` 决定，不同平台会不同。

The full absolute paths are resolved by Go's `os.UserConfigDir`, so they differ by platform.

注意：早期版本仍在完善隐私过滤规则。请不要在不可信环境中依赖它过滤密码或敏感内容。

Note: privacy filters are still being built. Do not rely on early versions to filter passwords or sensitive content in untrusted environments.

## 项目结构 / Project Structure

```text
.
├── main.go                         # Wails application entry point
├── clipboard_service.go            # Clipboard history service exposed to frontend
├── config_service.go               # Local settings service
├── internal/
│   ├── clipboard/                  # Platform clipboard listeners, readers, polling
│   └── history/                    # History model, storage, sorting, service logic
├── frontend/
│   ├── src/
│   │   ├── features/clipboard/     # Clipboard panel, list, analysis, selection logic
│   │   └── features/settings/      # Settings panel and hotkey recorder
│   └── package.json
├── build/                          # Wails build config and platform Taskfiles
├── tools/                          # Icon generation helpers
└── docs/                           # Product notes and design documents
```

## 贡献 / Contributing

欢迎 issue、讨论和 PR。比较适合参与的方向：

Issues, discussions, and pull requests are welcome. Good contribution areas include:

- 完善跨平台剪贴板监听和权限处理。
- Improve cross-platform clipboard monitoring and permission handling.
- 实现更多内容解析规则。
- Add more content parsers.
- 改进键盘工作流和快捷键注册。
- Improve keyboard workflows and hotkey registration.
- 增加隐私过滤和敏感来源忽略策略。
- Add privacy filters and sensitive-source ignore rules.
- 补充发布包、安装器、签名和 CI。
- Add release packaging, installers, signing, and CI.
- 改进测试覆盖、文档和截图。
- Improve test coverage, documentation, and screenshots.

提交 PR 前建议至少运行：

Before opening a pull request, please run at least:

```bash
go test ./...
cd frontend
npm run test:unit
npm run build
```

## 路线图 / Roadmap

- [ ] 自定义快捷键动态生效 / Make custom hotkeys take effect dynamically
- [ ] 粘贴最近历史 / Paste latest history item
- [ ] 历史置顶 / Pin history items
- [ ] 清理未置顶历史 / Clear unpinned history
- [ ] 忽略下一次复制 / Ignore next copy
- [ ] 日期、Base64、颜色值、UUID 解析 / Datetime, Base64, color, and UUID parsers
- [ ] 本地或可配置翻译源 / Local or configurable translation provider
- [ ] 隐私忽略规则 / Privacy ignore rules
- [ ] macOS / Windows 预览版发布 / macOS / Windows preview releases
- [ ] CI 测试与构建 / CI tests and builds

## License

当前仓库尚未包含 LICENSE 文件。开源发布前建议补充明确许可证，例如 MIT、Apache-2.0 或 GPL 系列许可证。

This repository does not include a LICENSE file yet. Add an explicit license before public release, such as MIT, Apache-2.0, or a GPL-family license.
