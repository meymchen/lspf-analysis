# lspf-analysis

[English](./README.md) · [简体中文](./README.zh-CN.md)

**lspf-analysis** 是一个语言服务器，在你编写代码时指出哪些函数正在变得难以维护。

它使用基于 tree-sitter 的引擎计算代码指标，再将这些指标转为代码健康度评分，
通过 [lspf](https://github.com/meymchen/lspf) 发布为 LSP 诊断。
指标引擎派生自 [rust-code-analysis](https://github.com/mozilla/rust-code-analysis)。

支持的语言：**C++**、**Java**、**JavaScript**、**Python**、**Rust**、
**TypeScript**、**TSX**。

## 分析结果

将鼠标悬停在函数名上即可查看指标；质量低于阈值的函数会在名称下方显示诊断。
悬停提示只在函数名处响应，补充编辑器已有的符号信息，不会取代它。
需要自行展示列表的客户端可以请求[逐函数健康度明细](#逐函数健康度明细)，
无需逐个悬停查看。VS Code 扩展会将明细显示在资源管理器的树视图中。

对于引擎支持类级指标的语言（目前为 Java），类和接口也采用相同的报告方式，
在声明行和名称处展示结果。

函数质量由四个支柱综合得出。每个支柱取所属指标中的**最低分**，而非平均分，
因此函数无法用某项好指标掩盖另一项差指标，同一种属性也不会重复计分。

| 支柱 | 指标 | 默认阈值 | 依据 |
| --- | --- | ---: | --- |
| **控制流** | 认知复杂度 | 15 | [Campbell 2018][c18]；[Muñoz Barón 等，2020][mb20] 通过实测可理解性进行验证 |
| | 圈复杂度 | 10 | [McCabe 1976][mc76]；[Landman 等，2016][la16] 表明，在方法层面它与规模并不冗余 |
| **规模** | 语句数（逻辑行） | 30 | SIG 模型中的单元规模，见 [Heitlager 等，2007][hkv07]、[SIG/TÜViT][sig] |
| **词汇负担** | 同时持有的名称数 | 8 | [Miller 1956][mi56]、[Cowan 2001][co01] |
| | Halstead 难度 | 12 | 词汇规模会增加工作记忆负担，见 [Peitek 等，2021][pe21] |
| **接口** | 参数个数 | 4 | SIG 模型中的单元接口；在 [Topuz 2022][to22] 中，这是与缺陷相关性最高的代码异味 |

类设计使用独立的评分支柱。现有阈值来自 Java 代码语料，因此仅对 Java 类评分。
C++ 类指标会被计算和报告，但在建立适用的阈值之前，不影响健康度评分。
没有类指标的其他语言不参与这一支柱的评分。

| 支柱 | 指标 | 默认阈值 | 依据 |
| --- | --- | ---: | --- |
| **类设计** | 类的加权方法数 | 34 | [Filó 等，2015][fi15] 以 Qualitas.class Corpus 中的 111 个 Java 系统为基准，这是指标开始变得“不常见”的边界 |
| | 公有方法数 | 14 | 对同一阈值目录中每类方法数边界的保守解读；另见 [Ferreira 等，2012][fe12] |
| | 公有属性数 | 8 | 同上，采用每类字段数的边界 |

### 评分公式

先对每项指标单独评分，再进行综合，避免难以理解的函数仅凭代码短就获得高分。
对于原始值 $r \ge 0$ 和阈值 $t > 0$，得分为：

```math
s(r) = \frac{100}{1 + \left(\dfrac{r}{t}\right)^{2}}
```

因此，$s(0) = 100$、$s(t) = 50$、$s(2t) = 20$、$s(3t) = 10$。
该函数平滑、单调，得分始终落在 $(0, 100]$ 内。

支柱 $P$ 的得分取所属指标中的最低分，每项指标都使用自身的阈值：

```math
s_P = \min_{m \in P} s(r_m)
```

再对各支柱取加权几何平均，得到质量分数：

```math
Q = \prod_{P} s_P^{w_P}
```

其中，$w_P \ge 0$ 是归一化后的支柱权重，满足 $\sum_P w_P = 1$。
默认相对权重 $(1, 1, 1, 0.5)$ 归一化后为 $(2/7, 2/7, 2/7, 1/7)$。

采用几何平均后，某个支柱的低分会拉低整体分数，难以被其他支柱的高分掩盖。

类的得分由其唯一的支柱决定。文件同样使用几何平均综合函数和类的得分，
其中每个函数按长度加权，每个类按其定义的方法数加权。
如果文件中没有参与评分的类，则不包含类这一因子，文件得分完全由函数决定。

质量分为四个等级：**优秀**（excellent，≥80）、**良好**（good，≥50）、
**一般**（fair，≥25）、**较差**（poor，<25）。

### 认知复杂度的覆盖范围

对于在当前文件内解析出的递归环，环中的每个函数都会增加一点认知复杂度。
解析依赖词法作用域内的唯一名称；Java 调用还要求目标方法为 private、static
或 final，且固定参数个数与实参个数匹配。存在歧义的绑定和动态接收者会被跳过。

C++ 实现参考了
[rust-code-analysis 的 37e5d83 版本][rca-cpp]。
C++ 使用未经修改的 `tree-sitter-cpp` 0.23.4 语法。
函数、构造函数、析构函数、转换运算符、lambda、类、结构体、联合体和命名空间
各有独立的指标空间。解析前会对 `"%" PRIi32` 等标准 `<cinttypes>` 格式片段
进行等字节宽度的规范化处理；报告保留原始源码和位置。

C++ 宏替换列表中，源码可见的控制流和布尔运算序列只在定义处计入一次，
不会因调用次数增加而重复计入。这是源码分析：不加载头文件，不选择预处理分支，
也不展开依赖构建配置的宏。递归解析使用当前文件中唯一的非限定名称，
跳过重载、限定调用和间接调用。
上游关于 C++ 宏复杂度的 TODO 已由替换列表测试覆盖；
上游忽略的格式宏回归测试（Mozilla issue 1142）也会在本项目中运行。
上游关于预处理器警告和性能的 TODO 属于 Mozilla 的预处理流程，本项目不使用该流程。

对于 Rust，非限定的 `assert!`、`dbg!` 和 `vec!` 调用会计入可见实参表达式的复杂度，
包括嵌套宏以及调用方的嵌套层级，但不会进行宏展开。
对于导入的宏名或局部重新定义的宏名，采取保守策略，跳过处理。
限定调用、跨文件调用、更多绑定形式及更广泛的宏支持由
[issue #1](https://github.com/meymchen/lspf-analysis/issues/1) 跟踪。

[rca-cpp]: https://github.com/mozilla/rust-code-analysis/tree/37e5d83c056c8cbf827223d5814a93c5218df1a9

### C++ 的 ABC 和类指标

C++ ABC 将显式初始化、赋值、自增和自减操作计为 A；
将调用表达式、`new`、`delete` 以及跳入更深层代码块的跳转计为 B；
将比较、一元布尔测试、条件表达式、`else`、switch 标签和 `catch` 子句计为 C。
通过语法结构区分模板尖括号与比较、声明符与调用。
宏替换列表只在定义处计入一次，不会在调用处重复累计宏体。
不会根据语法推断隐式构造函数、析构函数和重载运算符调用。

NPA 统计每个类、结构体或联合体声明的公有数据成员数，NPM 统计其公有方法数。
统计包括静态成员、构造函数、析构函数、转换运算符和方法模板。
`class` 的默认访问权限为 private，`struct` 和 `union` 为 public，
之后按访问说明符调整。函数指针计为数据成员；友元和继承成员不计入。
匿名联合体的字段归属于外层类，嵌套类则单独统计。
预处理分支按源码顺序遍历，因此这些计数描述的是源文件，而非某个选定的构建配置。

WMC 累加方法的圈复杂度，不包含嵌套 lambda 和局部类。
方法归属于声明它的类；若能无歧义地匹配到类外定义，则使用该定义的复杂度，
但不增加 NPM。匹配依据包括命名空间、类名、参数类型 token 和方法的 cv/ref 限定符，
忽略参数名和默认值。对于所属类为泛型模板的情况，只要源码签名一致，也会进行匹配。
不解析类型别名、语义上的类型等价、重命名的模板参数或跨文件定义。
存在歧义的匹配会保留为未解析状态。

方法体缺失或格式错误会导致 WMC 不完整。此时 JSON 将受影响的 WMC 总和报告为 `null`，
并同时提供 `known_complexity` 和 `unresolved_methods`；
YAML 和文本使用各自的非有限值表示。
显式 defaulted、deleted 和纯虚声明的可执行源码复杂度计为零，
编译器为它们生成的行为不在此指标的范围内。
文本指标输出也包含上述两个覆盖情况字段。
C++ 的 ABC 和类指标目前不影响健康度评分。

### 有意不纳入评分的指标

引擎计算的指标多于评分实际使用的指标。以下取舍基于证据，并非遗漏。

- **可维护性指数**：作为熟悉的辅助参考，在每个文件的报告中展示，但不参与评分。
  它的常数拟合自 20 世纪 90 年代的一组语料，用平均值抹平了所概括的分布，
  并受到规模因素的混淆影响（[Heitlager 等，2007][hkv07]、
  [van Deursen 2014][vd14]、[El Emam 等，2001][ee01]、[Sjøberg 等，2012][sj12]）。
- **注释密度**：得到重复验证的研究结论关注的是注释是否*准确*，
  而非数量多少（[Rani 等，2023][ra23]）。
- **出口数量**：风格指南对单一出口规则各有主张，但文献检索未发现
  将出口数量与缺陷或理解能力联系起来的受控研究。
- **方法数量**：单纯计数缺少复杂度加权，而 WMC 正是借助这种加权
  反映类承担了多少工作。方法数量保留在类报告中，用作文件综合各类得分时的权重，
  本身不参与评分。
- **ABC**：对 Java 和 C++ 计算并报告，但不参与评分。
  只为这两种语言增加另一项规模指标会削弱可比性。未计算时不显示。

### 默认设置

| 设置 | 默认值 | 含义 |
| --- | --- | --- |
| `complexityThreshold` | 15 | 认知复杂度达到此值时得 50 分 |
| `cyclomaticThreshold` | 10 | 圈复杂度达到此值时得 50 分 |
| `lengthThreshold` | 30 | 语句数达到此值时得 50 分 |
| `workingMemoryThreshold` | 8 | 名称数达到此值时得 50 分 |
| `halsteadDifficultyThreshold` | 12 | Halstead 难度达到此值时得 50 分 |
| `parametersThreshold` | 4 | 参数个数达到此值时得 50 分 |
| `wmcThreshold` | 34 | 类的加权方法数达到此值时得 50 分 |
| `publicMethodsThreshold` | 14 | 类的公有方法数达到此值时得 50 分 |
| `publicAttributesThreshold` | 8 | 类的公有属性数达到此值时得 50 分 |
| `weights` | 1 / 1 / 1 / 0.5 | 支柱的相对权重，使用前归一化 |
| `weights.classDesign` | 0.5 | 文件中的类相对于函数的权重 |
| `qualityWarn` | 25 | 函数质量低于此值时报告诊断 |
| `qualityError` | 10 | 低于此值时报告错误，而非警告 |

在引入第四个支柱之前已有的三个阈值保留原名，旧模型的配置仍保持原有含义。

### 参考文献

- \[c18\] G. A. Campbell. *Cognitive Complexity: an overview and evaluation.*
  TechDebt 2018. <https://doi.org/10.1145/3194164.3194186>
- \[mb20\] M. Muñoz Barón, M. Wyrich, S. Wagner. *An Empirical Validation of
  Cognitive Complexity as a Measure of Source Code Understandability.* ESEM
  2020. <https://doi.org/10.1145/3382494.3410636>
  另见 L. Lavazza 等发表于 JSS 197（2023）的研究，
  <https://doi.org/10.1016/j.jss.2022.111561>；
  该研究对认知复杂度是否优于较早的度量持更怀疑的态度。
- \[mc76\] T. J. McCabe. *A Complexity Measure.* IEEE TSE SE-2(4), 1976.
  <https://doi.org/10.1109/TSE.1976.233837>
- \[la16\] D. Landman, A. Serebrenik, E. Bouwers, J. J. Vinju. *Empirical
  analysis of the relationship between CC and SLOC in a large corpus of Java
  methods and C functions.* JSEP 28(7), 2016.
  <https://doi.org/10.1002/smr.1760>
- \[hkv07\] I. Heitlager, T. Kuipers, J. Visser. *A Practical Model for
  Measuring Maintainability.* QUATIC 2007.
  <https://doi.org/10.1109/QUATIC.2007.7>
- \[sig\] SIG/TÜV NORD CERT. *Evaluation Criteria Trusted Product
  Maintainability.* 阈值校准依据：T. L. Alves, C. Ypma, J. Visser,
  *Deriving metric thresholds from benchmark data*, ICSM 2010.
  <https://doi.org/10.1109/ICSM.2010.5609747>
- \[mi56\] G. A. Miller. *The magical number seven, plus or minus two.*
  Psychological Review 63(2), 1956. <https://doi.org/10.1037/h0043158>
- \[co01\] N. Cowan. *The magical number 4 in short-term memory.* Behavioral
  and Brain Sciences 24(1), 2001.
  <https://doi.org/10.1017/S0140525X01003922>
- \[pe21\] N. Peitek, S. Apel, C. Parnin, A. Brechmann, J. Siegmund. *Program
  Comprehension and Code Complexity Metrics: An fMRI Study.* ICSE 2021.
  <https://doi.org/10.1109/ICSE43902.2021.00056>
- \[to22\] F. N. Topuz. *Empirical Evidence of the Consequences of Bad Smells in
  Software.* Auburn University, 2022.
  <https://etd.auburn.edu/handle/10415/8100>
- \[vd14\] A. van Deursen. *Think Twice Before Using the "Maintainability
  Index".* 2014.
  <https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/>
- \[ee01\] K. El Emam, S. Benlarbi, N. Goel, S. N. Rai. *The Confounding Effect
  of Class Size on the Validity of Object-Oriented Metrics.* IEEE TSE 27(7),
  2001. <https://doi.org/10.1109/32.935855>
- \[sj12\] D. I. K. Sjøberg, B. Anda, A. Mockus. *Questioning software
  maintenance metrics.* ESEM 2012.
  <https://doi.org/10.1145/2372251.2372269>
- \[ra23\] P. Rani, A. Blasi, N. Stulova, et al. *A decade of code comment
  quality assessment: a systematic literature review.* JSS 195, 2023.
  <https://doi.org/10.1016/j.jss.2022.111515>
- \[fi15\] T. Filó, M. Bigonha, K. Ferreira. *A Catalogue of Thresholds for
  Object-Oriented Software Metrics.* SOFTENG 2015.
  <https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf>
  同一作者团队在 JBCS 2024 的后续研究中评估了这些阈值用于代码异味检测和缺陷预测的效果，
  <https://journals-sol.sbc.org.br/index.php/jbcs/article/view/3373>。
- \[fe12\] K. Ferreira, M. Bigonha, R. Bigonha, L. Mendes, H. Almeida.
  *Identifying thresholds for object-oriented software metrics.* JSS 85(2),
  2012. <https://doi.org/10.1016/j.jss.2011.05.044>

[c18]: https://doi.org/10.1145/3194164.3194186
[mb20]: https://doi.org/10.1145/3382494.3410636
[mc76]: https://doi.org/10.1109/TSE.1976.233837
[la16]: https://doi.org/10.1002/smr.1760
[hkv07]: https://doi.org/10.1109/QUATIC.2007.7
[sig]: https://doi.org/10.1109/ICSM.2010.5609747
[mi56]: https://doi.org/10.1037/h0043158
[co01]: https://doi.org/10.1017/S0140525X01003922
[pe21]: https://doi.org/10.1109/ICSE43902.2021.00056
[to22]: https://etd.auburn.edu/handle/10415/8100
[vd14]: https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/
[ee01]: https://doi.org/10.1109/32.935855
[sj12]: https://doi.org/10.1145/2372251.2372269
[ra23]: https://doi.org/10.1016/j.jss.2022.111515
[fi15]: https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf
[fe12]: https://doi.org/10.1016/j.jss.2011.05.044

## 安装

### VS Code

[`clients/vscode`](./clients/vscode) 是 VS Code 扩展，内置服务器可执行文件，
无需额外安装服务器。在扩展发布之前，可为当前平台构建 VSIX：

```console
npm --prefix clients/vscode ci
npm --prefix clients/vscode run package
code --install-extension clients/vscode/lspf-analysis-<platform>-<version>.vsix
```

`npm run package -- --target darwin-arm64` 可为其他平台构建；
[扩展的 README](./clients/vscode/README.md) 列出了支持的平台。

### IntelliJ IDEA

[`clients/intellij`](./clients/intellij) 是 IntelliJ IDEA 插件，
基于平台自带的 LSP 客户端，同样内置服务器可执行文件。
需要 **2026.1.4 或更高版本**：该版本开放了 LSP 客户端 API 的源码，
此前基于该 API 构建的插件只能在商业版 IDE 中生效。
在插件发布之前，可构建 ZIP 并从磁盘安装：

```console
clients/intellij/gradlew -p clients/intellij buildPlugin
```

然后打开 **Settings → Plugins → ⚙ → Install Plugin from Disk…**，选择
`clients/intellij/build/distributions/lspf-analysis-intellij-win32-x64-<version>.zip`。

目前仅 Windows x64 版本附带服务器可执行文件。其他平台上的插件用法相同，
但需要单独安装服务器，并在 **Language server path** 中指定其路径。
详情见[插件的 README](./clients/intellij/README.md)。

### Visual Studio

[`clients/vs`](./clients/vs) 包含适用于 Visual Studio 2026 和 2022 的 Windows x64 扩展。
打开仓库根目录的 `lspf-analysis.sln`，按 F5 即可构建并在实验实例中调试。
受支持的源码编辑器会自动启动分析。诊断、悬停表格、文件健康度摘要和可排序的函数树
均使用与其他客户端相同的服务器。可配置健康度阈值和权重、诊断、服务器路径及协议跟踪。
打包、stdio/TCP/WebSocket 调试及集成测试的说明见
[扩展的 README](./clients/vs/README.md)。

### 单独安装可执行文件

```console
cargo install --path crates/lspf-analysis
```

该命令会将 `lspf-analysis` 可执行文件放入 Cargo 的 bin 目录（默认为 `~/.cargo/bin`）。
请确保该目录已加入 `PATH`。

## 运行服务器

```console
lspf-analysis serve                       # stdio: what editors launch
lspf-analysis serve --stdio               # the same, said explicitly
lspf-analysis serve --tcp 127.0.0.1:9257  # one TCP client
lspf-analysis serve --ws 127.0.0.1:9258   # one WebSocket client
```

三种传输方式互斥。日志写入 stderr；stdout 仅用于传输 LSP 协议数据。
设置 `RUST_LOG=lspf=trace` 可查看连接建立过程。

### 编辑器配置

VS Code 和 IntelliJ IDEA 客户端会自动完成配置。
其他编辑器可按配置普通语言服务器的方式启动 `lspf-analysis serve --stdio`，
对应的语言 ID 为 `java`、`rust`、`python`、`javascript`、`javascriptreact`、
`typescript` 和 `typescriptreact`。

打开包含较长或深层嵌套函数的源文件，函数名处应出现警告，
悬停在名称上应能看到四项数值。

### 文件健康度摘要

文件质量属于整个文档，没有合适的局部代码范围可以承载它。
因此，每次分析都会推送一条 `lspfAnalysis/fileHealth` 通知：

```json
{
  "uri": "file:///src/lib.rs",
  "quality": 63.4,
  "grade": "good",
  "functions": 7,
  "below": 1,
  "bands": { "excellent": 4, "good": 2, "fair": 0, "poor": 1 },
  "worst": [
    {
      "name": "tangled",
      "quality": 22.1,
      "grade": "poor",
      "line": 84,
      "weakestPillar": "control flow",
      "weakestMetric": "cognitive complexity"
    }
  ]
}
```

VS Code 扩展在状态栏的悬停提示中显示 `bands` 的计数和 `worst` 列表，
后者的长度上限由 `worstFunctions` 决定。
不监听此方法的客户端会忽略该通知，无需做其他修改。

### 逐函数健康度明细

自行绘制界面的客户端需要原始数值，因此可以通过 `lspfAnalysis/functionHealth`
请求获取某个文档中所有函数的数据：

```json
{ "uri": "file:///src/lib.rs" }
```

响应按源码顺序列出函数，并提供各支柱对应的具体指标：

```json
{
  "uri": "file:///src/lib.rs",
  "functions": [
    {
      "name": "tangled",
      "startLine": 84,
      "endLine": 131,
      "quality": 22.1,
      "grade": "poor",
      "weakestPillar": "control flow",
      "weakestMetric": "cognitive complexity",
      "pillars": [
        {
          "name": "control flow",
          "score": 11.9,
          "measures": [
            {
              "name": "cognitive complexity",
              "value": 41,
              "threshold": 15,
              "score": 11.9
            }
          ]
        }
      ]
    }
  ]
}
```

如果服务器尚未分析该文档，响应为 `null`。
明细通过独立请求获取，而不是作为 `fileHealth` 的附加字段推送：
后者会在每次按键后发送，而明细的 JSON 数据量大一个数量级，
只有打开相关视图的客户端才需要。VS Code 扩展会为其“函数健康度”树视图拉取这些数据。

### 配置

设置放在 `initializationOptions` 或 `workspace/didChangeConfiguration`
中的 `lspfAnalysis` 节下。未指定的设置使用默认值，
修改设置后会重新发布所有已打开文档的分析结果：

```json
{
  "lspfAnalysis": {
    "health": {
      "complexityThreshold": 20,
      "qualityWarn": 40
    },
    "diagnostics": {
      "enabled": true,
      "perMetric": false,
      "file": false
    },
    "locale": "zh-cn"
  }
}
```

- `diagnostics.perMetric`：对于整体质量达标、但某个支柱得分很差的函数，
  额外给出 Information 级别的提示。
- `diagnostics.file`：当文件整体质量低于 `qualityError` 时，额外报告文件级诊断。

如果客户端完全不发送配置，也可以通过环境变量 `LSPF_ANALYSIS_SETTINGS`
提供相同格式的 JSON。

### 界面语言

`locale` 使用编辑器的语言标签，例如 `zh-cn`、`en` 或客户端提供的其他标签。
目前提供简体中文和英文翻译，其他语言统一显示英文。
VS Code 扩展会发送自身的显示语言，无需额外设置。

该选项只影响供人阅读的文本，包括悬停提示和诊断消息。
`lspfAnalysis/fileHealth` 和 `lspfAnalysis/functionHealth` 中的支柱名、指标名和等级名
始终使用英文，便于客户端处理。诊断的 `code` 和 `source` 也保持不变。
自行绘制界面的客户端需要自行翻译这些词汇，VS Code 扩展就是这样处理的。

## 命令行分析

```console
lspf-analysis metrics --paths src
lspf-analysis metrics --paths src -O json --pr -o /tmp/report
lspf-analysis metrics --paths src -I '*.rs' -X '*/generated/*' -j 8
```

不指定 `--output-format` 时，会按文件打印指标树和文件质量；
指定后，每个文件的 `{metrics, health}` 会序列化为 `json`、`toon` 或 `cbor`，
分别适用于程序解析、模型上下文和数据传输。
无论哪种方式，最后都会输出仓库摘要，包括整体质量、各等级的数量和质量最差的函数。

[`toon`](https://github.com/toon-format/toon) 适合将报告放入模型上下文。
报告中大部分内容是结构相同的指标行，TOON 将这些键只写一次作为表头，
无需在每一行重复：

```toon
measures[2]{name,value,threshold,score}:
  cognitive complexity,10,15,69.23076923076923
  cyclomatic complexity,5,10,80
```

在本仓库源码生成的报告中，这比带缩进的 JSON 大约小三分之一。
TOON 只有一种输出格式，因此 `--pr` 对它不起作用。

## 构建与测试

```console
cargo build --workspace
cargo test --workspace
```

`crates/lspf-analysis-core/tests` 中的快照测试为每种语言选取一个小文件，
覆盖其指标和健康度评分。要审查快照变更，请安装
[cargo-insta](https://crates.io/crates/cargo-insta)，然后运行：

```console
cargo insta test --review
```

`crates/lspf-analysis/tests/server_journey.rs` 通过 lspf 的内存传输进行端到端测试，
覆盖打开、编辑和关闭文档时的诊断，以及配置变更和悬停提示。

健康度模块使用 LaTeX 编写公式。rustdoc 本身不支持数学公式，
需要传入 KaTeX 页头才能渲染：

```console
RUSTDOCFLAGS="--html-in-header crates/lspf-analysis-core/katex.html" \
  cargo doc -p lspf-analysis-core --no-deps --open
```

docs.rs 会从 `package.metadata.docs.rs` 中读取同一个页头配置。

VS Code 客户端有独立的测试：

```console
npm --prefix clients/vscode ci
npm --prefix clients/vscode test
```

在 VS Code 中选择以下三个调试入口之一，按 F5 启动：

- **stdio**：启动扩展开发宿主，由宿主以 `serve --stdio` 启动服务器，
  与发布版扩展使用相同的传输方式。除非配置了 `lspfAnalysis.server.path`，
  否则使用本地调试构建。服务器日志显示在 **Output → LSPF Analysis** 中。
- **tcp**：通过 CodeLLDB 在 `127.0.0.1:9257` 启动服务器，
  同时启动经 TCP 连接服务器的扩展宿主。
- **websocket**：通过 CodeLLDB 在 `127.0.0.1:9258` 启动服务器，
  同时启动经 WebSocket 连接服务器的扩展宿主。

每个入口都会在启动前构建服务器并准备扩展。
TCP 和 WebSocket 将服务器与宿主作为两个独立的调试会话运行，
支持 Rust 和扩展代码断点。服务器启动期间，宿主会重试连接。
停止任一会话都会同时停止两个会话；关闭或重新加载宿主后，需要重新启动调试入口。
服务器日志输出到集成终端。stdio 入口支持扩展代码断点。

菜单中只显示这三个入口。网络入口使用的固定服务器和宿主配置会被隐藏，
无需选择协议，也不需要控制台监听程序或 Python 辅助脚本。

运行 `npm --prefix clients/vscode run test:debug-transport`，
可构建服务器并验证 stdio、TCP 和 WebSocket 下的 LSP 初始化及关闭流程。
需要原生 POSIX 或 Windows 路径行为的测试会在其他系统上跳过。

### 更新语法

更新 `crates/lspf-analysis-core/Cargo.toml` 和 `enums/Cargo.toml`
中的语法 crate 版本，然后运行 `./recreate-grammars.sh`，
重新生成 `crates/lspf-analysis-core/src/languages` 下的语言绑定。
使用 `check-grammar-crate.py` 比较版本更新前后在大型仓库上计算出的指标差异。

## 目录结构

- [`crates/lspf-analysis-core`](./crates/lspf-analysis-core)：指标引擎和健康度评分层。
- [`crates/lspf-analysis`](./crates/lspf-analysis)：语言服务器及其 `lspf-analysis` 可执行文件。
- [`clients/vscode`](./clients/vscode)：VS Code 扩展，每个 VSIX 打包一个平台的可执行文件。
- [`clients/intellij`](./clients/intellij)：IntelliJ IDEA 插件，
  基于平台自带的 LSP 客户端，以相同方式打包。
- [`clients/vs`](./clients/vs)：Visual Studio 2026/2022 扩展及其内置语言服务器。
- [`enums`](./enums)：tree-sitter 节点类型绑定的生成器。

## 致谢

指标引擎派生自 Mozilla 的
[**rust-code-analysis**](https://github.com/mozilla/rust-code-analysis)。
没有它就不会有本项目。原作者构建了 tree-sitter 解析层、语言抽象、空间模型，
以及本项目使用的几乎所有指标；本项目在此基础上增加了一层轻量的评分逻辑和语言服务器。
上游仓库已停止维护，因此创建了这个分支。原有工作的成果应归功于原作者。

如果在学术工作中使用这些指标，请引用他们的论文：

```bibtex
@article{ARDITO2020100635,
    title = {rust-code-analysis: A Rust library to analyze and extract maintainability information from source codes},
    journal = {SoftwareX},
    volume = {12},
    pages = {100635},
    year = {2020},
    issn = {2352-7110},
    doi = {https://doi.org/10.1016/j.softx.2020.100635},
    url = {https://www.sciencedirect.com/science/article/pii/S2352711020303484},
    author = {Luca Ardito and Luca Barbato and Marco Castelluccio and Riccardo Coppola and Calixte Denizet and Sylvestre Ledru and Michele Valsesia},
    keywords = {Algorithm, Software metrics, Software maintainability, Software quality},
}
```

## 许可证

- Mozilla 定义的语法使用 MIT 许可证。
- **lspf-analysis** 和 **lspf-analysis-core** 使用继承自 rust-code-analysis 的
  [Mozilla Public License v2.0](https://www.mozilla.org/MPL/2.0/)。
