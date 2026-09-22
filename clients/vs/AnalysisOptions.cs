using System;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media;
using Microsoft.VisualStudio.PlatformUI;
using Microsoft.VisualStudio.Shell;
using Newtonsoft.Json.Linq;

namespace LspfAnalysis
{
    // This property-grid options page is edited as code, not with a component designer.
    [DesignerCategory("")]
    [Guid("2FC9E51E-82D2-411E-95EF-E491D546BDF9")]
    public sealed class AnalysisOptions : DialogPage
    {
        [OptionCategory("Server", "服务器"), OptionName("Server executable", "服务器程序"),
         OptionDescription("Absolute path to lspf-analysis.exe. Empty uses the bundled server. Restart after changing.", "lspf-analysis.exe 的绝对路径。留空时使用扩展内置服务器，修改后需重启服务器。")]
        public string ServerPath { get; set; } = "";
        [OptionCategory("Server", "服务器"), OptionName("Protocol trace", "协议跟踪"),
         OptionDescription("LSP logging level: Off, Messages, or Verbose.", "LSP 日志级别：关闭、消息或详细。")]
        [TypeConverter(typeof(OptionTraceConverter))]
        public ServerTrace Trace { get; set; } = ServerTrace.Off;
        [OptionCategory("Display", "显示"), OptionName("File health summary", "文件健康摘要"),
         OptionDescription("Show the current file health summary.", "显示当前文件的健康摘要。")]
        [TypeConverter(typeof(OptionBooleanConverter))]
        public bool StatusBarEnabled { get; set; } = true;
        [OptionCategory("Display", "显示"), OptionName("Sort functions by position", "按位置排列函数"),
         OptionDescription("Sort functions by source position instead of health.", "按函数在源文件中的位置排序。")]
        [TypeConverter(typeof(OptionBooleanConverter))]
        public bool SortByPosition { get; set; }
        [OptionCategory("Diagnostics", "诊断"), OptionName("Enabled", "启用诊断"),
         OptionDescription("Enable code health diagnostics.", "启用代码健康诊断。")]
        [TypeConverter(typeof(OptionBooleanConverter))]
        public bool DiagnosticsEnabled { get; set; } = true;
        [OptionCategory("Diagnostics", "诊断"), OptionName("Per metric", "逐指标诊断"),
         OptionDescription("Report individual metric diagnostics.", "为各项指标分别报告诊断。")]
        [TypeConverter(typeof(OptionBooleanConverter))]
        public bool DiagnosticsPerMetric { get; set; }
        [OptionCategory("Diagnostics", "诊断"), OptionName("File diagnostics", "文件诊断"),
         OptionDescription("Include file-level diagnostics.", "包含文件级诊断。")]
        [TypeConverter(typeof(OptionBooleanConverter))]
        public bool DiagnosticsFile { get; set; }
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Cognitive complexity", "认知复杂度"),
         OptionDescription("Positive threshold for cognitive complexity. Default: 15.", "认知复杂度阈值，必须大于 0。默认值：15。")]
        public double ComplexityThreshold { get; set; } = 15;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Cyclomatic complexity", "圈复杂度"),
         OptionDescription("Positive threshold for cyclomatic complexity. Default: 10.", "圈复杂度阈值，必须大于 0。默认值：10。")]
        public double CyclomaticThreshold { get; set; } = 10;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Statement count", "语句数"),
         OptionDescription("Positive threshold for statement count. Default: 30.", "语句数阈值，必须大于 0。默认值：30。")]
        public double LengthThreshold { get; set; } = 30;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Working memory", "工作记忆"),
         OptionDescription("Positive threshold for working memory. Default: 8.", "工作记忆阈值，必须大于 0。默认值：8。")]
        public double WorkingMemoryThreshold { get; set; } = 8;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Halstead difficulty", "Halstead 难度"),
         OptionDescription("Positive threshold for halstead difficulty. Default: 12.", "Halstead 难度阈值，必须大于 0。默认值：12。")]
        public double HalsteadDifficultyThreshold { get; set; } = 12;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Parameter count", "参数个数"),
         OptionDescription("Positive threshold for parameter count. Default: 4.", "参数个数阈值，必须大于 0。默认值：4。")]
        public double ParametersThreshold { get; set; } = 4;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Weighted methods per class (WMC)", "类加权方法数（WMC）"),
         OptionDescription("Positive threshold for weighted methods per class (wmc). Default: 34.", "类加权方法数（WMC）阈值，必须大于 0。默认值：34。")]
        public double WmcThreshold { get; set; } = 34;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Public methods", "公开方法数"),
         OptionDescription("Positive threshold for public methods. Default: 14.", "公开方法数阈值，必须大于 0。默认值：14。")]
        public double PublicMethodsThreshold { get; set; } = 14;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Public attributes", "公开属性数"),
         OptionDescription("Positive threshold for public attributes. Default: 8.", "公开属性数阈值，必须大于 0。默认值：8。")]
        public double PublicAttributesThreshold { get; set; } = 8;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Quality warning (%)", "质量警告（％）"),
         OptionDescription("Quality warning threshold, from 0 to 100. Default: 25.", "质量警告阈值，范围为 0～100。默认值：25。")]
        public double QualityWarn { get; set; } = 25;
        [OptionCategory("Health thresholds", "健康阈值"), OptionName("Quality error (%)", "质量错误（％）"),
         OptionDescription("Quality error threshold, from 0 to 100; cannot exceed the warning threshold. Default: 10.", "质量错误阈值，范围为 0～100，不能超过警告阈值。默认值：10。")]
        public double QualityError { get; set; } = 10;
        [OptionCategory("Health weights", "健康权重"), OptionName("Control flow", "控制流"),
         OptionDescription("Nonnegative weight for control flow. Default: 1.", "控制流权重，必须大于或等于 0。默认值：1。")]
        public double WeightComplexity { get; set; } = 1;
        [OptionCategory("Health weights", "健康权重"), OptionName("Size", "规模"),
         OptionDescription("Nonnegative weight for size. Default: 1.", "规模权重，必须大于或等于 0。默认值：1。")]
        public double WeightLength { get; set; } = 1;
        [OptionCategory("Health weights", "健康权重"), OptionName("Lexical load", "词汇负担"),
         OptionDescription("Nonnegative weight for lexical load. Default: 1.", "词汇负担权重，必须大于或等于 0。默认值：1。")]
        public double WeightWorkingMemory { get; set; } = 1;
        [OptionCategory("Health weights", "健康权重"), OptionName("Interface", "接口"),
         OptionDescription("Nonnegative weight for interface. Default: 0.5.", "接口权重，必须大于或等于 0。默认值：0.5。")]
        public double WeightInterface { get; set; } = 0.5;
        [OptionCategory("Health weights", "健康权重"), OptionName("Class design", "类设计"),
         OptionDescription("Nonnegative weight for class design. Default: 0.5.", "类设计权重，必须大于或等于 0。默认值：0.5。")]
        public double WeightClassDesign { get; set; } = 0.5;

        internal JObject Payload()
        {
            var health = new JObject();
            var weights = new JObject();
            foreach (var property in GetType().GetProperties())
            {
                if (property.PropertyType != typeof(double)) continue;
                var name = property.Name;
                var value = (double)property.GetValue(this);
                if (name.StartsWith("Weight", StringComparison.Ordinal))
                {
                    name = name.Substring(6);
                    weights[char.ToLowerInvariant(name[0]) + name.Substring(1)] = value;
                }
                else health[char.ToLowerInvariant(name[0]) + name.Substring(1)] = value;
            }
            health["weights"] = weights;
            return new JObject { ["lspfAnalysis"] = new JObject {
                ["health"] = health,
                ["diagnostics"] = new JObject { ["enabled"] = DiagnosticsEnabled, ["perMetric"] = DiagnosticsPerMetric, ["file"] = DiagnosticsFile },
                ["locale"] = CultureInfo.CurrentUICulture.Name.ToLowerInvariant(),
                // Says that this client keeps a <span>, which is what lets the
                // server colour a grade letter. LSP's own place for this is
                // general.markdown.allowedTags in the initialize request, and
                // that is where the VS Code and IntelliJ clients say it. This
                // one cannot: ILanguageClient exposes no hook for shaping
                // client capabilities, and the platform sends initialize
                // without consulting the middle layer. initializationOptions
                // is the part of the handshake this client does control, so
                // the same list travels here and the server unions the two.
                ["markdown"] = new JObject { ["allowedTags"] = new JArray("span") },
                // The appearance the server colours grade letters for. Two
                // separate things: the kind is the reader's intent, which
                // decides the palette, and the background is the surface the
                // letters land on, which the palette is then fitted to. They
                // answer different questions, so they cannot disagree.
                ["theme"] = ThemePayload()
            } };
        }

        // The tooltip surface as the server needs to see it.
        //
        // The same resource the hover popup is painted with, resolved here
        // rather than inside the popup because the server has to be told
        // before it answers. A theme whose tooltip brush is missing or
        // translucent identifies no usable surface, so only the kind is sent
        // and the server falls back to a stand-in for it.
        private static JObject ThemePayload()
        {
            var highContrast = SystemParameters.HighContrast;
            var surface = VSColorTheme.GetThemedColor(EnvironmentColors.ToolTipColorKey);
            var theme = new JObject
            {
                ["kind"] = HealthTheme.Kind(
                    Color.FromRgb(surface.R, surface.G, surface.B), highContrast)
            };
            if (surface.A == 255)
                theme["background"] = HealthTheme.Hex(Color.FromRgb(surface.R, surface.G, surface.B));
            return theme;
        }

        internal string ValidateSettings()
        {
            foreach (var property in GetType().GetProperties())
            {
                if (property.PropertyType != typeof(double)) continue;
                var value = (double)property.GetValue(this);
                if (double.IsNaN(value) || double.IsInfinity(value) || value < 0 ||
                    (property.Name.EndsWith("Threshold", StringComparison.Ordinal) && value == 0) ||
                    (property.Name.StartsWith("Quality", StringComparison.Ordinal) && value > 100))
                    return TypeDescriptor.GetProperties(this)[property.Name].DisplayName + Labels.Text(
                        ": enter a finite value in its valid range (thresholds > 0, weights >= 0, quality 0–100).",
                        "：请输入有效范围内的有限数值（阈值大于 0，权重大于或等于 0，质量为 0～100）。");
            }
            if (QualityError > QualityWarn) return Labels.Text("Quality error threshold must not exceed the warning threshold.", "质量错误阈值不能超过质量警告阈值。");
            if (!string.IsNullOrWhiteSpace(ServerPath) && (!Path.IsPathRooted(ServerPath.Trim()) || !File.Exists(ServerPath.Trim())))
                return Labels.Text("Server executable must be an existing absolute file path.", "服务器程序必须是已存在文件的绝对路径。");
            return null;
        }

        protected override void OnApply(PageApplyEventArgs e)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            var error = ValidateSettings();
            if (error != null)
            {
                e.ApplyBehavior = ApplyKind.CancelNoNavigate;
                System.Windows.Forms.MessageBox.Show(error, "LSPF Analysis");
                return;
            }
            base.OnApply(e);
            AnalysisPackage.Instance?.ApplySettings();
        }
    }
}
