param(
    [string]$AssemblyPath = "$PSScriptRoot/../bin/Debug/LspfAnalysis.dll",
    [string]$VisualStudioDirectory = 'C:/Program Files/Microsoft Visual Studio/18/Community/Common7/IDE',
    [string]$RenderPath
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -ne 'Desktop' -or [Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') {
    throw 'Run with powershell.exe -NoProfile -Sta -File clients/vs/tests/Test-HoverLayout.ps1.'
}
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
$shellAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'PublicAssemblies/Microsoft.VisualStudio.Shell.15.0.dll'))
$frameworkAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'PublicAssemblies/Microsoft.VisualStudio.Shell.Framework.dll'))
$textUiAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'CommonExtensions/Microsoft/Editor/Microsoft.VisualStudio.Text.UI.dll'))
$immutableAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'PublicAssemblies/System.Collections.Immutable.dll'))
# Match the VS host's binding of SDK 17 references to its installed shell.
$resolveShell = [ResolveEventHandler] {
    param($sender, $eventArgs)
    switch ([Reflection.AssemblyName]::new($eventArgs.Name).Name) {
        'Microsoft.VisualStudio.Shell.15.0' { return $shellAssembly }
        'Microsoft.VisualStudio.Shell.Framework' { return $frameworkAssembly }
        'Microsoft.VisualStudio.Text.UI' { return $textUiAssembly }
        'System.Collections.Immutable' { return $immutableAssembly }
    }
    return $null
}
[AppDomain]::CurrentDomain.add_AssemblyResolve($resolveShell)
$assembly = [Reflection.Assembly]::LoadFrom([IO.Path]::GetFullPath($AssemblyPath))
$diagnosticMethod = $assembly.GetType('LspfAnalysis.DiagnosticHover').GetMethod('Create', [Reflection.BindingFlags]'Static,NonPublic')
foreach ($case in @(
    @{ Message = '函数 `register_scene_types`：质量 2%（较差）。'; Text = '函数 register_scene_types：质量 2%（较差）。'; Code = @('register_scene_types') },
    @{ Message = 'function `add`: quality 100% (good)'; Text = 'function add: quality 100% (good)'; Code = @('add') },
    @{ Message = '`first` and `second`'; Text = 'first and second'; Code = @('first', 'second') },
    @{ Message = 'plain summary'; Text = 'plain summary'; Code = @() },
    @{ Message = 'function `unfinished'; Text = 'function `unfinished'; Code = @() },
    @{ Message = ''; Text = ''; Code = @() }
)) {
    $element = $diagnosticMethod.Invoke($null, @($case.Message))
    if ($element.GetType().FullName -ne 'Microsoft.VisualStudio.Text.Adornments.ClassifiedTextElement') {
        throw 'Diagnostic tooltip must supply a native VS presentation model.'
    }
    $actualText = ($element.Runs | ForEach-Object { $_.Text }) -join ''
    $codeRuns = @($element.Runs | Where-Object { $_.Style.ToString() -eq 'UseClassificationFont' } | ForEach-Object { $_.Text })
    if ($actualText -cne $case.Text -or ($codeRuns -join '|') -cne ($case.Code -join '|')) {
        throw "Diagnostic inline code mismatch: $($case.Message)"
    }
}
Write-Host 'PASS: diagnostic summaries preserve text and format inline code using the editor font.'
$method = $assembly.GetType('LspfAnalysis.HealthHoverView').GetMethod('Create', [Reflection.BindingFlags]'Static,NonPublic')
$markdown = @'
**`register_scene_types`** · 质量 **2%** · 较差

| 等级 | 支柱 | 指标 | 数值 |
| :-: | :-- | :-- | --: |
| C | 控制流 | 认知复杂度 | 17 / 15 |
| C | | 圈复杂度 | 17 / 10 |
| D | 规模 | 语句数 | 729 / 30 |
| D | 词汇负担 | 工作记忆 | 138 / 8 |
| C | | Halstead 难度 | 17 / 12 |
| A | 接口 | 参数个数 | 0 / 4 |
| B | 接口 | 示例指标 | 2 / 4 |

最弱：规模——语句数 729，阈值 30，得分 0%（较差）。
'@
$view = $method.Invoke($null, @($markdown))
$hostPanel = New-Object Windows.Controls.StackPanel
$hostPanel.Children.Add($view) | Out-Null
# A diagnostic supplied by another Quick Info provider widens the shared popup.
$diagnostic = New-Object Windows.Controls.TextBlock
$diagnostic.Text = ('很长的诊断总结，包含质量和各项指标。' * 10)
$diagnostic.Width = 1200
$diagnostic.TextWrapping = 'Wrap'
$hostPanel.Children.Add($diagnostic) | Out-Null
$foregroundKey = [Microsoft.VisualStudio.PlatformUI.EnvironmentColors]::ToolTipTextBrushKey
$backgroundKey = [Microsoft.VisualStudio.PlatformUI.EnvironmentColors]::ToolTipBrushKey
function Get-Luminance($Color) {
    $channels = @($Color.R, $Color.G, $Color.B) | ForEach-Object {
        $value = $_ / 255.0
        if ($value -le 0.04045) { $value / 12.92 } else { [Math]::Pow(($value + 0.055) / 1.055, 2.4) }
    }
    return 0.2126 * $channels[0] + 0.7152 * $channels[1] + 0.0722 * $channels[2]
}
$failures = New-Object 'Collections.Generic.List[string]'
$darkGrades = $null
foreach ($theme in @(
    @{ Name = 'dark'; Foreground = '#FFF1F1F1'; Background = '#FF2B2B2B' },
    @{ Name = 'light'; Foreground = '#FF1E1E1E'; Background = '#FFF5F5F5' },
    @{ Name = 'custom dark'; Foreground = '#FFF1F1F1'; Background = '#FF606060' },
    @{ Name = 'custom light'; Foreground = '#FF000000'; Background = '#FF999999' },
    @{ Name = 'high contrast'; Foreground = '#FFFFFF00'; Background = '#FF000000' },
    @{ Name = 'dark again'; Foreground = '#FFF1F1F1'; Background = '#FF2B2B2B' },
    @{ Name = 'unknown'; Foreground = '#FFF5F5F7'; Background = '#FF2B2B2B' }
)) {
    $brush = [Windows.Media.BrushConverter]::new().ConvertFromString($theme.Foreground)
    $hostPanel.Background = [Windows.Media.BrushConverter]::new().ConvertFromString($theme.Background)
    $highContrast = $theme.Name -eq 'high contrast'
    $hostPanel.Resources[[Windows.SystemParameters]::HighContrastKey] = $highContrast
    if ($theme.Name -eq 'unknown') {
        $hostPanel.Resources.Remove($backgroundKey)
        $hostPanel.Resources.Remove($foregroundKey)
    } else {
        $hostPanel.Resources[$foregroundKey] = $brush
        $hostPanel.Resources[$backgroundKey] = $hostPanel.Background
    }
    $diagnostic.Foreground = $brush
    $hostPanel.Measure([Windows.Size]::new(1200, [double]::PositiveInfinity))
    $hostPanel.Arrange([Windows.Rect]::new(0, 0, 1200, $hostPanel.DesiredSize.Height))
    $hostPanel.UpdateLayout()
    $left = $view.TranslatePoint([Windows.Point]::new(0, 0), $hostPanel).X
    if ([Math]::Abs($left) -gt 0.1) { $failures.Add("$($theme.Name): hover starts at x=$left instead of x=0") }
    $texts = @($view.Children | ForEach-Object {
        if ($_ -is [Windows.Controls.Grid]) { $_.Children } else { $_ }
    })
    $gradeColors = @()
    $minimumContrast = 100.0
    foreach ($text in $texts) {
        $label = [Windows.Documents.TextRange]::new($text.ContentStart, $text.ContentEnd).Text
        if ($label -cmatch '^[ABCD]$') {
            if ($text.FontWeight -ne [Windows.FontWeights]::Bold) { $failures.Add("$($theme.Name): grade is not bold") }
            $gradeColors += $text.Foreground.ToString()
            $a = Get-Luminance $text.Foreground.Color
            $b = Get-Luminance $hostPanel.Background.Color
            $contrast = ([Math]::Max($a, $b) + 0.05) / ([Math]::Min($a, $b) + 0.05)
            $minimumContrast = [Math]::Min($minimumContrast, $contrast)
            if ($contrast -lt 4.5) { $failures.Add("$($theme.Name): $label contrast $contrast is below 4.5") }
            if (!$highContrast) { continue }
        }
        if ($text.Foreground.ToString() -ne $brush.ToString()) {
            $failures.Add("$($theme.Name): text foreground $($text.Foreground) does not follow $brush")
            break
        }
    }
    if ($theme.Name -eq 'dark') { $darkGrades = $gradeColors -join ',' }
    if ($theme.Name -eq 'light' -and ($gradeColors -join ',') -eq $darkGrades) { $failures.Add('Light theme did not change the palette') }
    if ($theme.Name -in @('unknown', 'dark again') -and ($gradeColors -join ',') -ne $darkGrades) { $failures.Add("$($theme.Name): expected the dark palette") }
    if (!$highContrast -and @($gradeColors | Sort-Object -Unique).Count -ne 4) { $failures.Add("$($theme.Name): expected four distinct grade colors") }
    Write-Host "$($theme.Name): left=$left, title=$($texts[0].Foreground), minimum grade contrast=$([Math]::Round($minimumContrast, 2)):1"
    if ($RenderPath -and $theme.Name -eq 'dark') {
        $bitmap = [Windows.Media.Imaging.RenderTargetBitmap]::new(1200, [int][Math]::Ceiling($hostPanel.ActualHeight), 96, 96, [Windows.Media.PixelFormats]::Pbgra32)
        $bitmap.Render($hostPanel)
        $encoder = New-Object Windows.Media.Imaging.PngBitmapEncoder
        $encoder.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($bitmap))
        $stream = [IO.File]::Create([IO.Path]::GetFullPath($RenderPath))
        try { $encoder.Save($stream) } finally { $stream.Dispose() }
    }
}
if ($failures.Count) { throw ($failures -join "`n") }
Write-Host 'PASS: alignment, bold colored grades, live theme changes, contrast, high contrast, and unknown-theme dark fallback.'
