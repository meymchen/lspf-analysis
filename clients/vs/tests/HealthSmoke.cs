using System;
using System.IO;
using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using LspfAnalysis;
using Newtonsoft.Json.Linq;
using StreamJsonRpc;

internal static class HealthSmoke
{
    private sealed class ServerHost : IDisposable
    {
        private readonly ServerProcess stdio = new ServerProcess(Console.WriteLine);
        private Process external;
        private DebugWebSocketStream websocket;
        internal Stream Reader => websocket == null ? stdio.Reader : websocket;
        internal Stream Writer => websocket == null ? stdio.Writer : websocket;
        internal async Task StartAsync(string executable, bool ws)
        {
            if (!ws) { stdio.Start(executable, CancellationToken.None); return; }
            var probe = new TcpListener(IPAddress.Loopback, 0);
            probe.Start();
            var port = ((IPEndPoint)probe.LocalEndpoint).Port;
            probe.Stop();
            external = Process.Start(new ProcessStartInfo(executable, "serve --ws 127.0.0.1:" + port) {
                UseShellExecute = false, CreateNoWindow = true
            });
            websocket = await DebugWebSocketStream.ConnectAsync(port, CancellationToken.None);
        }
        internal async Task WaitForExitAsync()
        {
            if (external == null) { await stdio.WaitForExitAsync(); return; }
            Check(await Task.Run(() => external.WaitForExit(5000)) && external.ExitCode == 0, "WebSocket server exited normally");
        }
        public void Dispose()
        {
            websocket?.Dispose();
            stdio.Dispose();
            if (external == null) return;
            try { if (!external.HasExited) { external.Kill(); external.WaitForExit(5000); } }
            finally { external.Dispose(); }
        }
    }

    private static int checks;
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception(message);
        checks++;
        Console.WriteLine("PASS " + message);
    }
    private sealed class Notifications
    {
        internal TaskCompletionSource<JToken> Summary = Next();
        internal TaskCompletionSource<JToken> Diagnostics = Next();
        internal static TaskCompletionSource<JToken> Next() => new TaskCompletionSource<JToken>(TaskCreationOptions.RunContinuationsAsynchronously);
        [JsonRpcMethod(HealthProtocol.FileHealthMethod, UseSingleObjectParameterDeserialization = true)]
        public void FileHealth(JToken payload) => Summary.TrySetResult(payload);
        [JsonRpcMethod("textDocument/publishDiagnostics", UseSingleObjectParameterDeserialization = true)]
        public void PublishDiagnostics(JToken payload) => Diagnostics.TrySetResult(payload);
    }

    private static async Task<T> WithinAsync<T>(Task<T> task)
    {
        if (await Task.WhenAny(task, Task.Delay(10000)) != task) throw new TimeoutException("Protocol response timed out.");
        return await task;
    }

    private static async Task<int> Main(string[] args)
    {
        try { await RunAsync(args); return 0; }
        catch (Exception error) { await Console.Error.WriteLineAsync(error.GetType().FullName + ": " + error.Message); return 1; }
    }

    private static async Task RunAsync(string[] args)
    {
        var executable = Path.GetFullPath(args.Length == 0 ? "target/debug/lspf-analysis.exe" : args[0]);
        var payload = new JObject { ["lspfAnalysis"] = new JObject { ["locale"] = "en" } };
        Check(HealthProtocol.Language("A.TSX") == "typescriptreact" && HealthProtocol.Language("A.cs") == null, "supported extensions match the analysis languages");
        Check(!HealthProtocol.IsLocalUri("https://example.com/a.rs") && !HealthProtocol.IsLocalUri("file://server/share/a.rs"), "navigation excludes remote and UNC paths");

        using (var server = new ServerHost())
        {
            await server.StartAsync(executable, args.Contains("--ws"));
            var notifications = new Notifications();
            using (var rpc = new JsonRpc(new HeaderDelimitedMessageHandler(server.Writer, server.Reader), notifications))
            {
                rpc.StartListening();
                var init = await WithinAsync(rpc.InvokeWithParameterObjectAsync<JObject>("initialize", new {
                    processId = System.Diagnostics.Process.GetCurrentProcess().Id,
                    rootUri = new Uri(Environment.CurrentDirectory + Path.DirectorySeparatorChar).AbsoluteUri,
                    capabilities = new { general = new { positionEncodings = new[] { "utf-16" } }, textDocument = new { hover = new { contentFormat = new[] { "markdown" } } } },
                    initializationOptions = payload
                }));
                Check(init["capabilities"] != null, "real server handshake");
                await rpc.NotifyWithParameterObjectAsync("initialized", new { });
                var uri = new Uri(Path.Combine(Environment.CurrentDirectory, "clients/vs/tests/health sample 测试.rs")).AbsoluteUri;
                var text = "// 😀 UTF-16 and CRLF\r\nfn add(a: u32, b: u32) -> u32 {\r\n    a + b\r\n}\r\n\r\nfn one() -> u32 { 1 }\r\n";
                await rpc.NotifyWithParameterObjectAsync("textDocument/didOpen", new { textDocument = new { uri, languageId = "rust", version = 1, text } });
                var summary = HealthProtocol.FileHealth(await WithinAsync(notifications.Summary.Task));
                Check(summary != null && (int)summary["functions"] == 2 && (string)summary["uri"] == uri, "custom summary notification deserializes as one parameter object");
                Check((await WithinAsync(notifications.Diagnostics.Task))["diagnostics"] is JArray, "standard diagnostics received");
                var functions = HealthProtocol.FunctionHealth(await WithinAsync(rpc.InvokeWithParameterObjectAsync<JToken>(HealthProtocol.FunctionHealthMethod, new { uri })));
                Check(functions != null && ((JArray)functions["functions"]).Count == 2, "function details parse from the real server");
                Check((int)functions["functions"][0]["startLine"] == 2 && ((JArray)functions["functions"][0]["pillars"]).Count > 0, "one-based navigation and pillar details");
                var hover = await WithinAsync(rpc.InvokeWithParameterObjectAsync<JObject>("textDocument/hover", new { textDocument = new { uri }, position = new { line = 1, character = 4 } }));
                Check(hover["contents"]["value"].Type == JTokenType.String && (int)hover["range"]["start"]["line"] == 1, "hover uses zero-based UTF-16 editor coordinates");

                var bad = (JObject)functions.DeepClone();
                bad["functions"][0]["pillars"][0]["measures"][0]["score"] = "broken";
                Check(HealthProtocol.FunctionHealth(bad) == null, "malformed nested function data rejected");
                bad = (JObject)summary.DeepClone();
                bad["quality"] = double.NaN;
                Check(HealthProtocol.FileHealth(bad) == null, "nonfinite file score rejected");
                bad = (JObject)functions.DeepClone();
                bad["functions"][0]["quality"] = 50;
                bad["functions"][1]["quality"] = 50;
                Check((int)HealthProtocol.Sort(bad, false).First()["startLine"] == 2 && (int)HealthProtocol.Sort(bad, true).First()["startLine"] == 2, "sort ties use source order without mutating the payload");

                notifications.Summary = Notifications.Next();
                notifications.Diagnostics = Notifications.Next();
                payload["lspfAnalysis"]["health"] = new JObject { ["qualityWarn"] = 100, ["qualityError"] = 100 };
                await rpc.NotifyWithParameterObjectAsync("workspace/didChangeConfiguration", new { settings = payload });
                Check((int)(await WithinAsync(notifications.Summary.Task))["below"] > 0, "settings immediately recompute open document health");
                Check(((JArray)(await WithinAsync(notifications.Diagnostics.Task))["diagnostics"]).Count > 0, "configured thresholds produce diagnostics");
                notifications.Diagnostics = Notifications.Next();
                payload["lspfAnalysis"]["diagnostics"] = new JObject { ["enabled"] = false };
                await rpc.NotifyWithParameterObjectAsync("workspace/didChangeConfiguration", new { settings = payload });
                Check(((JArray)(await WithinAsync(notifications.Diagnostics.Task))["diagnostics"]).Count == 0, "disabling diagnostics clears errors");

                notifications.Summary = Notifications.Next();
                await rpc.NotifyWithParameterObjectAsync("textDocument/didChange", new { textDocument = new { uri, version = 2 }, contentChanges = new[] { new { text = "// no functions\r\n" } } });
                Check((int)(await WithinAsync(notifications.Summary.Task))["functions"] == 0, "unsaved full-text changes update analysis");
                var empty = HealthProtocol.FunctionHealth(await WithinAsync(rpc.InvokeWithParameterObjectAsync<JToken>(HealthProtocol.FunctionHealthMethod, new { uri })));
                Check(empty != null && ((JArray)empty["functions"]).Count == 0, "empty function view is a valid result");
                await rpc.NotifyWithParameterObjectAsync("textDocument/didClose", new { textDocument = new { uri } });
                Check(await WithinAsync(rpc.InvokeWithParameterObjectAsync<JToken>(HealthProtocol.FunctionHealthMethod, new { uri })) == null, "closed documents release server analysis");
                await WithinAsync(rpc.InvokeWithParameterObjectAsync<JToken>("shutdown", null));
                await rpc.NotifyWithParameterObjectAsync("exit", null);
                await server.WaitForExitAsync();
            }
        }
        Console.WriteLine($"PASS {checks} checks");
    }
}
