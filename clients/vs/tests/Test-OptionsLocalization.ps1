param(
    [string]$Culture = 'zh-CN',
    [string]$AssemblyPath = "$PSScriptRoot/../bin/Debug/LspfAnalysis.dll",
    [string]$VisualStudioDirectory = 'C:/Program Files/Microsoft Visual Studio/18/Community/Common7/IDE'
)
$ErrorActionPreference = 'Stop'
[Threading.Thread]::CurrentThread.CurrentUICulture = [Globalization.CultureInfo]::GetCultureInfo($Culture)
$shellAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'PublicAssemblies/Microsoft.VisualStudio.Shell.15.0.dll'))
$frameworkAssembly = [Reflection.Assembly]::LoadFrom((Join-Path $VisualStudioDirectory 'PublicAssemblies/Microsoft.VisualStudio.Shell.Framework.dll'))
$resolve = [ResolveEventHandler] {
    param($sender, $eventArgs)
    switch ([Reflection.AssemblyName]::new($eventArgs.Name).Name) {
        'Microsoft.VisualStudio.Shell.15.0' { return $shellAssembly }
        'Microsoft.VisualStudio.Shell.Framework' { return $frameworkAssembly }
    }
    return $null
}
[AppDomain]::CurrentDomain.add_AssemblyResolve($resolve)
$assembly = [Reflection.Assembly]::LoadFrom([IO.Path]::GetFullPath($AssemblyPath))
$resources = [Resources.ResourceManager]::new('LspfAnalysis.VSPackage', $assembly)
$pageName = $resources.GetString('110', [Globalization.CultureInfo]::CurrentUICulture)
if ($pageName -cne $(if ($Culture -in @('zh-CN', 'zh-TW')) { '常规' } else { 'General' })) {
    throw "Unexpected localized page title: $pageName"
}
$type = $assembly.GetType('LspfAnalysis.AnalysisOptions')
$properties = [ComponentModel.TypeDescriptor].GetMethod('GetProperties', [type[]]@([type])).Invoke($null, @($type))
$chinese = $Culture.StartsWith('zh')
$count = 0
for ($index = 0; $index -lt $properties.Count; $index++) {
    $property = $properties[$index]
    if ($property.ComponentType -ne $type) { continue }
    foreach ($label in @($property.DisplayName, $property.Category, $property.Description)) {
        if (!$label -or ($label -match '[\u4e00-\u9fff]') -ne $chinese) {
            throw "Untranslated $($property.Name): $label"
        }
    }
    if ($property.PropertyType -eq [bool] -or $property.Name -eq 'Trace') {
        $converter = $property.Converter
        foreach ($value in $converter.GetStandardValues()) {
            $display = $converter.ConvertTo($null, [Globalization.CultureInfo]::CurrentCulture, $value, [string])
            if (($display -match '[\u4e00-\u9fff]') -ne $chinese) { throw "Untranslated value: $display" }
            if ($converter.ConvertFrom($null, [Globalization.CultureInfo]::CurrentCulture, $display) -ne $value) { throw 'Display round trip failed' }
            $stored = $converter.ConvertToInvariantString($value)
            if ($stored -cne $value.ToString() -or $converter.ConvertFromInvariantString($stored) -ne $value) { throw 'Serialized setting changed' }
        }
    }
    $count++
}
if ($count -ne 23) { throw "Expected 23 options; checked $count" }
Write-Host "PASS: $Culture, $count localized option names/categories/descriptions, dropdown round trips, invariant persistence."
