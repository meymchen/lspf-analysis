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
                        // A leading cell the server coloured is drawn as the
                        // letter alone, in the colour it chose. Anything else,
                        // including a bare letter from a server that was never
                        // told a theme, falls through to the ordinary inline
                        // path and is drawn in the text colour.
                        TextBlock text;
                        if (column == 0 && row > 0 &&
                            HealthTheme.TryReadGrade(cells[column], out var grade, out var color))
                        {
                            text = Inline(grade);
                            panel.AddGrade(text, color);
                        }
                        else
                        {
                            text = Inline(cells[column]);
                        }
                        text.Margin = new Thickness(3, 2, 12, 2);
                        if (row == 0) text.FontWeight = FontWeights.SemiBold;
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
        private readonly Dictionary<TextBlock, Color> grades = new Dictionary<TextBlock, Color>();
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

        // Records a grade letter and the colour the server chose for it.
        //
        // The colour was already fitted to the background reported to the
        // server, so nothing is adjusted here. A popup already open when the
        // theme changes keeps these colours: the server has to be asked again
        // to get new ones, and it is asked on the next hover. The ordinary
        // text around them still follows the theme live, through the dynamic
        // resources above.
        internal void AddGrade(TextBlock text, Color color)
        {
            text.FontWeight = FontWeights.Bold;
            grades.Add(text, color);
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
            var dark = !knownBackground || HealthTheme.Luminance(surface) < 0.179;
            var foreground = GetValue(ThemeForegroundProperty) as Brush ??
                MakeBrush(dark ? "#F5F5F7" : "#1D1D1F");
            SetValue(TextElement.ForegroundProperty, foreground);
            var highContrast = (bool)GetValue(HighContrastProperty);
            foreach (var grade in grades)
            {
                // Under high contrast the server sends no colour at all, so
                // there is normally nothing here to override. This still
                // honours it, for the case where the theme turned on after a
                // coloured answer was received: the letter and its bold weight
                // carry the grade without relying on hue.
                grade.Key.Foreground = highContrast ? foreground : MakeBrush(grade.Value);
            }
        }

        private static SolidColorBrush MakeBrush(string color) =>
            MakeBrush((Color)ColorConverter.ConvertFromString(color));

        private static SolidColorBrush MakeBrush(Color color)
        {
            var brush = new SolidColorBrush(color);
            brush.Freeze();
            return brush;
        }
    }
}
