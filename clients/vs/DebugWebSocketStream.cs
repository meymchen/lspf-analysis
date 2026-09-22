using System;
using System.Globalization;
using System.IO;
using System.Net.WebSockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace LspfAnalysis
{
    // The native broker speaks Content-Length streams; the WS server speaks one
    // JSON object per text message. Only framing is adapted, never the LSP session.
    internal sealed class DebugWebSocketStream : Stream
    {
        private const int MaxMessageBytes = 64 * 1024 * 1024;
        private readonly ClientWebSocket socket;
        private readonly SemaphoreSlim reading = new(1, 1);
        private readonly SemaphoreSlim writing = new(1, 1);
        private readonly MemoryStream pending = new();
        private byte[] incoming = [];
        private int incomingOffset;
        private bool disposed;
        private DebugWebSocketStream(ClientWebSocket socket) => this.socket = socket;

        internal static async Task<DebugWebSocketStream> ConnectAsync(int port, CancellationToken cancellationToken)
        {
            if (port < 1 || port > 65535) throw new ArgumentOutOfRangeException(nameof(port));
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeout.CancelAfter(TimeSpan.FromSeconds(20));
            try
            {
                while (true)
                {
                    timeout.Token.ThrowIfCancellationRequested();
                    var socket = new ClientWebSocket();
                    // Debug traffic stays on loopback, even on machines with an HTTP proxy.
                    socket.Options.Proxy = null;
                    try
                    {
                        await socket.ConnectAsync(new Uri($"ws://127.0.0.1:{port}/"), timeout.Token).ConfigureAwait(false);
                        return new DebugWebSocketStream(socket);
                    }
                    catch (WebSocketException) { socket.Dispose(); }
                    catch { socket.Dispose(); throw; }
                    await Task.Delay(200, timeout.Token).ConfigureAwait(false);
                }
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                throw new TimeoutException($"No WebSocket debug server after 20 seconds. Start lspf-analysis serve --ws 127.0.0.1:{port} and try again.");
            }
        }

        public override async Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken token)
        {
            ValidateBuffer(buffer, offset, count);
            if (count == 0) return 0;
            await reading.WaitAsync(token).ConfigureAwait(false);
            try
            {
                if (incomingOffset == incoming.Length)
                {
                    using var message = new MemoryStream();
                    var chunk = new byte[8192];
                    WebSocketReceiveResult result;
                    do
                    {
                        result = await socket.ReceiveAsync(new ArraySegment<byte>(chunk), token).ConfigureAwait(false);
                        if (result.MessageType == WebSocketMessageType.Close) return 0;
                        if (result.MessageType != WebSocketMessageType.Text) throw new InvalidDataException("Expected a WebSocket text message.");
                        if (message.Length + result.Count > MaxMessageBytes) throw new InvalidDataException("WebSocket message exceeds 64 MiB.");
                        await message.WriteAsync(chunk, 0, result.Count, token).ConfigureAwait(false);
                    } while (!result.EndOfMessage);
                    var header = Encoding.ASCII.GetBytes("Content-Length: " + message.Length.ToString(CultureInfo.InvariantCulture) + "\r\n\r\n");
                    incoming = new byte[header.Length + message.Length];
                    Buffer.BlockCopy(header, 0, incoming, 0, header.Length);
                    Buffer.BlockCopy(message.GetBuffer(), 0, incoming, header.Length, (int)message.Length);
                    incomingOffset = 0;
                }
                var copied = Math.Min(count, incoming.Length - incomingOffset);
                Buffer.BlockCopy(incoming, incomingOffset, buffer, offset, copied);
                incomingOffset += copied;
                return copied;
            }
            finally { reading.Release(); }
        }

        public override async Task WriteAsync(byte[] buffer, int offset, int count, CancellationToken token)
        {
            ValidateBuffer(buffer, offset, count);
            await writing.WaitAsync(token).ConfigureAwait(false);
            try
            {
                if (pending.Length + count > MaxMessageBytes + 8192) throw new InvalidDataException("LSP frame exceeds 64 MiB.");
                pending.Position = pending.Length;
                pending.Write(buffer, offset, count);
                while (pending.Length > 0)
                {
                    var bytes = pending.GetBuffer();
                    var end = -1;
                    for (var i = 0; i + 3 < pending.Length && i < 8192; i++)
                        if (bytes[i] == 13 && bytes[i + 1] == 10 && bytes[i + 2] == 13 && bytes[i + 3] == 10) { end = i + 4; break; }
                    if (end < 0)
                    {
                        if (pending.Length > 8192) throw new InvalidDataException("Invalid LSP header.");
                        return;
                    }
                    int? length = null;
                    foreach (var line in Encoding.ASCII.GetString(bytes, 0, end - 4).Split(["\r\n"], StringSplitOptions.None))
                    {
                        var split = line.IndexOf(':');
                        if (split < 0 || !line.Substring(0, split).Equals("Content-Length", StringComparison.OrdinalIgnoreCase)) continue;
                        if (length.HasValue || !int.TryParse(line.Substring(split + 1).Trim(), NumberStyles.None, CultureInfo.InvariantCulture, out var parsed) || parsed > MaxMessageBytes)
                            throw new InvalidDataException("Invalid Content-Length.");
                        length = parsed;
                    }
                    if (!length.HasValue) throw new InvalidDataException("Missing Content-Length.");
                    if (pending.Length < end + length.Value) return;
                    await socket.SendAsync(new ArraySegment<byte>(bytes, end, length.Value), WebSocketMessageType.Text, true, token).ConfigureAwait(false);
                    var remaining = (int)pending.Length - end - length.Value;
                    Buffer.BlockCopy(bytes, end + length.Value, bytes, 0, remaining);
                    pending.SetLength(remaining);
                }
            }
            finally { writing.Release(); }
        }

        private static void ValidateBuffer(byte[] buffer, int offset, int count)
        {
            if (buffer == null) throw new ArgumentNullException(nameof(buffer));
            if (offset < 0 || count < 0 || offset > buffer.Length - count) throw new ArgumentOutOfRangeException(nameof(count));
        }

        // Stream requires synchronous counterparts; all socket awaits above avoid
        // capturing the UI context. The broker normally uses the async overrides.
#pragma warning disable VSTHRD002
        public override int Read(byte[] buffer, int offset, int count) => ReadAsync(buffer, offset, count, CancellationToken.None).GetAwaiter().GetResult();
        public override void Write(byte[] buffer, int offset, int count) => WriteAsync(buffer, offset, count, CancellationToken.None).GetAwaiter().GetResult();
#pragma warning restore VSTHRD002
        public override void Flush() { }
        public override Task FlushAsync(CancellationToken token) => Task.CompletedTask;
        public override bool CanRead => !disposed;
        public override bool CanWrite => !disposed;
        public override bool CanSeek => false;
        public override long Length => throw new NotSupportedException();
        public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        protected override void Dispose(bool disposing)
        {
            if (disposing && !disposed)
            {
                disposed = true;
                socket.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
