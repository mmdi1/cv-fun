# FunCV 插件示例

通用接口（**仅 2 个入参**）：

```json
{ "content": "剪贴板内容或图片相对路径", "type": "text" }
```

`type` 取值：`text` | `img`（可扩展）。

## 输出

```json
{
  "ok": true,
  "title": "显示名",
  "body": "完整结果（点击推荐后写入主面板）",
  "preview": "一行预览",
  "hint": "可选"
}
```

失败：`{ "ok": false, "error": "原因" }`

## plugin.json

| 字段 | 说明 |
|------|------|
| `id` | 唯一 id |
| `name` | 展示名 |
| `runtime` | `node` / `python` / `go` / `shell` |
| `entry` | 入口文件 |
| `types` | 接受的 type 列表 |
| `description` | 说明 |

## 运行时

| runtime | 命令 |
|---------|------|
| node | `node entry` |
| python | `python3 entry` |
| go | `go run entry.go` 或可执行文件 |
| shell | `sh entry` |

主机通过 **stdin** 写入 JSON，从 **stdout** 读取最后一行 JSON。

## 获取示例

应用内 **插件 → 自定义** 可：

- 查看完整接口规范
- 按 runtime 下载 Node / Python / Go / Shell 示例
- 或「下载全部示例」

也可直接使用本仓库 `plugin-examples/` 目录。

## 上传

应用内 **插件 → 列表 → 上传插件**，选择含 `plugin.json` 的目录。

## 内置

- **英汉互译** (`translate-en-zh`)：本地 [ECDICT](https://github.com/skywind3000/ECDICT)，需先在插件面板下载词典。
