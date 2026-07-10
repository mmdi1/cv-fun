# FunCV

**本地优先的智能剪贴板** — 系统级历史、内容推理、可扩展插件。

| 项 | 说明 |
| --- | --- |
| 产品名 | **FunCV** |
| 官网 | [cv.nfun.org](https://cv.nfun.org) |
| 工程名 | `nfun-cv` |
| 技术栈 | Tauri 2 · Rust · Vue 3 · TypeScript |
| 平台 | macOS（托盘 / 全局快捷键 / Dock） |

---

## 产品截图

| 主界面 · 历史与预览 | 插件 · 列表 |
| --- | --- |
| ![主界面](img/screenshot-main.png) | ![插件列表](img/screenshot-plugins-list.png) |

| 插件 · 自定义与接口 | 设置 · 推荐解析 |
| --- | --- |
| ![插件自定义](img/screenshot-plugins-custom.png) | ![设置与推荐](img/screenshot-settings.png) |

更多介绍与下载见官网：**[cv.nfun.org](https://cv.nfun.org)**

---

## 为什么用 FunCV

复制之后经常要：找历史、格式化 JSON、压一行 SQL、看时间戳、查个英文单词……  
FunCV 在本地完成监听、存储与推理，**默认不上传云端**，并通过插件协议支持 Node / Python / Go / Shell 扩展。

---

## 功能一览

### 剪贴板历史

- **系统剪贴板监听**：轮询整板内容（非键盘 hook），支持 `Ctrl/Cmd+C`、右键复制等
- **文本 + 图片**：文本入库；图片存 PNG，可预览并写回剪贴板
- **历史管理**：搜索、选中预览、双击复制、删除、清空
- **hash 去重**：相同内容合并更新，避免刷屏

### 智能推荐解析

右侧**默认显示原文**；底部「推荐解析」展示规则与预览，**单击应用 / 双击复制**：

| 类型 | 能力 |
| --- | --- |
| JSON | 单行格式化 / 多行压缩 / 清洗转义后格式化 |
| SQL | 格式化（关键字大写 + 缩进）/ 压缩为一行 |
| 时间戳 | Unix 秒/毫秒 ↔ 可读时间 |
| URL / UUID / 颜色 / Base64 | 识别与规范化 |
| 插件结果 | 启用插件后自动参与推荐 |

### 通用插件系统

- 顶部 **插件** 面板，**列表 / 自定义** 双 Tab
- **列表**：已安装插件开关、上传、删除；ECDICT 词典管理
- **自定义**：完整接口规范 + 下载 Node / Python / Go / Shell 示例
- **统一协议**（仅 2 个入参）：

```json
{ "content": "剪贴板内容", "type": "text" }
```

`type` 为 `"text"` | `"img"`（可扩展）。插件从 **stdin** 读 JSON，向 **stdout** 写结果。

### 内置 · 英汉互译

- 数据源：[skywind3000/ECDICT](https://github.com/skywind3000/ECDICT)
- 本地下载导入 SQLite，**离线查词**
- 英文 → 中文释义；中文 → 反查英文词条
- 结果进入推荐解析卡片

### 桌面体验

- **无边框**主窗口，关闭进托盘（可隐藏 Dock）
- **全局快捷键**唤起 / 隐藏；未置顶时按快捷键会**置顶**而非误关
- 设置：历史条数、轮询间隔、快捷键录制

---

## 存储

```text
~/Library/Application Support/NfunCv/   # macOS
├── config.json
├── history.db              # SQLite + WAL
├── images/<id>.png
└── plugins/
    ├── registry.json
    ├── user/<plugin-id>/   # 用户上传插件
    └── data/ecdict/        # ECDICT SQLite
```

全部默认本地；词典下载为可选联网行为。

---

## 快速开始

```bash
cd nfun-cv
npm install
npm run tauri dev
```

构建：

```bash
npm run build
npm run tauri build
```

### 使用提示

1. 任意复制文本或图片 → 左侧出现历史  
2. 点选记录 → 右侧看原文；底部点推荐解析  
3. **插件** → 下载 ECDICT → 复制英文单词即可英汉互译  
4. **插件 → 自定义** 下载示例 → 改脚本 → **列表 → 上传**  
5. 关闭窗口仍由托盘 / 快捷键控制  

---

## 插件协议（摘要）

| 方向 | 格式 |
| --- | --- |
| 入参 | `{ "content": string, "type": "text" \| "img" }` |
| 出参 | `{ "ok": true, "title", "body", "preview", "hint" }` |
| 失败 | `{ "ok": false, "error": "..." }` |
| 清单 | `plugin.json`：`id` / `name` / `runtime` / `entry` / `types` |

运行时：`node` · `python` · `go` · `shell` · `builtin`

示例目录：[`plugin-examples/`](./plugin-examples/)

---

## 隐私

- 历史与图片默认仅存本机  
- 不依赖键盘监听，只读系统剪贴板  
- ECDICT 等资源下载为用户主动触发  

---

## 链接

- 官网：[cv.nfun.org](https://cv.nfun.org)  
- 词典数据：[ECDICT](https://github.com/skywind3000/ECDICT)  

---

## License

以仓库内声明为准。
