# ntools 剪贴板助手产品规划

## 1. 项目定位

`ntools` 的核心目标是做一个面向 macOS/Windows 的剪贴板效率工具。第一阶段完整实现类似 Maccy 的剪贴板历史能力：记录用户复制过的文本，提供快速搜索、选择、复制、粘贴、置顶、删除和清理。第二阶段在历史能力之上增加“复制内容解析”：当用户复制时间戳、日期、英文单词、URL、JSON、颜色值等文本时，自动识别类型并在面板中展示可直接使用的解析结果。

产品原则：

- 轻量：后台常驻但不打扰，复制行为不能被拦截或破坏。
- 键盘优先：历史面板、搜索、选择、复制、粘贴都能通过快捷键完成。
- 本地优先：历史数据默认只保存在本机。
- 可扩展：解析能力按插件式规则扩展，不把所有逻辑写死在剪贴板监听里。
- 可配置：复制监听、历史面板、粘贴历史、解析规则、隐私过滤都允许用户配置。

参考产品：

- Maccy 官方站点强调“只做一件事：把复制历史放在手边”，并突出轻量、键盘优先、本地存储和隐私。
- Maccy 默认通过 `Shift + Command + C` 打开历史面板，支持搜索、Enter 选择历史项、Option + Enter 选择并粘贴、Option + P 置顶、Option + Delete 删除、清理未置顶或全部历史。
- Maccy 默认会忽略临时、隐藏、自动生成等剪贴板类型，并提供忽略下一次复制、暂停记录、调整剪贴板检查间隔等高级能力。

资料来源：

- [Maccy 官方网站](https://maccy.app/)
- [Maccy GitHub README](https://github.com/p0deje/Maccy)

## 2. 总体范围

### 2.1 第一阶段：Maccy 风格剪贴板历史

目标是先把剪贴板历史做扎实，作为后续解析能力的底座。

功能：

- 后台监听复制内容。
- 保存文本历史到本地 JSON 或轻量数据库。
- 去重：连续相同内容不重复记录。
- 列表面板：展示最近复制内容。
- 搜索：按内容快速过滤历史。
- 选择历史项：复制回系统剪贴板。
- 粘贴历史项：复制到系统剪贴板后模拟粘贴。
- 置顶：常用内容固定在顶部。
- 删除单条历史。
- 清理所有未置顶历史。
- 清理全部历史。
- 暂停记录。
- 忽略下一次复制。
- 隐私过滤：默认忽略密码管理器、临时剪贴板、远程通用剪贴板等类型。

### 2.2 第二阶段：复制内容自动解析

当检测到新复制文本时，后台解析器自动识别类型，并把结果展示在面板中。

首批解析类型：

- 时间戳：秒级、毫秒级、微秒级时间戳。
- 日期时间：常见日期字符串统一格式化。
- 英文单词：展示中文翻译、音标、词性、例句。
- JSON：校验、格式化、压缩。
- URL：展示协议、域名、路径、查询参数。
- Base64：检测并解码。
- UUID：识别版本与格式。
- 颜色值：识别 HEX/RGB/HSL，并互相转换。

时间戳输出格式：

```text
YYYY-MM-DD HH:mm:ss
```

示例：

```text
复制：1719820800/1719820800000
展示：2024-07-01 16:00:00
类型：Unix 秒/毫秒级时间戳
```

### 2.3 第三阶段：自定义快捷键与用户配置

功能：

- 配置“打开历史面板”的快捷键。
- 配置“粘贴最近一条历史”的快捷键。
- 配置“复制监听”的触发方式。
- 配置历史最大条数。
- 配置历史保留天数。
- 配置是否开机启动。
- 配置是否记录图片、文件、富文本。
- 配置解析规则开关。
- 配置翻译服务来源。
- 配置隐私忽略规则。

## 3. 推荐目录结构

当前 `ntools` 是 Wails + Vue 项目。建议把后端能力按领域拆分，避免 `main.go` 继续变胖。

```text
ntools/
  main.go
  app/
    bootstrap.go              # 应用启动编排：窗口、服务、监听器、配置加载
    lifecycle.go              # 启动/退出/异常恢复
  internal/
    clipboard/
      monitor.go              # 剪贴板监听调度
      polling.go              # 无权限时的轮询 fallback
      hotkey_darwin.go        # macOS 全局快捷键监听
      hotkey_windows.go       # Windows 全局快捷键监听
      reader.go               # 系统剪贴板读取
      writer.go               # 系统剪贴板写入
      types.go                # ClipboardItem、ClipboardSource 等类型
    history/
      store.go                # 历史存储接口
      json_store.go           # JSON 存储实现
      sqlite_store.go         # 后续可选 SQLite 实现
      service.go              # 去重、置顶、删除、清理、查询
      types.go
    parser/
      service.go              # 解析调度器
      rule.go                 # ParserRule 接口
      timestamp.go            # 时间戳解析
      datetime.go             # 日期解析
      word.go                 # 单词解析
      json.go                 # JSON 解析
      url.go                  # URL 解析
      base64.go
      color.go
      uuid.go
      types.go
    hotkey/
      config.go               # 快捷键配置模型
      recorder.go             # 前端录制快捷键
      service.go              # 注册/更新/冲突处理
    config/
      config.go               # 用户配置结构
      store.go                # 配置读写
      defaults.go             # 默认值
    privacy/
      filter.go               # 忽略规则
      pasteboard_types.go     # 敏感剪贴板类型
  frontend/
    src/
      api/
        clipboard.ts          # 绑定后端服务
        config.ts
        parser.ts
      components/
        ClipboardPanel.vue    # 历史面板
        HistoryList.vue       # 历史列表
        HistoryItem.vue       # 单条历史
        ParserResult.vue      # 解析结果展示
        SettingsModal.vue     # 设置
        HotkeyRecorder.vue    # 快捷键录制
      stores/
        clipboard.ts
        settings.ts
      views/
        Home.vue
        Settings.vue
```

说明：

- `clipboard` 只负责监听和系统剪贴板读写。
- `history` 只负责历史数据管理。
- `parser` 只负责把文本转成结构化解析结果。
- `hotkey` 只负责快捷键配置、注册和冲突处理。
- `frontend` 不直接处理系统剪贴板，统一调用 Go 服务。

## 4. 数据模型规划

### 4.1 剪贴板历史项

```json
{
  "id": "uuid",
  "contentType": "text",
  "text": "1719820800",
  "preview": "1719820800",
  "hash": "sha256",
  "sourceApp": "Chrome",
  "sourceBundleId": "com.google.Chrome",
  "pasteboardTypes": ["public.utf8-plain-text"],
  "copiedAt": "2026-07-03T10:20:30Z",
  "lastUsedAt": "2026-07-03T10:25:30Z",
  "useCount": 3,
  "pinned": false,
  "ignored": false,
  "parserResults": []
}
```

### 4.2 解析结果

```json
{
  "type": "timestamp",
  "title": "Unix 时间戳",
  "confidence": 0.98,
  "summary": "2024-07-01 16:00:00",
  "actions": [
    {
      "label": "复制格式化时间",
      "value": "2024-07-01 16:00:00"
    }
  ],
  "details": {
    "unit": "seconds",
    "timezone": "Asia/Shanghai"
  }
}
```

### 4.3 用户配置

```json
{
  "history": {
    "maxItems": 500,
    "retentionDays": 30,
    "dedupe": true,
    "storePinnedForever": true
  },
  "hotkeys": {
    "openPanel": "CmdOrCtrl+Shift+C",
    "pasteLatest": "CmdOrCtrl+Shift+V",
    "copyShortcut": "CmdOrCtrl+C",
    "toggleRecording": ""
  },
  "clipboard": {
    "pollingIntervalMs": 500,
    "useGlobalHotkeyListener": true,
    "fallbackPolling": true
  },
  "privacy": {
    "ignoreConcealedTypes": true,
    "ignoreTransientTypes": true,
    "ignoreUniversalClipboard": true,
    "ignoredApps": [],
    "ignoredTypes": [
      "org.nspasteboard.TransientType",
      "org.nspasteboard.ConcealedType",
      "org.nspasteboard.AutoGeneratedType",
      "com.apple.is-remote-clipboard"
    ]
  },
  "parsers": {
    "timestamp": true,
    "datetime": true,
    "wordTranslation": true,
    "json": true,
    "url": true,
    "base64": true,
    "color": true,
    "uuid": true
  }
}
```

## 5. 核心交互设计

### 5.1 历史面板

行为参考 Maccy，但为解析能力增加右侧或下方详情区。

默认布局：

- 顶部搜索框。
- 左侧历史列表。
- 右侧解析结果面板。
- 底部状态栏：历史数量、暂停状态、设置入口。

键盘行为：

- 打开面板后自动聚焦搜索框。
- 输入即搜索。
- `Enter`：把选中项复制回剪贴板。
- `Option + Enter`：把选中项复制回剪贴板并粘贴到当前应用。
- `Option + Shift + Enter`：粘贴纯文本。
- `Option + P`：置顶或取消置顶。
- `Option + Delete`：删除选中项。
- `Escape`：关闭面板。
- `Cmd + ,`：打开设置。

### 5.2 解析结果面板

每条历史项可以有多个解析结果。展示顺序按置信度和常用程度排序。

示例：

复制 `1719820800000`：

- 主结果：`2024-07-01 16:00:00`
- 识别类型：Unix 毫秒时间戳
- 快捷操作：复制格式化时间、复制 ISO 时间、复制秒级时间戳

复制 `apple`：

- 主结果：`苹果；苹果公司`
- 识别类型：英文单词
- 快捷操作：复制翻译、收藏单词、打开详情

### 5.3 设置面板

设置分组：

- 快捷键：打开面板、粘贴最近历史、暂停记录、忽略下一次复制。
- 历史：最大条数、保留天数、是否去重、是否保留置顶。
- 解析：开启/关闭具体解析器。
- 翻译：本地词典、在线翻译、API Key。
- 隐私：忽略应用、忽略剪贴板类型、暂停记录。
- 高级：轮询间隔、调试日志、导入导出配置。

## 6. 历史剪贴板能力细节

### 6.1 记录策略

- 每次检测到新文本后计算 hash。
- 如果与最近一条 hash 相同，不重复记录，只更新 `copiedAt` 可选。
- 如果历史中已有相同 hash，移动到顶部并更新 `copiedAt`。
- 置顶项不参与普通排序下沉，始终在顶部。
- 普通项按 `copiedAt` 倒序。

### 6.2 搜索策略

第一版：

- 大小写不敏感。
- 支持按正文搜索。
- 支持按解析结果搜索，例如搜索 `2024-07-01` 能找到原始时间戳。

第二版：

- 拼音/模糊搜索。
- 搜索来源应用。
- 搜索类型，例如 `type:timestamp`、`type:url`。

### 6.3 粘贴策略

粘贴历史项时：

1. 保存当前系统剪贴板。
2. 写入选中历史项。
3. 模拟系统粘贴快捷键。
4. 可选：延迟恢复原剪贴板。

默认不恢复，因为 Maccy 类产品通常会把选中历史项留在剪贴板中，符合用户预期。

### 6.4 隐私策略

默认忽略：

- `org.nspasteboard.TransientType`
- `org.nspasteboard.ConcealedType`
- `org.nspasteboard.AutoGeneratedType`
- `com.apple.is-remote-clipboard`
- 常见密码管理器来源。

用户能力：

- 暂停记录。
- 忽略下一次复制。
- 忽略指定应用。
- 忽略指定剪贴板类型。
- 一键清空历史。

## 7. 解析能力规划

### 7.1 ParserRule 接口

```go
type ParserRule interface {
    Name() string
    Match(text string) bool
    Parse(text string, context ParseContext) ([]ParseResult, error)
}
```

要求：

- `Match` 必须便宜，不能访问网络。
- `Parse` 可以稍慢，但要设置超时。
- 在线翻译类解析必须异步，不阻塞历史记录。
- 每个解析结果必须包含可复制动作。

### 7.2 时间戳解析

识别：

- 10 位秒级时间戳。
- 13 位毫秒级时间戳。
- 16 位微秒级时间戳。
- 19 位纳秒级时间戳。

输出：

- 本地时间：`YYYY-MM-DD HH:mm:ss`
- UTC 时间。
- ISO 8601。
- 原始单位判断。

### 7.3 英文单词翻译

第一版建议先做本地/离线词典或可插拔接口，不强绑定外部服务。

能力：

- 单词检测。
- 基础中文释义。
- 音标。
- 词性。
- 常见短语。

后续：

- 接入在线翻译。
- 支持句子翻译。
- 支持用户配置翻译服务。

## 8. 开发里程碑

### M1：稳定剪贴板历史底座

- 重构当前监听代码到 `internal/clipboard`。
- 建立 `history` 服务。
- 保存 JSON 历史。
- 去重、查询、删除、清空。
- 前端展示历史列表。
- 支持搜索。

交付标准：

- 复制任意文本后，历史面板能看到记录。
- 搜索能过滤历史。
- 点击或回车能复制历史项。

### M2：Maccy 风格键盘操作

- 打开历史面板快捷键。
- 选择并复制。
- 选择并粘贴。
- 置顶。
- 删除。
- 清空。
- 暂停记录。
- 忽略下一次复制。

交付标准：

- 不依赖鼠标即可完成主要操作。
- 快捷键冲突时有清晰提示。

### M3：解析面板 MVP

- 时间戳解析。
- JSON 格式化。
- URL 解析。
- Base64 解码。
- 解析结果展示。
- 解析结果一键复制。

交付标准：

- 复制时间戳后自动展示 `YYYY-MM-DD HH:mm:ss`。
- 复制 JSON 后自动展示格式化结果。

### M4：英文单词翻译

- 单词识别。
- 本地词典或翻译接口。
- 翻译结果展示。
- 翻译结果复制。

交付标准：

- 复制英文单词后自动展示中文释义。

### M5：设置与持久化完善

- 自定义打开面板快捷键。
- 自定义粘贴历史快捷键。
- 历史上限与保留天数。
- 解析器开关。
- 隐私忽略规则。
- 配置导入导出。

交付标准：

- 用户可以不改代码完成主要行为配置。

## 9. 技术风险

### 9.1 macOS 辅助功能权限

全局按键监听和自动粘贴都依赖辅助功能权限。开发模式下 `wails dev` 实际运行的是 `.dev.app`，用户必须给真实 app 授权。

缓解：

- 启动时检测权限。
- 权限不足时自动使用剪贴板轮询。
- UI 显示权限状态和修复指引。
- 写入 `debug.log` 方便定位问题。

### 9.2 翻译服务不稳定

在线翻译可能受网络、额度、API Key 影响。

缓解：

- 第一版用可插拔接口。
- UI 显示翻译失败原因。
- 允许关闭翻译解析器。

### 9.3 历史文件膨胀

长期保存 JSON 可能变大，读写性能下降。

缓解：

- 第一版限制最大条数。
- 第二版切换到 SQLite。
- 置顶项单独保留。

### 9.4 隐私风险

剪贴板可能包含密码、Token、密钥。

缓解：

- 默认忽略敏感 pasteboard type。
- 支持暂停记录和忽略下一次复制。
- 支持应用黑名单。
- 设置页明确展示本地存储位置。

## 10. 推荐下一步

先做 M1，不要一开始同时做翻译和复杂设置。建议下一份实施计划围绕“稳定剪贴板历史底座”展开：

1. 重构 Go 后端目录。
2. 把当前 JSON 历史写入升级为 `history.Service`。
3. 暴露 Wails 服务给前端查询历史。
4. 前端做历史列表和搜索。
5. 保留当前轮询 fallback，确保没有权限时也可用。

M1 完成后，再进入 M2 的快捷键和粘贴历史能力。
