using System;
using System.Collections.Generic;
using System.ComponentModel.Composition;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Controls;
using Microsoft.VisualStudio;
using Microsoft.VisualStudio.Language.Intellisense;
using Microsoft.VisualStudio.Shell;
using Microsoft.VisualStudio.Shell.Interop;
using Microsoft.VisualStudio.Text;
using Microsoft.VisualStudio.Text.Adornments;
using Microsoft.VisualStudio.Text.Editor;
using Microsoft.VisualStudio.Text.Tagging;
using Microsoft.VisualStudio.Utilities;
using Newtonsoft.Json.Linq;

namespace LspfAnalysis
{
    [Export(typeof(IWpfTextViewCreationListener))]
    [ContentType("text")]
    [TextViewRole(PredefinedTextViewRoles.Document)]
    internal sealed class AnalysisViewListener : IWpfTextViewCreationListener
    {
        [Import] internal ITextDocumentFactoryService Documents = null;

        public void TextViewCreated(IWpfTextView view)
        {
            if (!Documents.TryGetTextDocument(view.TextBuffer, out var document)) return;
            AnalysisSession.Instance.Run(async () =>
            {
                if (view.IsClosed) return;
                AnalysisSession.Instance.Register(view, document);
                if (HealthProtocol.Language(document.FilePath) == null) return;
                if (AnalysisPackage.Instance == null)
                {
                    var service = await AsyncServiceProvider.GlobalProvider.GetServiceAsync(typeof(SVsShell));
                    await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync();
                    var shell = service as IVsShell ?? throw new InvalidOperationException("VS shell is unavailable.");
                    var id = typeof(AnalysisPackage).GUID;
                    ErrorHandler.ThrowOnFailure(shell.LoadPackage(ref id, out var package));
                }
                if (!view.IsClosed) await AnalysisPackage.Instance.EnsureStartedAsync();
            });
        }
    }

    [Export(typeof(IAsyncQuickInfoSourceProvider))]
    [Name("LSPF Analysis health")]
    [ContentType("text")]
    internal sealed class HealthQuickInfoProvider : IAsyncQuickInfoSourceProvider
    {
        public IAsyncQuickInfoSource TryCreateQuickInfoSource(ITextBuffer textBuffer) => new HealthQuickInfoSource(textBuffer);
    }

    internal sealed class HealthQuickInfoSource : IAsyncQuickInfoSource
    {
        private readonly ITextBuffer buffer;
        internal HealthQuickInfoSource(ITextBuffer buffer) => this.buffer = buffer;
        public void Dispose() { }

        public async Task<QuickInfoItem> GetQuickInfoItemAsync(IAsyncQuickInfoSession session, CancellationToken cancellationToken)
        {
            try
            {
                await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync(cancellationToken);
                var snapshot = buffer.CurrentSnapshot;
                var point = session.GetTriggerPoint(snapshot);
                if (!point.HasValue) return null;
                var line = point.Value.GetContainingLine();
                using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                timeout.CancelAfter(TimeSpan.FromSeconds(5));
                var answer = await AnalysisSession.Instance.HoverAsync(buffer, line.LineNumber, point.Value.Position - line.Start.Position, timeout.Token);
                await ThreadHelper.JoinableTaskFactory.SwitchToMainThreadAsync(cancellationToken);
                if (snapshot != buffer.CurrentSnapshot || answer is not JObject obj || obj["contents"]?["value"]?.Type != JTokenType.String) return null;
                var range = EditorRange.Span(snapshot, obj["range"]);
                if (!range.HasValue || !range.Value.Contains(point.Value)) return null;
                var content = HealthHoverView.Create((string)obj["contents"]["value"]);
                return new QuickInfoItem(snapshot.CreateTrackingSpan(range.Value.Span, SpanTrackingMode.EdgeInclusive), content);
            }
            catch (OperationCanceledException) { return null; }
            catch (Exception error) { AnalysisSession.Instance.Log?.Invoke("Hover: " + error.Message); return null; }
        }
    }

    internal static class EditorRange
    {
        internal static SnapshotSpan? Span(ITextSnapshot snapshot, JToken range)
        {
            if (range is not JObject) return null;
            var start = Position(snapshot, range["start"]);
            var end = Position(snapshot, range["end"]);
            if (!start.HasValue || !end.HasValue || end < start) return null;
            return new SnapshotSpan(snapshot, Microsoft.VisualStudio.Text.Span.FromBounds(start.Value, end.Value));
        }

        private static int? Position(ITextSnapshot snapshot, JToken point)
        {
            if (point is not JObject || point["line"]?.Type != JTokenType.Integer || point["character"]?.Type != JTokenType.Integer) return null;
            var row = (double)point["line"];
            var column = (double)point["character"];
            if (row < 0 || row >= snapshot.LineCount || column < 0 || column > int.MaxValue) return null;
            var line = snapshot.GetLineFromLineNumber((int)row);
            if (column > line.Length) return null;
            return line.Start.Position + (int)column;
        }
    }

    [Export(typeof(IViewTaggerProvider))]
    [ContentType("text")]
    [TagType(typeof(ErrorTag))]
    internal sealed class HealthErrorTaggerProvider : IViewTaggerProvider
    {
        public ITagger<T> CreateTagger<T>(ITextView textView, ITextBuffer buffer) where T : ITag =>
            buffer == textView.TextBuffer ? new HealthErrorTagger(textView, buffer) as ITagger<T> : null;
    }

    internal sealed class HealthErrorTagger : ITagger<ErrorTag>, IDisposable
    {
        private readonly ITextBuffer buffer;
        private readonly ITextView view;
        internal HealthErrorTagger(ITextView view, ITextBuffer buffer)
        {
            this.view = view;
            this.buffer = buffer;
            AnalysisSession.Instance.Changed += Refresh;
            view.Closed += Closed;
        }
        public event EventHandler<SnapshotSpanEventArgs> TagsChanged;
        private void Refresh() => TagsChanged?.Invoke(this, new SnapshotSpanEventArgs(new SnapshotSpan(buffer.CurrentSnapshot, 0, buffer.CurrentSnapshot.Length)));
        private void Closed(object sender, EventArgs e) => Dispose();
        public void Dispose() { AnalysisSession.Instance.Changed -= Refresh; view.Closed -= Closed; }
        public IEnumerable<ITagSpan<ErrorTag>> GetTags(NormalizedSnapshotSpanCollection spans)
        {
            if (spans.Count == 0) yield break;
            var doc = AnalysisSession.Instance.Find(buffer);
            if (doc == null || doc.Sent != spans[0].Snapshot) yield break;
            foreach (var diagnostic in doc.Diagnostics)
            {
                var span = EditorRange.Span(spans[0].Snapshot, diagnostic["range"]);
                if (!span.HasValue || !spans.IntersectsWith(span.Value)) continue;
                var severity = (int?)diagnostic["severity"] ?? 1;
                yield return new TagSpan<ErrorTag>(span.Value, new ErrorTag(severity == 1 ? "syntax error" : severity == 2 ? "warning" : "suggestion", DiagnosticHover.Create((string)diagnostic["message"])));
            }
        }
    }

    internal static class DiagnosticHover
    {
        // Error tags can be requested off the UI thread. Let VS materialize this
        // presentation model using its current tooltip and editor font settings.
        internal static ClassifiedTextElement Create(string message)
        {
            message ??= string.Empty;
            var runs = new List<ClassifiedTextRun>();
            var start = 0;
            while (start < message.Length)
            {
                var open = message.IndexOf('`', start);
                var close = open < 0 ? -1 : message.IndexOf('`', open + 1);
                if (close < 0)
                {
                    runs.Add(new ClassifiedTextRun(ClassifiedTextElement.TextClassificationTypeName, message.Substring(start)));
                    break;
                }
                if (open > start)
                    runs.Add(new ClassifiedTextRun(ClassifiedTextElement.TextClassificationTypeName, message.Substring(start, open - start)));
                runs.Add(new ClassifiedTextRun(ClassifiedTextElement.TextClassificationTypeName,
                    message.Substring(open + 1, close - open - 1), ClassifiedTextRunStyle.UseClassificationFont));
                start = close + 1;
            }
            return new ClassifiedTextElement(runs);
        }
    }
}
