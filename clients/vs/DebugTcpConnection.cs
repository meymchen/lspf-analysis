using System;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;

namespace LspfAnalysis
{
    internal static class DebugTcpConnection
    {
        internal const string PortVariable = "LSPF_ANALYSIS_DEBUG_PORT";
        internal const int DefaultPort = 9257;

        internal static int ParsePort(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return DefaultPort;
            if (!int.TryParse(value.Trim(), NumberStyles.None, CultureInfo.InvariantCulture, out var port) || port < 1 || port > 65535)
                throw new ArgumentException($"{PortVariable} must be a port number between 1 and 65535.");
            return port;
        }

        internal static async Task<TcpClient> ConnectAsync(int port, CancellationToken cancellationToken)
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeout.CancelAfter(TimeSpan.FromSeconds(20));
            try
            {
                while (true)
                {
                    timeout.Token.ThrowIfCancellationRequested();
                    var socket = new TcpClient(AddressFamily.InterNetwork) { NoDelay = true };
                    try
                    {
                        // Framework's ConnectAsync has no cancellation overload.
                        using (timeout.Token.Register(socket.Close))
                            await socket.ConnectAsync(IPAddress.Loopback, port).ConfigureAwait(false);
                        timeout.Token.ThrowIfCancellationRequested();
                        return socket;
                    }
                    catch (SocketException)
                    {
                        socket.Dispose();
                    }
                    // Framework's EndConnect can also throw NullReferenceException
                    // when cancellation closes the client before its callback runs.
                    catch (Exception) when (timeout.IsCancellationRequested)
                    {
                        socket.Dispose();
                    }
                    catch
                    {
                        socket.Dispose();
                        throw;
                    }
                    await Task.Delay(200, timeout.Token).ConfigureAwait(false);
                }
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                throw new TimeoutException($"No TCP debug server on 127.0.0.1:{port} after 20 seconds. Start lspf-analysis serve --tcp 127.0.0.1:{port} and try again.");
            }
        }
    }
}
