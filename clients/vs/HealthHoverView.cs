using System;
using System.Linq;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Media;
using Microsoft.VisualStudio.PlatformUI;

namespace LspfAnalysis
{
    // Render the server's portable Markdown subset as WPF text and a table.
    // HTML and links remain inert text; no browser or command URI is involved.
    internal static class HealthHoverView
    {
        internal static FrameworkElement Create(string markdown)
        {
            // Another Quick Info provider can widen the popup. Keep our capped
            // content at its left edge instead of letting WPF center the block.
            var panel = new HealthHoverPanel { MaxWidth = 760, HorizontalAlignment = HorizontalAlignment.Left };
            Grid table = null;
            foreach (var raw in markdown.Split('\n'))
            {
                var line = raw.TrimEnd('\r');
                if (line.StartsWith("|", StringComparison.Ordinal) && line.EndsWith("|", StringComparison.Ordinal))
                {
                    var cells = line.Substring(1, line.Length - 2).Split('|').Select(c => c.Trim()).ToArray();
                    if (cells.All(c => c.Length > 0 && c.All(ch => ch == ':' || ch == '-' || ch == ' '))) continue;
                    if (table == null)
                    {
                        table = new Grid { Margin = new Thickness(0, 3, 0, 3) };
                        panel.Children.Add(table);
                    }
                    while (table.ColumnDefinitions.Count < cells.Length) table.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                    var row = table.RowDefinitions.Count;
                    table.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
                    for (var column = 0; column < cells.Length; column++)
                    {
                        var text = Inline(cells[column]);
                        text.Margin = new Thickness(3, 2, 12, 2);
                        if (row == 0) text.FontWeight = FontWeights.SemiBold;
                        if (column == 0 && row > 0)
                        {
                            var grade = cells[column].Replace("**", "");
                            if (new[] { "A", "B", "C", "D" }.Contains(grade))
                                panel.AddGrade(text, grade);
                        }
                        Grid.SetRow(text, row);
                        Grid.SetColumn(text, column);
                        table.Children.Add(text);
                    }
                }
                else
                {
                    table = null;
                    if (line.Length == 0) continue;
                    var text = Inline(line);
                    text.Margin = new Thickness(0, 3, 0, 3);
                    panel.Children.Add(text);
                }
            }
            return panel;
        }

        private static TextBlock Inline(string value)
        {
            var text = new TextBlock { TextWrapping = TextWrapping.Wrap };
            bool bold = false, code = false;
            var start = 0;
            for (var i = 0; i <= value.Length; i++)
            {
                var isBold = i + 1 < value.Length && value[i] == '*' && value[i + 1] == '*';
                var isCode = i < value.Length && value[i] == '`';
                if (i != value.Length && !isBold && !isCode) continue;
                var run = new Run(value.Substring(start, i - start));
                if (bold) run.FontWeight = FontWeights.Bold;
                if (code) run.FontFamily = new FontFamily("Consolas");
                text.Inlines.Add(run);
                if (isBold) { bold = !bold; i++; }
                if (isCode) code = !code;
                start = i + 1;
            }
            return text;
        }
    }

    internal sealed class HealthHoverPanel : StackPanel
    {
        private readonly Dictionary<TextBlock, string> grades = new Dictionary<TextBlock, string>();
        private static readonly DependencyProperty ThemeBackgroundProperty = DependencyProperty.Register(
            "ThemeBackground", typeof(Brush), typeof(HealthHoverPanel), new PropertyMetadata(null, ThemeChanged));
        private static readonly DependencyProperty ThemeForegroundProperty = DependencyProperty.Register(
            "ThemeForeground", typeof(Brush), typeof(HealthHoverPanel), new PropertyMetadata(null, ThemeChanged));
        private static readonly DependencyProperty HighContrastProperty = DependencyProperty.Register(
            "HighContrast", typeof(bool), typeof(HealthHoverPanel), new PropertyMetadata(false, ThemeChanged));

        internal HealthHoverPanel()
        {
            RefreshColors();
            // These resources are supplied by the editor's current theme settings.
            // Dynamic references update existing popups on theme changes without a
            // global event subscription that could keep a closed popup alive.
            SetResourceReference(ThemeBackgroundProperty, EnvironmentColors.ToolTipBrushKey);
            SetResourceReference(ThemeForegroundProperty, EnvironmentColors.ToolTipTextBrushKey);
            SetResourceReference(HighContrastProperty, SystemParameters.HighContrastKey);
        }

        internal void AddGrade(TextBlock text, string grade)
        {
            text.FontWeight = FontWeights.Bold;
            grades.Add(text, grade);
            RefreshColors();
        }

        private static void ThemeChanged(DependencyObject sender, DependencyPropertyChangedEventArgs args) =>
            ((HealthHoverPanel)sender).RefreshColors();

        private void RefreshColors()
        {
            var background = GetValue(ThemeBackgroundProperty) as SolidColorBrush;
            // A missing, translucent, or non-solid resource doesn't identify a
            // usable appearance. Follow the requested dark-mode fallback.
            var knownBackground = background != null && background.Color.A == 255 && background.Opacity == 1;
            var surface = knownBackground ? background.Color : Color.FromRgb(43, 43, 43);
            var dark = !knownBackground || Luminance(surface) < 0.179;
            var foreground = GetValue(ThemeForegroundProperty) as Brush ??
                MakeBrush(dark ? "#F5F5F7" : "#1D1D1F");
            SetValue(TextElement.ForegroundProperty, foreground);
            foreach (var grade in grades)
            {
                if ((bool)GetValue(HighContrastProperty))
                {
                    // Respect the user's high-contrast text choice. The grade and
                    // bold weight convey the same meaning without relying on hue.
                    grade.Key.Foreground = foreground;
                    continue;
                }
                // Apple HIG-inspired semantic families with separate appearances.
                // These are adapted for VS surfaces, not fixed Apple system tokens.
                string color;
                switch (grade.Value)
                {
                    case "A": color = dark ? "#30D158" : "#248A3D"; break;
                    case "B": color = dark ? "#64D2FF" : "#0055CC"; break;
                    case "C": color = dark ? "#FFD60A" : "#895400"; break;
                    default: color = dark ? "#FF6961" : "#D70015"; break;
                }
                grade.Key.Foreground = WithContrast((Color)ColorConverter.ConvertFromString(color), surface, dark);
            }
        }

        private static SolidColorBrush MakeBrush(string color)
        {
            var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(color));
            brush.Freeze();
            return brush;
        }

        private static SolidColorBrush WithContrast(Color color, Color background, bool dark)
        {
            var adjusted = color;
            // Small colored text gets at least 4.5:1 against the actual VS popup
            // background, including custom themes. Preserve hue as far as possible.
            for (var step = 1; Contrast(adjusted, background) < 4.5 && step <= 255; step++)
            {
                var target = dark ? 255 : 0;
                adjusted = Color.FromRgb(
                    (byte)(color.R + (target - color.R) * step / 255),
                    (byte)(color.G + (target - color.G) * step / 255),
                    (byte)(color.B + (target - color.B) * step / 255));
            }
            var brush = new SolidColorBrush(adjusted);
            brush.Freeze();
            return brush;
        }

        private static double Contrast(Color a, Color b) =>
            (Math.Max(Luminance(a), Luminance(b)) + 0.05) / (Math.Min(Luminance(a), Luminance(b)) + 0.05);
        private static double Luminance(Color color) => 0.2126 * Linear(color.R) + 0.7152 * Linear(color.G) + 0.0722 * Linear(color.B);
        private static double Linear(byte channel)
        {
            var value = channel / 255.0;
            return value <= 0.04045 ? value / 12.92 : Math.Pow((value + 0.055) / 1.055, 2.4);
        }
    }
}
