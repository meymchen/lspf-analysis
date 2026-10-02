# LSPF Analysis for Zed

[English](./README.md)

在编辑时查看函数的代码健康度诊断、评分和度量值。
支持 C++、Java、JavaScript（含 JSX）、Python、Rust、TypeScript 和 TSX，
以及 Java 类健康度。可与已有语言服务器共同使用，保留补全和代码导航。

首版使用标准 LSP 诊断和 Markdown 悬停，不提供其他客户端的文件评分状态栏或函数树。
等级字母无需 HTML 颜色也可阅读。SSH 和 Dev Container 留待后续专项验证。

## 本地安装

扩展已准备上架材料。在固定版本的服务器发布之前，先从仓库根目录构建服务器：

```console
cargo build --release --package lspf-analysis --locked
```

在 Zed 的扩展页面选择 **Install Dev Extension**，选中 `clients/zed` 目录。
开发安装需要通过 rustup 安装 Rust；Zed 会构建 `wasm32-wasip2` 扩展。

将以下配置合并到 Zed 用户设置中，路径替换为本机可执行文件的绝对路径。
Windows 使用 `.exe` 后缀，macOS 和 Linux 不带后缀。

```json
{
  "lsp": {
    "lspf-analysis": {
      "binary": {
        "path": "C:/path/to/lspf-analysis/target/release/lspf-analysis.exe",
        "arguments": ["serve", "--stdio"]
      }
    }
  }
}
```

指定 `binary.path` 时必须同时填写 `arguments`，因为 Zed 可能直接使用用户命令，
跳过扩展的启动命令生成逻辑。修改启动配置后重启语言服务器。

按 Zed 提示信任项目后，语言服务器才能运行。
打开受支持的源文件，悬停在函数名上查看评分，低分函数会出现在诊断中。
如果 Zed 尚未识别 Java 文件，先安装提供 Java 语言定义的扩展。

若已自定义某种语言的服务器列表，将 `lspf-analysis` 加入原列表，例如：

```json
{
  "languages": {
    "Rust": {
      "language_servers": ["rust-analyzer", "lspf-analysis", "..."]
    }
  }
}
```

`"..."` 会启用其余已注册的服务器，请保留原有的 `"!服务器名称"` 禁用项。
Zed 语言名称为 `C++`、`Java`、`JavaScript`、`Python`、`Rust`、`TypeScript`、`TSX`。

## 下载与离线使用

未配置本地路径时，扩展从本仓库的 `server-v0.1.0` Release 下载服务器，
版本由 [`server-version`](./server-version) 固定。
三种系统均覆盖 x64 和 ARM64，Linux 使用 musl 构建。
扩展优先复用同版本缓存，只有下载完整后才将临时文件转为可执行缓存。
它不会自动追随仓库最新 Release，也不会自动选择 `PATH` 中的程序。

首次下载需要访问 GitHub。离线且无缓存、发行版本不存在或下载权限受限时，
启动错误会提示配置本地程序。通过 **zed: open log** 可查看日志。
已有完整缓存时，启动无需联网。

## 配置

评分设置统一放在 `lsp.lspf-analysis.settings.lspfAnalysis` 下。
用户设置全局生效，项目的 `.zed/settings.json` 可覆盖它。
不要把分析设置分散到 `initialization_options` 和 `settings` 两处。

例如，使用简体中文，并在评分低于 50 时发出警告：

```json
{
  "lsp": {
    "lspf-analysis": {
      "settings": {
        "lspfAnalysis": {
          "locale": "zh-cn",
          "health": { "qualityWarn": 50 }
        }
      }
    }
  }
}
```

默认输出英文，语言不会自动跟随编辑器。
评分与诊断配置更新后，服务器会重新分析已打开的文档，无需重启。
删除覆盖项即可恢复服务端默认值。字段类型错误会写入日志，并保留上一份有效配置。

[`settings.example.json`](./settings.example.json) 提供当前完整分析配置及默认值。
通常只需填写需要改变的项目。`diagnostics.enabled` 控制诊断开关，
`perMetric` 开启单项指标提示，`file` 开启文件级诊断。

构建、自动验证命令见[英文说明](./README.md#build-and-verify)。
扩展代码使用 MIT，服务器继续使用 MPL-2.0。
发布流程和人工验收清单见 [PUBLISHING.md](./PUBLISHING.md)。
