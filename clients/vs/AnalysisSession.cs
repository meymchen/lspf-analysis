using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.VisualStudio.Shell;
using Microsoft.VisualStudio.Text;
using Microsoft.VisualStudio.Text.Editor;
using Newtonsoft.Json.Linq;
using StreamJsonRpc;
using Microsoft.VisualStudio.Threading;

namespace LspfAnalysis
{
    // All state belongs to the UI thread. One queue orders open/change/close and
    // requests; a connection generation prevents old responses crossing a restart.
    internal sealed class AnalysisSession
    {
        internal static readonly AnalysisSession Instance = new();
        internal sealed class Document
        {
            internal ITextDocument Source;
            internal string Uri;
            internal ITextSnapshot Sent;
            internal int Version;
            internal int Revision;
            internal int Views;
            internal JObject Summary;
            internal JObject Functions;
            internal JArray Diagnostics = [];
        }

        private readonly Dictionary<ITextBuffer, Document> documents = [];
        private readonly SemaphoreSlim queue = new(1, 1);
        private JsonRpc rpc;
        private int generation;
        private int settingsRevision;
        private IWpfTextView activeView;
        internal event Action Changed;
        internal Action<string> Log;
        internal bool Ready => rpc != null;
        internal Document Active
        {
            get
            {
                var doc = activeView != null && !activeView.IsClosed ? Find(activeView.TextBuffer) : null;
                return doc != null && HealthProtocol.Language(doc.Source.FilePath) != null ? doc : null;
            }
        }
        internal Document Find(ITextBuffer buffer) => documents.TryGetValue(buffer, out var doc) ? doc : null;
        internal IEnumerable<Document> Documents => documents.Values;

        internal void Register(IWpfTextView view, ITextDocument source)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            view.GotAggregateFocus += Focus;
            view.Closed += Closed;
            if (HealthProtocol.Language(source.FilePath) == null || !HealthProtocol.IsLocalUri(new Uri(source.FilePath).AbsoluteUri))
            {
                if (view.HasAggregateFocus) Activate(view);
                return;
            }
            if (!documents.TryGetValue(source.TextBuffer, out var doc))
            {
                doc = new Document { Source = source, Uri = new Uri(source.FilePath).AbsoluteUri };
                documents.Add(source.TextBuffer, doc);
                source.TextBuffer.Changed += BufferChanged;
                source.FileActionOccurred += FileAction;
                Schedule(doc);
            }
            doc.Views++;
            if (view.HasAggregateFocus) Activate(view);
        }

        private void Focus(object sender, EventArgs args)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            Activate((IWpfTextView)sender);
        }
        private void Activate(IWpfTextView view)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            activeView = view;
            Changed?.Invoke();
            var doc = Active;
            if (doc != null) Schedule(doc);
        }

        private void BufferChanged(object sender, TextContentChangedEventArgs args)
        {
            // Some editor clients update buffers on a worker thread.
            Run(async () =>
            {
                var doc = Find((ITextBuffer)sender);
                if (doc == null) return;
                doc.Revision++;
                doc.Summary = doc.Functions = null;
                doc.Diagnostics = [];
                Changed?.Invoke();
                var revision = doc.Revision;
                await Task.Delay(250);
                await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
                if (doc.Revision == revision) Schedule(doc);
            });
        }

        private void FileAction(object sender, TextDocumentFileActionEventArgs args)
        {
            Run(async () =>
            {
                var doc = Find(((ITextDocument)sender).TextBuffer);
                if (doc == null) return;
                var uri = new Uri(doc.Source.FilePath).AbsoluteUri;
                if (uri != doc.Uri)
                {
                    var oldUri = doc.Uri;
                    doc.Uri = uri;
                    doc.Revision++;
                    doc.Sent = null;
                    doc.Summary = doc.Functions = null;
                    doc.Diagnostics = [];
                    await EnqueueAsync(async connection => await connection.NotifyWithParameterObjectAsync("textDocument/didClose", new { textDocument = new { uri = oldUri } }));
                }
                Schedule(doc);
                Changed?.Invoke();
            });
        }

        private void Closed(object sender, EventArgs args)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            var view = (IWpfTextView)sender;
            view.GotAggregateFocus -= Focus;
            view.Closed -= Closed;
            if (activeView == view) activeView = null;
            var doc = Find(view.TextBuffer);
            if (doc != null && --doc.Views == 0)
            {
                documents.Remove(view.TextBuffer);
                doc.Source.TextBuffer.Changed -= BufferChanged;
                doc.Source.FileActionOccurred -= FileAction;
                var uri = doc.Uri;
                Run(() => EnqueueAsync(connection => connection.NotifyWithParameterObjectAsync("textDocument/didClose", new { textDocument = new { uri } })));
            }
            Changed?.Invoke();
        }

        internal void Run(Func<Task> work)
        {
            // Editor events have no awaitable contract. Exceptions are handled
            // here, and queued results are guarded by session/document revisions.
#pragma warning disable VSSDK007
            ThreadHelper.JoinableTaskFactory.RunAsync(async () =>
            {
                try
                {
                    await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
                    await work();
                }
                catch (OperationCanceledException) { }
                catch (Exception error) { Log?.Invoke("Analysis: " + error.Message); }
            }).FileAndForget("LspfAnalysis/BackgroundWork");
#pragma warning restore VSSDK007
        }

        private async Task EnqueueAsync(Func<JsonRpc, Task> work)
        {
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            var epoch = generation;
            var connection = rpc;
            if (connection == null) return;
            await queue.WaitAsync();
            try
            {
                await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
                if (epoch == generation && connection == rpc) await work(connection);
            }
            finally { queue.Release(); }
        }

        internal void Start(JsonRpc connection)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            Stop();
            rpc = connection;
            var epoch = generation;
            connection.Disconnected += (sender, args) => Run(() =>
            {
                if (epoch == generation) Stop();
                return Task.CompletedTask;
            });
            foreach (var doc in documents.Values.ToArray()) Schedule(doc);
            Changed?.Invoke();
        }

        internal void Stop()
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            generation++;
            rpc = null;
            foreach (var doc in documents.Values)
            {
                doc.Sent = null;
                doc.Summary = doc.Functions = null;
                doc.Diagnostics = [];
            }
            Changed?.Invoke();
        }

        private void Schedule(Document doc) => Run(() => EnqueueAsync(async connection =>
        {
            if (Find(doc.Source.TextBuffer) != doc || HealthProtocol.Language(doc.Source.FilePath) == null) return;
            await SynchronizeAsync(connection, doc);
            if (doc != Active || doc.Functions != null) return;
            var snapshot = doc.Source.TextBuffer.CurrentSnapshot;
            var uri = doc.Uri;
            var epoch = generation;
            var configuration = settingsRevision;
            using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            var answer = await connection.InvokeWithParameterObjectAsync<JToken>(HealthProtocol.FunctionHealthMethod, new { uri }, timeout.Token);
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            if (epoch != generation || configuration != settingsRevision || Find(doc.Source.TextBuffer) != doc || doc.Uri != uri || snapshot != doc.Source.TextBuffer.CurrentSnapshot) return;
            var health = HealthProtocol.FunctionHealth(answer);
            doc.Functions = (string)health?["uri"] == uri ? health : null;
            Changed?.Invoke();
        }));

        private async Task SynchronizeAsync(JsonRpc connection, Document doc)
        {
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            var snapshot = doc.Source.TextBuffer.CurrentSnapshot;
            if (doc.Sent == snapshot) return;
            var version = ++doc.Version;
            var uri = doc.Uri;
            var epoch = generation;
            if (doc.Sent == null)
                await connection.NotifyWithParameterObjectAsync("textDocument/didOpen", new { textDocument = new {
                    uri, languageId = HealthProtocol.Language(doc.Source.FilePath), version, text = snapshot.GetText() } });
            else
                await connection.NotifyWithParameterObjectAsync("textDocument/didChange", new {
                    textDocument = new { uri, version }, contentChanges = new[] { new { text = snapshot.GetText() } } });
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            if (epoch == generation && doc.Uri == uri) doc.Sent = snapshot;
        }

        internal void Configure(JObject settings)
        {
            ThreadHelper.ThrowIfNotOnUIThread();
            settingsRevision++;
            foreach (var doc in documents.Values) { doc.Summary = doc.Functions = null; doc.Diagnostics = []; }
            Changed?.Invoke();
            Run(async () =>
            {
                await EnqueueAsync(connection => connection.NotifyWithParameterObjectAsync("workspace/didChangeConfiguration", new { settings }));
                // Work already queued before this change may have completed using
                // the old settings. Invalidate again after the ordered push.
                foreach (var doc in documents.Values) doc.Functions = null;
                if (Active != null) Schedule(Active);
            });
        }

        internal Task ReceiveSummaryAsync(JToken payload, JsonRpc connection) => DispatchAsync(connection, () =>
        {
            var health = HealthProtocol.FileHealth(payload);
            var doc = health == null ? null : documents.Values.FirstOrDefault(d => d.Uri == (string)health["uri"]);
            if (doc == null || doc.Sent != doc.Source.TextBuffer.CurrentSnapshot) return;
            doc.Summary = health;
            Changed?.Invoke();
        });

        internal Task ReceiveDiagnosticsAsync(JToken payload, JsonRpc connection) => DispatchAsync(connection, () =>
        {
            if (payload is not JObject obj || obj["uri"]?.Type != JTokenType.String || obj["diagnostics"] is not JArray diagnostics) return;
            var doc = documents.Values.FirstOrDefault(d => d.Uri == (string)obj["uri"]);
            if (doc == null || doc.Sent != doc.Source.TextBuffer.CurrentSnapshot ||
                (obj["version"]?.Type == JTokenType.Integer && (int)obj["version"] != doc.Version)) return;
            doc.Diagnostics = new JArray(diagnostics.Where(ValidDiagnostic));
            Changed?.Invoke();
        });

        private static bool ValidDiagnostic(JToken d)
        {
            if (d is not JObject || d["message"]?.Type != JTokenType.String || d["range"] is not JObject range) return false;
            foreach (var end in new[] { "start", "end" })
            {
                if (range[end] is not JObject point) return false;
                foreach (var axis in new[] { "line", "character" })
                    if (point[axis]?.Type != JTokenType.Integer || (double)point[axis] < 0 || (double)point[axis] > int.MaxValue) return false;
            }
            return d["severity"] == null || (d["severity"].Type == JTokenType.Integer && (int)d["severity"] >= 1 && (int)d["severity"] <= 4);
        }

        private async Task DispatchAsync(JsonRpc connection, Action action)
        {
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
            if (connection != null && connection == rpc) action();
        }

        internal async Task<JToken> HoverAsync(ITextBuffer buffer, int line, int character, CancellationToken token)
        {
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync(token);
            var doc = Find(buffer);
            if (doc == null || rpc == null) return null;
            JToken result = null;
            var snapshot = buffer.CurrentSnapshot;
            var epoch = generation;
            await EnqueueAsync(async connection =>
            {
                token.ThrowIfCancellationRequested();
                await SynchronizeAsync(connection, doc);
                result = await connection.InvokeWithParameterObjectAsync<JToken>("textDocument/hover", new {
                    textDocument = new { uri = doc.Uri }, position = new { line, character } }, token);
            });
            await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync(token);
            return epoch == generation && snapshot == buffer.CurrentSnapshot ? result : null;
        }
    }
}
