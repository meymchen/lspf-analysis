using System;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using Microsoft.VisualStudio.Shell;
using Microsoft.VisualStudio.Text.Editor;
using Newtonsoft.Json.Linq;

namespace LspfAnalysis
{
    internal static class Labels
    {
        internal static string Text(string en, string zh) => CultureInfo.CurrentUICulture.Name.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? zh : en;
        internal static string Name(string name)
        {
            switch (name)
            {
                case "excellent": return Text("Excellent", "优秀");
                case "good": return Text("Good", "良好");
                case "fair": return Text("Fair", "一般");
                case "poor": return Text("Poor", "较差");
                case "control flow": return Text("Control flow", "控制流");
                case "size": return Text("Size", "规模");
                case "vocabulary load": return Text("Vocabulary load", "词汇负担");
                case "class design": return Text("Class design", "类设计");
                case "statements": return Text("Statements", "语句数");
                case "cognitive complexity": return Text("Cognitive complexity", "认知复杂度");
                case "cyclomatic complexity": return Text("Cyclomatic complexity", "圈复杂度");
                case "working memory": return Text("Working memory", "工作记忆");
                case "Halstead difficulty": return Text("Halstead difficulty", "Halstead 难度");
                case "<anonymous>": return Text("<anonymous>", "＜匿名函数＞");
                case "complexity": return Text("Complexity", "复杂度");
                case "length": return Text("Length", "长度");
                case "workingMemory": return Text("Working memory", "工作记忆");
                case "interface": return Text("Interface", "接口");
                case "classDesign": return Text("Class design", "类设计");
                case "cognitive": return Text("Cognitive complexity", "认知复杂度");
                case "cyclomatic": return Text("Cyclomatic complexity", "圈复杂度");
                case "sloc": return Text("Source lines", "源代码行数");
                case "halsteadDifficulty": return Text("Halstead difficulty", "Halstead 难度");
                case "parameters": return Text("Parameters", "参数数量");
                case "wmc": return Text("Weighted methods", "加权方法数");
                case "publicMethods": return Text("Public methods", "公共方法数");
                case "publicAttributes": return Text("Public attributes", "公共属性数");
                default: return name;
            }
        }
        internal static string Summary(JObject health) => health == null ? Text("Waiting for analysis…", "等待分析……") :
            $"LSPF  {HealthProtocol.Percent(health["quality"])}  {Name((string)health["grade"])}  ·  " +
            string.Format(Text("{0} functions, {1} below warning", "{0} 个函数，{1} 个低于警告阈值"), health["functions"], health["below"]);
        internal static Brush Color(string grade)
        {
            switch (grade)
            {
                case "excellent": return Brushes.SeaGreen;
                case "good": return Brushes.SteelBlue;
                case "fair": return Brushes.DarkGoldenrod;
                default: return Brushes.IndianRed;
            }
        }
    }

    [Guid("9F7DC9A9-7EDD-4E12-AF58-27C78427F8CE")]
    public sealed class HealthToolWindow : ToolWindowPane
    {
        public HealthToolWindow() : base(null)
        {
            Caption = Labels.Text("Function Health", "函数健康度");
            Content = new HealthPanel();
        }
    }

    internal sealed class HealthPanel : UserControl
    {
        private readonly TextBlock summary = new TextBlock { TextWrapping = TextWrapping.Wrap, Margin = new Thickness(6) };
        private readonly TreeView tree = new TreeView();
        private readonly StackPanel worst = new StackPanel { Margin = new Thickness(6) };
        private readonly ComboBox sort = new ComboBox { Margin = new Thickness(3), MinWidth = 100 };
        private JObject shown;
        private string shownUri;
        private bool shownSort;
        internal HealthPanel()
        {
            SetResourceReference(BackgroundProperty, VsBrushes.WindowKey);
            SetResourceReference(ForegroundProperty, VsBrushes.WindowTextKey);
            tree.SetResourceReference(BackgroundProperty, VsBrushes.WindowKey);
            tree.SetResourceReference(ForegroundProperty, VsBrushes.WindowTextKey);
            var root = new DockPanel();
            var tools = new WrapPanel();
            sort.Items.Add(Labels.Text("Worst first", "低分优先"));
            sort.Items.Add(Labels.Text("Source order", "源码顺序"));
            System.Windows.Automation.AutomationProperties.SetName(sort, Labels.Text("Sort functions", "函数排序"));
            sort.SelectedIndex = AnalysisPackage.Instance?.Options.SortByPosition == true ? 1 : 0;
            sort.SelectionChanged += (sender, args) =>
            {
                ThreadHelper.ThrowIfNotOnUIThread();
                var options = AnalysisPackage.Instance?.Options;
                if (options != null) { options.SortByPosition = sort.SelectedIndex == 1; options.SaveSettingsToStorage(); }
                Render();
            };
            tools.Children.Add(sort);
            tools.Children.Add(Button(Labels.Text("Restart server", "重启服务器"), () => AnalysisPackage.Instance?.Restart()));
            tools.Children.Add(Button(Labels.Text("Settings", "设置"), () => AnalysisPackage.Instance?.OpenSettings()));
            tools.Children.Add(Button(Labels.Text("Problems", "问题"), () => AnalysisPackage.Instance?.ShowProblems()));
            DockPanel.SetDock(tools, Dock.Top);
            DockPanel.SetDock(summary, Dock.Top);
            DockPanel.SetDock(worst, Dock.Top);
            root.Children.Add(tools);
            root.Children.Add(summary);
            root.Children.Add(worst);
            root.Children.Add(tree);
            Content = root;
            Loaded += (sender, args) => { AnalysisSession.Instance.Changed += Render; Render(); };
            Unloaded += (sender, args) => AnalysisSession.Instance.Changed -= Render;
        }

        internal static Button Button(string text, Action click)
        {
            var button = new Button { Content = text, Margin = new Thickness(3), Padding = new Thickness(5, 2, 5, 2) };
            button.Click += (sender, args) => click();
            return button;
        }

        private void Render()
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            var session = AnalysisSession.Instance;
            var doc = session.Active;
            summary.Text = !session.Ready ? Labels.Text("Language server stopped.", "语言服务器已停止。") : doc == null ?
                Labels.Text("Open a supported source file.", "请打开支持的源文件。") : Labels.Summary(doc.Summary);
            worst.Children.Clear();
            if (doc?.Summary != null)
            {
                var bands = (JObject)doc.Summary["bands"];
                worst.Children.Add(new TextBlock { Text = string.Join("  ·  ", bands.Properties().Select(p => Labels.Name(p.Name) + " " + p.Value)), TextWrapping = TextWrapping.Wrap });
                foreach (var item in (JArray)doc.Summary["worst"])
                {
                    var line = (int)item["line"];
                    var uri = doc.Uri;
                    var button = Button($"{item["name"]}  {HealthProtocol.Percent(item["quality"])}", () => AnalysisPackage.Instance?.Navigate(uri, line));
                    button.HorizontalAlignment = HorizontalAlignment.Left;
                    button.ToolTip = Labels.Name((string)item["weakestPillar"]) + " · " + Labels.Name((string)item["weakestMetric"]);
                    worst.Children.Add(button);
                }
            }
            var byPosition = AnalysisPackage.Instance?.Options.SortByPosition == true;
            if (shown == doc?.Functions && shownUri == doc?.Uri && shownSort == byPosition) return;
            shown = doc?.Functions;
            shownUri = doc?.Uri;
            shownSort = byPosition;
            tree.Items.Clear();
            if (shown == null) return;
            if (((JArray)shown["functions"]).Count == 0)
                tree.Items.Add(new TextBlock { Text = Labels.Text("No functions to analyze in this file.", "此文件中没有可分析的函数。"), Margin = new Thickness(6) });
            foreach (var function in HealthProtocol.Sort(shown, byPosition))
            {
                var uri = doc.Uri;
                var line = (int)function["startLine"];
                var node = new TreeViewItem {
                    Header = $"{function["name"]}  {HealthProtocol.Percent(function["quality"])}  ·  {Labels.Name((string)function["weakestPillar"])}",
                    ToolTip = $"{Labels.Name((string)function["grade"])}  ·  {function["startLine"]}–{function["endLine"]}\n{Labels.Name((string)function["weakestMetric"])}"
                };
                node.Foreground = Labels.Color((string)function["grade"]);
                node.MouseDoubleClick += (sender, args) => { if (node.IsSelected) { AnalysisPackage.Instance?.Navigate(uri, line); args.Handled = true; } };
                node.KeyDown += (sender, args) => { if (args.Key == Key.Enter && node.IsSelected) { AnalysisPackage.Instance?.Navigate(uri, line); args.Handled = true; } };
                foreach (var pillar in (JArray)function["pillars"])
                {
                    var pillarNode = new TreeViewItem { Header = $"{Labels.Name((string)pillar["name"])}  {HealthProtocol.Percent(pillar["score"])}" };
                    foreach (var measure in (JArray)pillar["measures"])
                        pillarNode.Items.Add(new TreeViewItem { Header = $"{Labels.Name((string)measure["name"])}  {measure["value"]} / {measure["threshold"]}  ·  {HealthProtocol.Percent(measure["score"])}",
                            ToolTip = Labels.Text("Measured value / threshold (50% score)", "测量值／阈值（对应 50% 得分）") });
                    node.Items.Add(pillarNode);
                }
                tree.Items.Add(node);
            }
        }
    }
}
