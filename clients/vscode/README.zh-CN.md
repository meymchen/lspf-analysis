# LSPF Analysis for VS Code

[English][readme-en] · [简体中文][readme-zh]

在编辑代码时找出复杂函数，查看当前文件的代码健康度。
LSPF Analysis 提供悬停详情、问题诊断，以及按质量排序的函数列表。

支持 **C++**、**Java**、**JavaScript**、**Python**、**Rust**、
**TypeScript** 和 **TSX**。

扩展内置 [lspf-analysis][project] 语言服务器，使用时无需安装 Rust
或配置其他 LSP 客户端。界面、悬停内容和诊断信息支持英文与简体中文，
随 VS Code 的显示语言切换。

## 开始使用

1. 在 VS Code 1.106.0 或更新版本中安装扩展。
2. 打开受支持的源文件，扩展会自动开始分析。
3. 将鼠标悬停在函数名称上，查看度量值和评分。
4. 展开资源管理器中的“代码健康度”，查看当前文件的函数列表。
   默认按得分从低到高排序，点击函数即可跳转到定义。

低于警告阈值的函数也会出现在“问题”面板中。
将鼠标悬停在状态栏的代码健康度项目上，可查看文件概况，
并跳转到得分最低的函数。

## 可以查看哪些信息

- 函数悬停详情展示度量值与评分依据。
- “代码健康度”树按评分支柱组织度量值，工具栏可切换质量排序与源码顺序。
- 状态栏展示当前文件的综合评分与等级分布。
- 诊断标出低于所设阈值的函数。Java 类和接口也有类健康度评分与诊断。

函数健康度由四个支柱组成。每个支柱取其度量中的最低得分，
再将各支柱合成为综合评分。

| 支柱 | 度量 |
| --- | --- |
| 控制流 | 认知复杂度、圈复杂度 |
| 规模 | 语句数，即逻辑行数 |
| 词汇负担 | 工作记忆、Halstead 难度 |
| 接口 | 参数个数 |

评分采用 0—100 分制，分数越高，代码健康度越好。
等级分别为 **A**（至少 80 分）、**B**（至少 50 分且低于 80 分）、
**C**（至少 25 分且低于 50 分）和 **D**（低于 25 分）。

Java 类健康度使用类加权方法数、公开方法数和公开属性数。
类评分阈值来自 Java 研究，目前 C++ 类度量不参与健康度评分。
资源管理器中的树展示函数，类的详情通过悬停查看。

这些评分可用于安排代码审查和重构。
[评分公式、研究依据与覆盖范围][scoring]说明了度量如何组合，
以及各语言构造的分析限制。

## 设置

在设置中搜索 `lspfAnalysis`，或点击状态栏悬停内容中的设置链接。
评分与诊断设置无需重启即可生效。

| 设置 | 默认值 | 含义 |
| --- | --- | --- |
| `lspfAnalysis.health.complexityThreshold` | 15 | 认知复杂度达到此值时得 50 分 |
| `lspfAnalysis.health.cyclomaticThreshold` | 10 | 圈复杂度达到此值时得 50 分 |
| `lspfAnalysis.health.lengthThreshold` | 30 | 语句数达到此值时得 50 分 |
| `lspfAnalysis.health.workingMemoryThreshold` | 8 | 同时持有的名称数达到此值时得 50 分 |
| `lspfAnalysis.health.halsteadDifficultyThreshold` | 12 | Halstead 难度达到此值时得 50 分 |
| `lspfAnalysis.health.parametersThreshold` | 4 | 参数个数达到此值时得 50 分 |
| `lspfAnalysis.health.wmcThreshold` | 34 | Java 类加权方法数达到此值时得 50 分 |
| `lspfAnalysis.health.publicMethodsThreshold` | 14 | Java 类公开方法数达到此值时得 50 分 |
| `lspfAnalysis.health.publicAttributesThreshold` | 8 | Java 类公开属性数达到此值时得 50 分 |
| `lspfAnalysis.health.qualityWarn` | 25 | 低于此分数时报告警告 |
| `lspfAnalysis.health.qualityError` | 10 | 低于此分数时报告错误 |
| `lspfAnalysis.diagnostics.enabled` | `true` | 显示诊断 |
| `lspfAnalysis.diagnostics.perMetric` | `false` | 综合评分达标但某个支柱得分偏低时，也给出提示 |
| `lspfAnalysis.diagnostics.file` | `false` | 文件综合评分低于错误阈值时报告诊断 |
| `lspfAnalysis.statusBar.enabled` | `true` | 在状态栏显示当前文件的代码健康度 |

`lspfAnalysis.health.weights.*` 控制相对权重。
控制流、规模和词汇负担的默认权重各为 `1`，接口和类设计各为 `0.5`。
类设计权重控制 Java 类对文件评分的贡献。

排查问题时，可通过 `lspfAnalysis.trace.server` 开启协议跟踪。
`lspfAnalysis.server.path` 用于指定自定义服务器程序，留空则使用内置程序。
修改此路径后，请重新加载窗口。

## 命令

在命令面板中搜索 `LSPF Analysis`：

- **重启服务器**：重新启动语言服务器。
- **按质量排序**：优先列出得分最低的函数。
- **按位置排序**：按源码顺序列出函数。

“代码健康度”工具栏中也有排序按钮。
树中的函数与状态栏悬停内容中的函数链接均可直接跳转到对应行。

## 平台支持

打包目标包括 Windows、macOS、Linux 和 Alpine Linux，各自提供 x64 与 ARM64 版本。
手动安装时，应选择与扩展运行环境的操作系统和架构对应的 VSIX。
Linux 版语言服务器静态链接 musl，不依赖发行版的 glibc 版本。
扩展依赖本地语言服务器，无法直接在 vscode.dev 的浏览器环境中运行。

## 获取帮助

请通过 [GitHub Issues][issues] 报告问题或提出功能建议。
报告问题时，附上扩展版本、VS Code 版本、操作系统与架构，
以及能够复现问题的简短源码。
启动失败时，可在“查看 → 输出 → LSPF Analysis”中检查日志并提供相关行。
分享日志前，请移除私有源码与敏感路径。

版本变化见[更新记录][changelog]。

## 自行构建

安装 Rust 和 C 工具链后，在 `clients/vscode` 目录执行：

```console
npm ci
npm run package
```

也可以指定目标平台：

```console
npm run package -- --target darwin-arm64
```

可用目标包括 `win32-x64`、`win32-arm64`、`darwin-x64`、`darwin-arm64`、
`linux-x64`、`linux-arm64`、`alpine-x64` 和 `alpine-arm64`。
跨平台编译需要对应的 Rust 目标，以及该目标的 C 工具链和链接器，
因为 tree-sitter 语法解析器使用 C。
Linux 与 Alpine 目标使用 `musl-gcc` 构建，在 Debian 和 Ubuntu 上由 `musl-tools` 包提供。
构建完成后，通过“扩展 → … → 从 VSIX 安装”选择生成的文件。

预览包使用 `npm run package:pre-release` 构建。
预发布上传步骤与版本管理方式见[发布指南][publishing]。

## 许可证

[MPL-2.0][license]。

[readme-en]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/README.md
[readme-zh]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/README.zh-CN.md
[project]: https://github.com/meymchen/lspf-analysis
[scoring]: https://github.com/meymchen/lspf-analysis/blob/main/README.zh-CN.md#分析结果
[issues]: https://github.com/meymchen/lspf-analysis/issues
[changelog]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/CHANGELOG.md
[license]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/LICENSE
[publishing]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/PUBLISHING.md
