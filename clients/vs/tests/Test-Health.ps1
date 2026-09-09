# Run with Windows PowerShell 5.1 against an already-open experimental VS instance.
param([int]$VisualStudioProcessId, [string]$RootSuffix = "Exp")
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -ne 'Desktop') {
    throw 'Run this test with Windows PowerShell (powershell.exe), not pwsh.'
}
$instances = @(Get-CimInstance Win32_Process -Filter "Name='devenv.exe'" |
    Where-Object { $_.CommandLine -match ('/RootSuffix\s+' + [regex]::Escape($RootSuffix) + '\b') -and
        (!$VisualStudioProcessId -or $_.ProcessId -eq $VisualStudioProcessId) })
if ($instances.Count -ne 1) {
    throw 'Open lspf-analysis.sln and press F5 first. If multiple experimental instances are open, pass -VisualStudioProcessId.'
}
$instance = $instances[0]
$assemblies = Join-Path (Split-Path $instance.ExecutablePath) 'PublicAssemblies'
$references = @((Join-Path $assemblies 'envdte.dll'), (Join-Path $assemblies 'Microsoft.VisualStudio.Interop.dll'), (Join-Path $assemblies 'envdte80.dll'))
foreach ($reference in $references) { Add-Type -Path $reference }
Add-Type -ReferencedAssemblies ($references + "System.Core") -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Linq;
using System.Runtime.InteropServices.ComTypes;
public static class LspfVsSmoke {
    [DllImport("ole32.dll")] private static extern int GetRunningObjectTable(int reserved, out IRunningObjectTable table);
    [DllImport("ole32.dll")] private static extern int CreateBindCtx(int reserved, out IBindCtx context);
    private static EnvDTE.DTE Connect(int pid) {
        IRunningObjectTable table; Marshal.ThrowExceptionForHR(GetRunningObjectTable(0, out table));
        IBindCtx context; Marshal.ThrowExceptionForHR(CreateBindCtx(0, out context));
        IEnumMoniker iterator; table.EnumRunning(out iterator);
        try {
            var monikers = new IMoniker[1];
            while (iterator.Next(1, monikers, IntPtr.Zero) == 0) {
                try {
                    string name; monikers[0].GetDisplayName(context, null, out name);
                    if (name.StartsWith("!VisualStudio.DTE.", StringComparison.Ordinal) && name.EndsWith(":" + pid, StringComparison.Ordinal)) {
                        object result; table.GetObject(monikers[0], out result);
                        var dte = (EnvDTE.DTE)result;
                        // Keep the interactive IDE alive when automation releases its references.
                        dte.UserControl = true;
                        return dte;
                    }
                } finally { Marshal.ReleaseComObject(monikers[0]); }
            }
            throw new InvalidOperationException("Experimental VS automation object is not registered yet.");
        } finally { Marshal.ReleaseComObject(iterator); Marshal.ReleaseComObject(context); Marshal.ReleaseComObject(table); }
    }
    public static void Command(int pid, int id) {
        var dte = Connect(pid);
        try { object input = null, output = null; dte.Commands.Raise("{FE1B9436-D20D-4608-961C-A9BEA967940E}", id, ref input, ref output); }
        finally { Marshal.ReleaseComObject(dte); }
    }
    public static string Read(int pid) {
        var dte = Connect(pid);
        try {
            var window = dte.Windows.Item(EnvDTE.Constants.vsWindowKindOutput);
            var output = (EnvDTE.OutputWindow)window.Object;
            if (!window.Visible || output.ActivePane.Name != "LSPF Analysis")
                throw new InvalidOperationException("LSPF Analysis output is not visible and selected.");
            var doc = output.OutputWindowPanes.Item("LSPF Analysis").TextDocument;
            return doc.StartPoint.CreateEditPoint().GetText(doc.EndPoint);
        } finally { Marshal.ReleaseComObject(dte); }
    }
    public static void Open(int pid, string path) {
        var dte = Connect(pid);
        try { dte.ItemOperations.OpenFile(path); }
        finally { Marshal.ReleaseComObject(dte); }
    }
    public static void Replace(int pid, string path, string text) {
        var dte = Connect(pid);
        try {
            var doc = dte.Documents.Item(path);
            var editor = (EnvDTE.TextDocument)doc.Object("TextDocument");
            var edit = editor.StartPoint.CreateEditPoint();
            edit.Delete(editor.EndPoint);
            edit.Insert(text);
            doc.Activate();
        } finally { Marshal.ReleaseComObject(dte); }
    }
    public static void QuickInfo(int pid, string path) {
        var dte = Connect(pid);
        try {
            var doc = dte.Documents.Item(path);
            doc.Activate();
            ((EnvDTE.TextSelection)doc.Selection).MoveToLineAndOffset(1, 5);
            dte.ExecuteCommand("Edit.QuickInfo");
        } finally { Marshal.ReleaseComObject(dte); }
    }
    public static int Errors(int pid, string path) {
        var dte = Connect(pid);
        try {
            var items = ((EnvDTE80.DTE2)dte).ToolWindows.ErrorList.ErrorItems;
            int count = 0;
            for (int i = 1; i <= items.Count; i++)
                if (string.Equals(items.Item(i).FileName, path, StringComparison.OrdinalIgnoreCase)) count++;
            return count;
        } finally { Marshal.ReleaseComObject(dte); }
    }
    public static void CloseDocument(int pid, string path) {
        var dte = Connect(pid);
        try { dte.Documents.Item(path).Close(EnvDTE.vsSaveChanges.vsSaveChangesNo); }
        finally { Marshal.ReleaseComObject(dte); }
    }
    public static void Close(int pid) {
        var dte = Connect(pid);
        try {
            if (!dte.Solution.Saved) throw new InvalidOperationException("Refusing to close an unsaved solution.");
            foreach (EnvDTE.Document doc in dte.Documents)
                if (!doc.Saved) throw new InvalidOperationException("Refusing to close unsaved documents.");
            dte.Quit();
        } finally { Marshal.ReleaseComObject(dte); }
    }
}
'@

function Invoke-Vs([scriptblock]$Operation) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        try { return & $Operation }
        catch {
            $cause = $_.Exception.GetBaseException()
            if ($cause.HResult -notin @(-2147418111, -2147417846) -or [DateTime]::UtcNow -ge $deadline) { throw }
            Start-Sleep -Milliseconds 200
        }
    } while ($true)
}


Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Get-HealthNames {
    $root = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, [int]$instance.ProcessId))
    if (!$root) { return '' }
    return ($root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name }) -join "`n"
}
function Wait-Health([string]$Pattern) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $names = Get-HealthNames
        if ($names -match $Pattern) { Write-Host "PASS UI $Pattern"; return }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "UI did not match '$Pattern':`n$names"
}
function Wait-Errors([bool]$Expected, [string]$Path) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $count = Invoke-Vs { [LspfVsSmoke]::Errors($instance.ProcessId, $Path) }
        if (($count -gt 0) -eq $Expected) { Write-Host "PASS Error List count $count"; return }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Unexpected Error List count: $count"
}
$fixtureDir = Join-Path $PSScriptRoot 'obj'
[IO.Directory]::CreateDirectory($fixtureDir) | Out-Null
$fixtureId = [Guid]::NewGuid().ToString('N')
$sample = Join-Path $fixtureDir ($fixtureId + '.rs')
$unsupported = Join-Path $fixtureDir ($fixtureId + '.txt')
$opened = @()
try {
    [IO.File]::WriteAllText($sample, "fn add(a: u32, b: u32) -> u32 { a + b }`r`nfn one() -> u32 { 1 }`r`n")
    [IO.File]::WriteAllText($unsupported, 'Unsupported document')
    Invoke-Vs { [LspfVsSmoke]::Open($instance.ProcessId, $sample) }
    $opened += $sample
    Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 0x0104) }
    Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 0x0100) }
    Wait-Health 'LSPF  [0-9]+%.*2 (functions|个函数)'
    Wait-Health 'add  [0-9]+%'
    Invoke-Vs { [LspfVsSmoke]::QuickInfo($instance.ProcessId, $sample) }
    Wait-Health 'cognitive complexity|认知复杂度'
    Invoke-Vs { [LspfVsSmoke]::Replace($instance.ProcessId, $sample, "fn changed() -> u32 { 1 }`r`n") }
    Wait-Health 'LSPF  [0-9]+%.*1 (functions|个函数)'
    Wait-Health 'changed  [0-9]+%'
    $beforeRestart = Invoke-Vs { [LspfVsSmoke]::Read($instance.ProcessId) }
    Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 0x0103) }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $afterRestart = Invoke-Vs { [LspfVsSmoke]::Read($instance.ProcessId) }
        if ($afterRestart.Substring($beforeRestart.Length) -match 'Language server initialized\.') { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($afterRestart.Substring($beforeRestart.Length) -notmatch 'Language server initialized\.') { throw 'Restart did not initialize a new session.' }
    Write-Host 'PASS restart initialized a new session'
    Wait-Health 'changed  [0-9]+%'
    Invoke-Vs { [LspfVsSmoke]::Replace($instance.ProcessId, $sample, "// empty`r`n") }
    Wait-Health 'No functions to analyze|没有可分析的函数'
    $poor = "fn poor(x: i32) {`r`n" + ("if x > 0 {`r`n" * 35) + 'println!("poor");' + ("}`r`n" * 35) + "}`r`n"
    Invoke-Vs { [LspfVsSmoke]::Replace($instance.ProcessId, $sample, $poor) }
    Wait-Health 'poor  [0-9]+%'
    Wait-Errors $true $sample
    Invoke-Vs { [LspfVsSmoke]::Open($instance.ProcessId, $unsupported) }
    $opened += $unsupported
    Wait-Health 'Open a supported source file|请打开支持的源文件'
    Invoke-Vs { [LspfVsSmoke]::CloseDocument($instance.ProcessId, $unsupported) }
    $opened = @($opened | Where-Object { $_ -ne $unsupported })
    Invoke-Vs { [LspfVsSmoke]::CloseDocument($instance.ProcessId, $sample) }
    $opened = @($opened | Where-Object { $_ -ne $sample })
    Wait-Errors $false $sample
    Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 0x0101) }
    Wait-Health 'Language server stopped|语言服务器已停止'
    Write-Host 'PASS native VS health integration'
} finally {
    foreach ($path in $opened) { Invoke-Vs { [LspfVsSmoke]::CloseDocument($instance.ProcessId, $path) } }
    foreach ($path in @($sample, $unsupported)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path }
    }
}
