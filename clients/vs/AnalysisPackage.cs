using System;
using System.ComponentModel.Design;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio;
using Microsoft.VisualStudio.ComponentModelHost;
using Microsoft.VisualStudio.LanguageServer.Client;
using Microsoft.VisualStudio.Shell;
using Microsoft.VisualStudio.Shell.Interop;
using Microsoft.VisualStudio.Utilities;
using EnvDTE;
using System.Linq;

namespace LspfAnalysis
{
    [PackageRegistration(UseManagedResourcesOnly = true, AllowsBackgroundLoading = true)]
    [ProvideMenuResource("Menus.ctmenu", 1)]
    [ProvideToolWindow(typeof(HealthToolWindow), Style = VsDockStyle.Tabbed, Window = ToolWindowGuids80.SolutionExplorer)]
    [ProvideOptionPage(typeof(AnalysisOptions), "LSPF Analysis", "General", 0, 110, true)]
    [Guid("8389520C-7B24-42C0-A93B-D7C003E8BCA0")]
    public sealed class AnalysisPackage : AsyncPackage
    {
        private AnalysisLanguageClient client;
        private ILanguageClientBroker broker;
        private IVsOutputWindowPane output;
        private IVsUIShell shell;
        private bool closing;
        private bool manuallyStopped;
        private int? transportPort;
        private bool webSocketTransport;
        private ErrorListProvider errors;
        internal static AnalysisPackage Instance { get; private set; }
        internal AnalysisOptions Options { get; private set; }

        protected override async Task InitializeAsync(CancellationToken cancellationToken, IProgress<ServiceProgressData> progress)
        {
            var commandService = await GetServiceAsync(typeof(IMenuCommandService));
            var outputService = await GetServiceAsync(typeof(SVsOutputWindow));
            var shellService = await GetServiceAsync(typeof(SVsUIShell));
            var componentService = await GetServiceAsync(typeof(SComponentModel));
            await JoinableTaskFactory.SwitchToMainThreadAsync(cancellationToken);
            var commands = commandService as OleMenuCommandService ?? throw new InvalidOperationException("Command service is unavailable.");
            var window = outputService as IVsOutputWindow ?? throw new InvalidOperationException("Output window is unavailable.");
            shell = shellService as IVsUIShell ?? throw new InvalidOperationException("UI shell is unavailable.");
            var paneId = new Guid("21942127-0253-4D66-915D-FC9A1C0594B8");
            ErrorHandler.ThrowOnFailure(window.CreatePane(ref paneId, "LSPF Analysis", 1, 0));
            ErrorHandler.ThrowOnFailure(window.GetPane(ref paneId, out output));
            var components = componentService as IComponentModel ?? throw new InvalidOperationException("Component model is unavailable.");
            var contentTypes = components.GetService<IContentTypeRegistryService>();
            if (contentTypes.GetContentType(AnalysisLanguageClient.ContentTypeName) == null)
                contentTypes.AddContentType(AnalysisLanguageClient.ContentTypeName, new[] { CodeRemoteContentDefinition.CodeRemoteBaseTypeName });
            broker = components.GetService<ILanguageClientBroker>();
            Options = (AnalysisOptions)GetDialogPage(typeof(AnalysisOptions));
            client = new AnalysisLanguageClient(Log, Options);
            errors = new ErrorListProvider(this) { ProviderName = "LSPF Analysis", ProviderGuid = new Guid("C5CDA744-0AAA-41F9-A245-E2E59974B635") };
            Instance = this;
            AnalysisSession.Instance.Log = Log;
            AnalysisSession.Instance.Changed += RefreshResults;
            Log("Extension loaded. Supported source editors start analysis automatically; Tools commands also control the server.");
            var commandSet = new Guid("FE1B9436-D20D-4608-961C-A9BEA967940E");
            commands.AddCommand(new MenuCommand((sender, args) => RunCommand(true), new CommandID(commandSet, 0x0100)));
            commands.AddCommand(new MenuCommand((sender, args) => RunCommand(false), new CommandID(commandSet, 0x0101)));
            commands.AddCommand(new MenuCommand((sender, args) => RunCommand(true, true), new CommandID(commandSet, 0x0102)));
            commands.AddCommand(new MenuCommand((sender, args) => Restart(), new CommandID(commandSet, 0x0103)));
            commands.AddCommand(new MenuCommand((sender, args) => ShowHealth(), new CommandID(commandSet, 0x0104)));
            commands.AddCommand(new MenuCommand((sender, args) => OpenSettings(), new CommandID(commandSet, 0x0105)));
            commands.AddCommand(new MenuCommand((sender, args) => RunCommand(true, true, true), new CommandID(commandSet, 0x0106)));
        }

        internal async Task EnsureStartedAsync()
        {
            if (!closing && !manuallyStopped) await client.RequestStartAsync(broker, DisposalToken, transportPort, webSocketTransport);
        }

        internal void Restart() => AnalysisSession.Instance.Run(async () =>
        {
            manuallyStopped = false;
            await client.RequestStopAsync();
            await client.RequestStartAsync(broker, DisposalToken, transportPort, webSocketTransport);
        });

        internal void ApplySettings()
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            client.ApplyTraceOptions();
            AnalysisSession.Instance.Configure(Options.Payload());
        }
        internal void OpenSettings() => ShowOptionPage(typeof(AnalysisOptions));
        internal void ShowProblems() => errors.Show();
        internal void ShowHealth() => AnalysisSession.Instance.Run(async () =>
        {
            var window = await ShowToolWindowAsync(typeof(HealthToolWindow), 0, true, DisposalToken);
            if (window == null) throw new InvalidOperationException("Function Health tool window is unavailable.");
        });

        internal void Navigate(string uri, int line)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            if (!HealthProtocol.IsLocalUri(uri) || line < 1 || !AnalysisSession.Instance.Documents.Any(d => d.Uri == uri)) return;
            try
            {
                VsShellUtilities.OpenDocument(this, new Uri(uri).LocalPath);
                var dte = GetService(typeof(DTE)) as DTE ?? throw new InvalidOperationException("DTE is unavailable.");
                if (dte.ActiveDocument?.Selection is TextSelection selection)
                    selection.GotoLine(line, true);
            }
            catch (Exception error) { Log("Navigation: " + error.Message); }
        }

        private void RefreshResults()
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            if (closing) return;
            errors.SuspendRefresh();
            try
            {
                errors.Tasks.Clear();
                foreach (var doc in AnalysisSession.Instance.Documents)
                foreach (var diagnostic in doc.Diagnostics)
                {
                    var row = (int)diagnostic["range"]["start"]["line"];
                    var severity = (int?)diagnostic["severity"] ?? 1;
                    var uri = doc.Uri;
                    var task = new ErrorTask {
                        Category = TaskCategory.CodeSense,
                        ErrorCategory = severity == 1 ? TaskErrorCategory.Error : severity == 2 ? TaskErrorCategory.Warning : TaskErrorCategory.Message,
                        Text = (string)diagnostic["message"], Document = new Uri(uri).LocalPath,
                        Line = row, Column = (int)diagnostic["range"]["start"]["character"]
                    };
                    task.Navigate += (sender, args) => Navigate(uri, row + 1);
                    errors.Tasks.Add(task);
                }
            }
            finally { errors.ResumeRefresh(); }
        }

        private void RunCommand(bool start, bool tcp = false, bool websocket = false)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            if (closing)
                return;
            var outputWindow = VSConstants.StandardToolWindows.Output;
            ErrorHandler.ThrowOnFailure(shell.FindToolWindow((uint)__VSFINDTOOLWIN.FTW_fForceCreate, ref outputWindow, out var frame));
            ErrorHandler.ThrowOnFailure(frame.Show());
            ErrorHandler.ThrowOnFailure(output.Activate());
            Log(websocket ? "Connect to WebSocket debug server requested." : tcp ? "Connect to TCP debug server requested." : start ? "Start language server requested." : "Stop language server requested.");
            _ = JoinableTaskFactory.RunAsync(async () =>
            {
                try
                {
                    if (start)
                    {
                        manuallyStopped = false;
                        var requestedPort = tcp ? (int?)DebugTcpConnection.ParsePort(Environment.GetEnvironmentVariable(DebugTcpConnection.PortVariable)) : null;
                        if (await client.RequestStartAsync(broker, DisposalToken, requestedPort, websocket))
                        {
                            transportPort = requestedPort;
                            webSocketTransport = websocket;
                        }
                    }
                    else
                    {
                        manuallyStopped = true;
                        await client.RequestStopAsync();
                    }
                }
                catch (Exception error)
                {
                    Log($"Language server operation failed: {error}");
                }
            });
        }

        private void Log(string message)
        {
            var line = $"[{DateTimeOffset.Now:HH:mm:ss.fff}] {message}{Environment.NewLine}";
            // Even OutputStringThreadSafe can marshal synchronously to the UI
            // thread. Never block a shutdown task on that COM call.
            _ = JoinableTaskFactory.RunAsync(async () =>
            {
                await JoinableTaskFactory.SwitchToMainThreadAsync();
                if (!closing)
                    output?.OutputString(line);
            });
        }

        protected override int QueryClose(out bool canClose)
        {
            // Stop while the native LSP services are still available. Waiting
            // until package disposal is too late during shell shutdown.
            try
            {
                if (client != null)
                    JoinableTaskFactory.Run(() => client.RequestStopAsync());
            }
            catch (Exception error)
            {
                Log($"Language server shutdown failed: {error}");
            }
            return base.QueryClose(out canClose);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing && client != null)
            {
                closing = true;
                AnalysisSession.Instance.Changed -= RefreshResults;
                errors?.Dispose();
                Instance = null;
                try
                {
                    JoinableTaskFactory.Run(() => client.RequestStopAsync());
                }
                catch (Exception error)
                {
                    Log($"Language server shutdown failed: {error.Message}");
                }
            }
            base.Dispose(disposing);
        }
    }
}
