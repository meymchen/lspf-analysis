using System;
using System.Collections.Generic;
using System.IO;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio.LanguageServer.Client;
using Microsoft.VisualStudio.Threading;
using Microsoft.VisualStudio.Shell;
using Newtonsoft.Json.Linq;
using StreamJsonRpc;
using System.Diagnostics;

namespace LspfAnalysis
{
    // Loaded explicitly by the broker; the private content type has no file associations.
    internal sealed class AnalysisLanguageClient : ILanguageClient, ILanguageClientCustomMessage2
    {
        internal const string ContentTypeName = "lspf-analysis";
        private readonly SemaphoreSlim gate = new SemaphoreSlim(1, 1);
        private readonly ServerProcess server;
        private readonly Action<string> log;
        private bool loaded;
        private int? debugPort;
        private TcpClient tcp;
        private DebugWebSocketStream webSocket;
        private bool useWebSocket;
        private JsonRpc rpc;
        private readonly AnalysisOptions options;
        private MessageTarget messages;
        private TraceSource trace;

        internal AnalysisLanguageClient(Action<string> log, AnalysisOptions options)
        {
            this.log = log;
            this.options = options;
            server = new ServerProcess(log);
            messages = new MessageTarget();
        }

        public string Name => "LSPF Analysis";
        public IEnumerable<string> ConfigurationSections => null;
        public object InitializationOptions => options.Payload();
        public object MiddleLayer => messages;
        public object CustomMessageTarget => messages;
        public Task AttachForCustomMessageAsync(JsonRpc connection)
        {
            rpc = connection;
            messages.Connection = connection;
            trace?.Close();
            trace = new TraceSource("LSPF Analysis", SourceLevels.Off);
            trace.Listeners.Clear();
            trace.Listeners.Add(new ServerTraceListener(log));
            rpc.TraceSource = trace;
            ApplyTraceOptions();
            return Task.CompletedTask;
        }
        internal void ApplyTraceOptions()
        {
            if (trace != null) trace.Switch.Level = options.Trace == ServerTrace.Verbose ? SourceLevels.Verbose :
                options.Trace == ServerTrace.Messages ? SourceLevels.Information : SourceLevels.Off;
        }
        public IEnumerable<string> FilesToWatch => null;
        public bool ShowNotificationOnInitializeFailed => true;
        public event AsyncEventHandler<EventArgs> StartAsync;
        public event AsyncEventHandler<EventArgs> StopAsync;

        internal async Task<bool> RequestStartAsync(ILanguageClientBroker broker, CancellationToken cancellationToken, int? tcpPort = null, bool websocket = false)
        {
            await gate.WaitAsync(cancellationToken);
            try
            {
                if (server.IsRunning || tcp != null || webSocket != null)
                {
                    log("Language server is already running or initializing.");
                    return false;
                }
                debugPort = tcpPort;
                useWebSocket = websocket;
                if (!loaded)
                {
                    log("Loading ILanguageClient through the Visual Studio LSP broker.");
                    await broker.LoadAsync(new ClientMetadata(), this);
                }
                else
                    await StartAsync.InvokeAsync(this, EventArgs.Empty);
                return true;
            }
            finally
            {
                gate.Release();
            }
        }

        internal async Task RequestStopAsync()
        {
            await gate.WaitAsync();
            try
            {
                await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
                AnalysisSession.Instance.Stop();
                if (!loaded || (!server.IsRunning && tcp == null && webSocket == null))
                {
                    log("Language server is not running.");
                    server.Dispose();
                    return;
                }
                log("Requesting shutdown through the Visual Studio LSP client.");
                try
                {
                    await StopAsync.InvokeAsync(this, EventArgs.Empty);
                    if (tcp != null || webSocket != null)
                        log((tcp != null ? "TCP" : "WebSocket") + " debug connection stopped by Visual Studio. The external server is not owned by the extension.");
                    else
                        await server.WaitForExitAsync();
                }
                finally
                {
                    server.Dispose();
                    tcp?.Dispose();
                    tcp = null;
                    webSocket?.Dispose();
                    webSocket = null;
                }
            }
            finally
            {
                gate.Release();
            }
        }

        public async Task OnLoadedAsync()
        {
            loaded = true;
            await StartAsync.InvokeAsync(this, EventArgs.Empty);
        }

        public async Task<Connection> ActivateAsync(CancellationToken token)
        {
            // Each activation gets a receiver bound to that connection. Delayed
            // callbacks from a stopped session cannot publish into its successor.
            messages = new MessageTarget();
            if (debugPort.HasValue)
            {
                if (useWebSocket)
                {
                    log($"Connecting to WebSocket debug server at ws://127.0.0.1:{debugPort.Value} (timeout: 20 s). No server process will be launched.");
                    webSocket = await DebugWebSocketStream.ConnectAsync(debugPort.Value, token);
                    log($"WebSocket connected to 127.0.0.1:{debugPort.Value}. Waiting for Visual Studio to complete the LSP handshake.");
                    return new Connection(webSocket, webSocket);
                }
                log($"Connecting to TCP debug server at 127.0.0.1:{debugPort.Value} (timeout: 20 s). No server process will be launched.");
                tcp = await DebugTcpConnection.ConnectAsync(debugPort.Value, token);
                log($"TCP connected to 127.0.0.1:{debugPort.Value}. Waiting for Visual Studio to complete the LSP handshake.");
                var stream = tcp.GetStream();
                return new Connection(stream, stream);
            }
            var directory = Path.GetDirectoryName(typeof(AnalysisLanguageClient).Assembly.Location);
            var executable = string.IsNullOrWhiteSpace(options.ServerPath) ? Path.Combine(directory, "server", "lspf-analysis.exe") : options.ServerPath.Trim();
            await Task.Run(() => server.Start(executable, token), token);
            log("Process started. Waiting for Visual Studio to complete the LSP handshake.");
            return new Connection(server.Reader, server.Writer);
        }

        public async Task OnServerInitializedAsync()
        {
            log("Language server initialized. Visual Studio ILanguageClient reports the server is ready.");
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            AnalysisSession.Instance.Start(rpc);
        }

        public Task<InitializationFailureContext> OnServerInitializeFailedAsync(ILanguageClientInitializationInfo initializationState)
        {
            var message = $"Language server initialization failed: {initializationState.StatusMessage}\n{initializationState.InitializationException}";
            log(message);
            server.Dispose();
            tcp?.Dispose();
            tcp = null;
            webSocket?.Dispose();
            webSocket = null;
            return Task.FromResult(new InitializationFailureContext { FailureMessage = message });
        }

        private sealed class ClientMetadata : ILanguageClientMetadata
        {
            public string ClientName => "LSPF Analysis";
            public IEnumerable<string> ContentTypes => new[] { ContentTypeName };
        }

        private sealed class MessageTarget : ILanguageClientMiddleLayer
        {
            internal JsonRpc Connection;
            [JsonRpcMethod(HealthProtocol.FileHealthMethod, UseSingleObjectParameterDeserialization = true)]
            public Task FileHealthAsync(JToken payload) => AnalysisSession.Instance.ReceiveSummaryAsync(payload, Connection);
            public bool CanHandle(string methodName) => methodName == "textDocument/publishDiagnostics";
            public Task HandleNotificationAsync(string methodName, JToken methodParam, Func<JToken, Task> sendNotification) =>
                AnalysisSession.Instance.ReceiveDiagnosticsAsync(methodParam, Connection);
            public Task<JToken> HandleRequestAsync(string methodName, JToken methodParam, Func<JToken, Task<JToken>> sendRequest) => sendRequest(methodParam);
        }
    }
}
