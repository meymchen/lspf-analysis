# Run with Windows PowerShell 5.1 against an already-open experimental VS instance.
param([int]$VisualStudioProcessId, [switch]$VerifyIdeExit, [switch]$Tcp, [switch]$WebSocket, [ValidateRange(1, 65535)][int]$TcpPort = 9257, [string]$RootSuffix = "Exp")
$ErrorActionPreference = 'Stop'
if ($Tcp -and $WebSocket) { throw 'Choose either -Tcp or -WebSocket.' }
$network = $Tcp -or $WebSocket
$transport = if ($WebSocket) { 'ws' } else { 'tcp' }
$transportLabel = if ($WebSocket) { 'WebSocket' } else { 'TCP' }
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
$references = @((Join-Path $assemblies 'envdte.dll'), (Join-Path $assemblies 'Microsoft.VisualStudio.Interop.dll'))
foreach ($reference in $references) { Add-Type -Path $reference }
Add-Type -ReferencedAssemblies $references -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
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

function Wait-ForLog([string]$Pattern, [int]$Offset) {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        $output = Invoke-Vs { [LspfVsSmoke]::Read($instance.ProcessId) }
        $recent = $output.Substring($Offset)
        if ($recent -match '(?i)failed|Terminating language server') { throw $recent }
        if ($recent -match $Pattern) { return $output }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for '$Pattern'. Output:`n$output"
}

function Start-NetworkServer {
    # Refuse to attach the test to an unrelated listener. Do not probe by
    # connecting: lspf-analysis accepts only one TCP client per process.
    $probe = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, $TcpPort)
    try { $probe.Start() } finally { $probe.Stop() }
    $binary = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../target/debug/lspf-analysis.exe'))
    $info = New-Object Diagnostics.ProcessStartInfo($binary, "serve --$transport 127.0.0.1:$TcpPort")
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables.Remove('LSPF_ANALYSIS_LOG_FILE')
    $child = New-Object Diagnostics.Process
    $child.StartInfo = $info
    $child.EnableRaisingEvents = $true
    if (!$child.Start()) { throw 'Could not start external TCP server.' }
    $null = $child.StandardError.ReadToEndAsync()
    return $child
}

# First stop makes the test repeatable without closing the experimental IDE.
Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 257) }
$baseline = Invoke-Vs { [LspfVsSmoke]::Read($instance.ProcessId) }
$ready = 'Language server initialized\. Visual Studio ILanguageClient reports the server is ready\.'
for ($cycle = 1; $cycle -le 2; $cycle++) {
    $external = $null
    try {
        if ($network) { $external = Start-NetworkServer }
        $startCommand = if ($WebSocket) { 262 } elseif ($Tcp) { 258 } else { 256 }
        Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, $startCommand) }
        $output = Wait-ForLog $ready $baseline.Length
        if ($network) {
            $recent = $output.Substring($baseline.Length)
            if ($recent -notmatch "$transportLabel connected to 127\.0\.0\.1:$TcpPort" -or $recent -match 'Starting language server \(PID') {
                throw 'Network mode did not attach exclusively to the external server.'
            }
            $serverPid = $external.Id
        } else {
            $started = [regex]::Match($output.Substring($baseline.Length), 'Starting language server \(PID (\d+)\)')
            if (!$started.Success) { throw 'No server PID was logged.' }
            $serverPid = [int]$started.Groups[1].Value
        }
        if (!(Get-Process -Id $serverPid -ErrorAction SilentlyContinue)) { throw 'Initialized server is not running.' }
        Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, $startCommand) }
        $output = Wait-ForLog 'already running or initializing' $output.Length
        Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 257) }
        $stopPattern = if ($network) { "$transportLabel debug connection stopped by Visual Studio" } else { 'Language server stopped \(exit code 0\)' }
        $baseline = Wait-ForLog $stopPattern $output.Length
        if ($network -and (!$external.WaitForExit(5000) -or $external.ExitCode -ne 0)) { throw 'External network server did not exit gracefully.' }
        if (Get-Process -Id $serverPid -ErrorAction SilentlyContinue) { throw "Server PID $serverPid survived shutdown." }
        Write-Output "PASS cycle ${cycle}: native ILanguageClient initialized PID $serverPid, duplicate start ignored, graceful exit, Output visible."
    } finally {
        if ($external) {
            if (!$external.HasExited) { $external.Kill(); $null = $external.WaitForExit(5000) }
            $external.Dispose()
        }
    }
}
Write-Output $baseline
if ($VerifyIdeExit) {
    Invoke-Vs { [LspfVsSmoke]::Command($instance.ProcessId, 256) }
    $output = Wait-ForLog $ready $baseline.Length
    $serverPid = [int][regex]::Match($output.Substring($baseline.Length), 'Starting language server \(PID (\d+)\)').Groups[1].Value
    $server = Get-Process -Id $serverPid
    try {
        $server.EnableRaisingEvents = $true
        Invoke-Vs { [LspfVsSmoke]::Close($instance.ProcessId) }
        if (!$server.WaitForExit(15000) -or $server.ExitCode -ne 0) { throw 'IDE exit did not gracefully stop the server.' }
        Write-Output "PASS: closing the experimental IDE gracefully stopped PID $serverPid (exit code 0)."
    } finally { $server.Dispose() }
}
