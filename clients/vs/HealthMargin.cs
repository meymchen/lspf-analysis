using System;
using System.ComponentModel.Composition;
using System.Windows;
using System.Windows.Controls;
using Microsoft.VisualStudio.Shell;
using Microsoft.VisualStudio.Text.Editor;
using Microsoft.VisualStudio.Utilities;
using Newtonsoft.Json.Linq;

namespace LspfAnalysis
{
    [Export(typeof(IWpfTextViewMarginProvider))]
    [Name("LSPF File Health")]
    [Order(After = PredefinedMarginNames.HorizontalScrollBar)]
    [MarginContainer(PredefinedMarginNames.Bottom)]
    [ContentType("text")]
    [TextViewRole(PredefinedTextViewRoles.Document)]
    internal sealed class HealthMarginProvider : IWpfTextViewMarginProvider
    {
        public IWpfTextViewMargin CreateMargin(IWpfTextViewHost host, IWpfTextViewMargin container) => new HealthMargin(host.TextView);
    }

    internal sealed class HealthMargin : Button, IWpfTextViewMargin
    {
        private readonly IWpfTextView view;
        internal HealthMargin(IWpfTextView view)
        {
            this.view = view;
            HorizontalContentAlignment = HorizontalAlignment.Left;
            Padding = new Thickness(6, 2, 6, 2);
            BorderThickness = new Thickness(0);
            SetResourceReference(BackgroundProperty, VsBrushes.WindowKey);
            SetResourceReference(ForegroundProperty, VsBrushes.WindowTextKey);
            Click += (sender, args) => AnalysisPackage.Instance?.ShowHealth();
            AnalysisSession.Instance.Changed += Refresh;
            view.Closed += Closed;
            Refresh();
        }
        private void Refresh()
        {
            var doc = AnalysisSession.Instance.Find(view.TextBuffer);
            Visibility = doc != null && HealthProtocol.Language(doc.Source.FilePath) != null && AnalysisPackage.Instance?.Options.StatusBarEnabled != false && AnalysisSession.Instance.Ready ? Visibility.Visible : Visibility.Collapsed;
            Content = Labels.Summary(doc?.Summary);
            if (doc?.Summary is JObject health)
            {
                ToolTip = string.Join(Environment.NewLine, [
                    Labels.Summary(health),
                    $"{Labels.Name("excellent")}: {health["bands"]["excellent"]}  ·  {Labels.Name("good")}: {health["bands"]["good"]}",
                    $"{Labels.Name("fair")}: {health["bands"]["fair"]}  ·  {Labels.Name("poor")}: {health["bands"]["poor"]}",
                    Labels.Text("Click to explore functions and open the weakest ones.", "点击查看函数详情并跳转到较弱的函数。")
                ]);
            }
            else ToolTip = null;
        }
        private void Closed(object sender, EventArgs e) => Dispose();
        public void Dispose() { AnalysisSession.Instance.Changed -= Refresh; view.Closed -= Closed; }
        public FrameworkElement VisualElement => this;
        public double MarginSize => ActualHeight;
        public bool Enabled => Visibility == Visibility.Visible;
        public ITextViewMargin GetTextViewMargin(string marginName) => marginName == "LSPF File Health" ? this : null;
    }
}
