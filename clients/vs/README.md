# LSPF Analysis for Visual Studio

A Windows x64 VSIX for Visual Studio 2026 (primary development host) and
Visual Studio 2022. It uses the same analysis server, scoring, diagnostics, and
health protocol as the VS Code and IntelliJ clients.

## Analyze code

Open a local C++, Java, JavaScript/JSX, Python, Rust, or TypeScript/TSX source
file. Analysis starts automatically and updates after edits, including unsaved
changes. The supported extensions are `.cpp`, `.cc`, `.cxx`, `.h`, `.hpp`,
`.hxx`, `.java`, `.js`, `.mjs`, `.cjs`, `.jsx`, `.py`, `.pyw`, `.rs`, `.ts`,
`.mts`, `.cts`, and `.tsx`. C# is not supported by the analysis server.

- Diagnostics appear as editor squiggles and in **Error List**. Double-click an
  entry to navigate to its source line.
- Hover over a function or class name to see its health score and metric table.
  This contributes alongside the language's existing Quick Info provider.
- The editor's bottom margin shows file health, function count, and the number
  below the warning threshold. Click it to open **Code Health**.
- **Tools > LSPF Analysis: Code Health** opens a tree of functions, pillars,
  and measurements. Sort by lowest quality or source position. Double-click a
  function, or press Enter on it, to navigate. The file summary also offers
  shortcuts to the weakest functions, settings, and Error List.

Closing a file clears its diagnostics and health data. Switching to an
unsupported file clears the function view. A server restart reopens current
editor buffers, preserving unsaved text. Source file navigation is restricted
to local documents already tracked by the extension.

## Settings

Open **Tools > Options > LSPF Analysis > General**, or use the Settings button
in Code Health. Health thresholds, weights, diagnostic options, and
protocol tracing apply to the running server. Changing **Server executable**
takes effect at the next restart; an empty path uses the bundled binary.

The options page follows Visual Studio's UI language. Chinese uses translated
option names, categories, descriptions, dropdown values, and validation messages;
other languages fall back to English. Restart Visual Studio after changing its
UI language. Setting names and serialized values remain language-independent.

The health settings match the other clients: cognitive and cyclomatic
complexity, length, working memory, Halstead difficulty, parameter count,
class design thresholds, the five pillar weights, and warning/error quality
cutoffs. Thresholds must be positive; weights may be zero; quality cutoffs
must be in 0–100 with the error cutoff no higher than warning.

Diagnostics can be disabled entirely, extended to individual metrics, or
extended to whole files. **File health summary** controls the editor's bottom
margin. The function sort order is saved. **Protocol trace** offers Off,
Messages, and Verbose in the LSPF Analysis Output pane.

The health UI and server messages follow the IDE's English or Chinese display
language. Settings property names and Tools command titles use English.

## Debug from the repository root

Install Visual Studio with the **Visual Studio extension development** workload.
NuGet restores the .NET Framework 4.8 reference assemblies. Building the server
also requires the Rust MSVC toolchain specified by the repository and the C++
build tools.
Cargo must be on the PATH inherited by Visual Studio.

1. Open `lspf-analysis.sln` at the repository root in Visual Studio 2026 or 2022.
2. Select **Debug / Any CPU** and press **F5**. The build runs Cargo, bundles
   `target/debug/lspf-analysis.exe`, and deploys the extension to the `Exp`
   experimental instance of the Visual Studio version you are using.
3. Open a supported source file to start analysis. You can also choose
   **Tools > LSPF Analysis: Start language server**. The command opens **Output**
   and selects **LSPF Analysis**. The timestamped log shows the executable path,
   arguments, process ID, and handshake
   progress. `Language server initialized.` confirms the LSP handshake completed
   and the server is ready. This log comes from Visual Studio's
   `OnServerInitializedAsync` callback.
4. Choose **Tools > LSPF Analysis: Stop language server**. The output should
   report `Language server stopped (exit code 0).` A manual stop keeps analysis
   stopped when more files open; Start or Restart enables it again.

Closing the experimental IDE stops the server in the package's `QueryClose`
callback, while Visual Studio's LSP services are still available. Disposal also
cleans up any remaining owned process.
Close that IDE normally when checking shutdown; forcibly ending the debugger
does not guarantee package disposal. Set C# breakpoints in `AnalysisLanguageClient.cs`
or `ServerProcess.cs`. To debug Rust, attach the native debugger to the server
PID printed in Output.

There is one server per IDE instance. Opening a supported editor or using a
start command loads it.
`ILanguageClientBroker.LoadAsync` registers the client with Visual Studio;
`ActivateAsync` launches the process and returns its streams in a `Connection`.
Visual Studio owns the LSP handshake and shutdown sequence. The extension raises
the client's `StartAsync` / `StopAsync` events and logs initialization callbacks.
After the native client stops, the process has five seconds to exit before the
extension terminates it. Server stderr and lifecycle errors appear in Output.

The broker uses a private content type without file associations. MEF editor
providers observe text buffers, contribute Quick Info and error tags, and
send document notifications through the broker's `StreamJsonRpc.JsonRpc`
connection. This preserves the existing language services for C++, Python,
JavaScript, and the other supported editors. The custom message target receives
`lspfAnalysis/fileHealth`; a middle layer routes standard diagnostics to the
extension's tags and Error List provider. Visual Studio still owns LSP
initialization, shutdown, and exit. See [Microsoft's LSP extension guide][lsp]
and [ILanguageClientBroker][broker].

Changes are coalesced for 250 ms and sent as full-text updates with increasing
versions. File closure and renames send `didClose`; a restart clears cached
results and resends open buffers. Function requests have a ten-second timeout,
and hover requests a five-second timeout. Late replies are checked against the
connection generation, configuration revision, document URI, and text snapshot.
Server Markdown is rendered as WPF text and tables; HTML and command links are
never executed.

Hover text follows the VS tooltip text resource. Bold A/B/C/D grades use green,
blue, amber, and red families with separate light and dark variants, following
[Apple's color guidance](https://developer.apple.com/design/human-interface-guidelines/color).
The active VS tooltip background determines the appearance, including custom
themes; unavailable or ambiguous resources default to the dark palette. Dynamic
resource references update an open hover when the theme changes. Grade colors
are adjusted to at least 4.5:1 contrast against that background. High-contrast
mode uses the system text color while keeping the bold grade letters.

[lsp]: https://learn.microsoft.com/en-us/visualstudio/extensibility/adding-an-lsp-extension
[broker]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.visualstudio.languageserver.client.ilanguageclientbroker

## TCP debugging

Start the server separately from the repository root, either in a Rust debugger
or in a terminal:

```powershell
cargo build --locked -p lspf-analysis --target-dir target
& ./target/debug/lspf-analysis.exe serve --tcp 127.0.0.1:9257
```

Press F5 in Visual Studio, then choose **Tools > LSPF Analysis: Connect to TCP
debug server** in the experimental instance. Output shows the address, connection
progress, and `Language server initialized.` when the native client completes
the handshake. You can attach a native debugger to the separately launched
server and debug the extension and server independently.

The TCP command uses port **9257** by default. To change it, set
`LSPF_ANALYSIS_DEBUG_PORT` before launching the host Visual Studio so its
experimental instance inherits the value. Use the same port in `serve --tcp`.
Invalid ports produce an error. Connections are restricted to `127.0.0.1` and
retry for up to 20 seconds, allowing the server debugger time to start listening.
An unavailable server produces a timeout with the command needed to start it;
there is no fallback to launching a stdio server.

**Start language server** still launches the bundled stdio server.
**Stop language server** stops either transport through the native LSP client.
Stop the current session before switching transports. In TCP mode the extension
owns only the socket: it never launches or forcibly kills the external process.
The server normally exits after the LSP shutdown sequence. Start a fresh server
before reconnecting; each `serve --tcp` process accepts one LSP connection.
Server stderr stays in the external terminal or debugger; connection and LSP
initialization messages appear in Visual Studio's **LSPF Analysis** Output pane.

## WebSocket debugging

Start an external server with `lspf-analysis serve --ws 127.0.0.1:9257`, then
choose **Tools > LSPF Analysis: Connect to WebSocket debug server**. This uses
the same port setting and 20-second connection timeout as TCP. Stop the current
session before changing transports. The WebSocket connection is restricted to
loopback and does not use the system HTTP proxy.

The adapter converts between the native client's Content-Length framing and
WebSocket JSON text messages. Reads and writes are serialized separately, as
required by [ClientWebSocket][websocket]. The external process remains owned by
its launcher. Restart retains the selected transport; restart an external
server before reconnecting because it accepts one LSP session.

[websocket]: https://learn.microsoft.com/en-us/dotnet/api/system.net.websockets.clientwebsocket.receiveasync

## Build and install

From a Visual Studio Developer PowerShell at the repository root:

```powershell
msbuild lspf-analysis.sln /restore /p:Configuration=Release /p:DeployExtension=false
```

Install `clients/vs/bin/Release/LspfAnalysis.vsix` through Visual Studio's VSIX
Installer. Release builds bundle the Cargo release binary. Debug builds bundle
the debug binary; both builds explicitly use the repository's `target` directory.
Users installing the VSIX do not need Rust.

The extension targets .NET Framework 4.8 and VS SDK 17.0 APIs. Its manifest uses
the `[17.0,)` installation range and `amd64` architecture, covering Community,
Professional, and Enterprise on x64. Microsoft documents VS 2026's compatibility
with VS 2022 extensions in [Extension compatibility][compatibility].
ARM64 packaging is not included.

[compatibility]: https://learn.microsoft.com/en-us/visualstudio/extensibility/migration/extension-compatibility

## Test the native client lifecycle

Open the root solution and press F5. With the experimental instance open, run
this command from the repository root using **Windows PowerShell 5.1**:

```powershell
powershell.exe -NoProfile -File clients/vs/tests/Test-Lifecycle.ps1
```

The test invokes the extension's real commands through Visual Studio automation.
It checks two startup/shutdown cycles, the native initialization callback,
duplicate starts, Output visibility, exit code zero, and absence of surviving
server processes. It leaves the experimental IDE open with the server stopped.
Use `-VisualStudioProcessId <pid>` if multiple experimental instances are open.
Add `-VerifyIdeExit` to also test shutdown by closing the experimental IDE; this
option refuses to close an IDE with unsaved documents or an unsaved solution.
Run the test in each supported Visual Studio version.

Add `-Tcp` or `-WebSocket` to test the corresponding menu against external
server processes created by the test. These processes are cleaned up by the test,
not by the extension. If the experimental instance uses a custom debug port,
also pass `-TcpPort <port>`.
With either network mode and `-VerifyIdeExit`, the final IDE-exit check uses
stdio and also verifies switching back from the network transport. To test an
isolated registry hive, build with `/p:VSSDKTargetPlatformRegRootSuffix=LspfParity`,
launch `devenv /RootSuffix LspfParity`, and pass `-RootSuffix LspfParity` to the
automation scripts.

The transport-only smoke test checks port validation, delayed server startup,
stream I/O, socket disposal, and cancellation without launching Visual Studio:

```powershell
msbuild clients/vs/tests/TcpTransportSmoke.csproj /restore /v:minimal
& ./clients/vs/tests/bin/Debug/net48/TcpTransportSmoke.exe
```

## Test analysis features

The standalone protocol test uses the real server and the client's payload
validators. It covers custom notifications, function details, hover coordinates,
malformed payloads, sorting, live configuration changes, diagnostic clearing,
unsaved text updates, empty files, close, and graceful shutdown:

```powershell
msbuild clients/vs/tests/HealthSmoke.csproj /restore /v:minimal
& ./clients/vs/tests/bin/HealthProtocol/net48/HealthSmoke.exe
& ./clients/vs/tests/bin/HealthProtocol/net48/HealthSmoke.exe target/debug/lspf-analysis.exe --ws
```

With an experimental IDE open, test actual editor and tool-window behavior:

```powershell
powershell.exe -NoProfile -File clients/vs/tests/Test-Health.ps1
```

The script checks visible file scores, the function tree, Quick Info metric
content, unsaved edits, restart recovery, an empty file, Error List population,
unsupported-file switching, and diagnostic cleanup. It creates uniquely named
fixtures under `tests/obj`, closes its documents without saving, and removes
those fixtures. It leaves the experimental IDE open and the server stopped.
Use `-VisualStudioProcessId` and `-RootSuffix` to select an isolated instance.

The automated IDE checks were run on Visual Studio 2026. The extension compiles
against VS SDK 17.0 for Visual Studio 2022 compatibility; running the same IDE
checks on a 2022 installation is still required to verify that host.

To check hover alignment, live palette changes, grade contrast, and the dark
fallback against the built WPF control:

```powershell
powershell.exe -NoProfile -Sta -File clients/vs/tests/Test-HoverLayout.ps1
```

Pass `-VisualStudioDirectory` to use another VS installation, or `-RenderPath`
to save the dark-theme rendering as a PNG.
