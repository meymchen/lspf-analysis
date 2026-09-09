using System;
using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;

namespace LspfAnalysis
{
    internal static class TcpTransportSmoke
    {
        private static async Task<int> Main()
        {
            try
            {
                Require(DebugTcpConnection.ParsePort(null) == 9257, "Default port");
                Require(DebugTcpConnection.ParsePort(" 12345 ") == 12345, "Custom port");
                foreach (var invalid in new[] { "0", "65536", "-1", "abc", "9257.5" })
                {
                    try { DebugTcpConnection.ParsePort(invalid); throw new Exception("Accepted invalid port: " + invalid); }
                    catch (ArgumentException) { }
                }

                var listener = new TcpListener(IPAddress.Loopback, 0);
                listener.Start();
                var port = ((IPEndPoint)listener.LocalEndpoint).Port;
                listener.Stop();
                listener = new TcpListener(IPAddress.Loopback, port);
                try
                {
                    // The server starts after the first connection attempt.
                    var connecting = DebugTcpConnection.ConnectAsync(port, CancellationToken.None);
                    await Task.Delay(300);
                    listener.Start();
                    using (var socket = await connecting)
                    using (var peer = await listener.AcceptTcpClientAsync())
                    {
                        await peer.GetStream().WriteAsync(new byte[] { 42 }, 0, 1);
                        var bytes = new byte[1];
                        Require(await socket.GetStream().ReadAsync(bytes, 0, 1) == 1 && bytes[0] == 42, "Connected stream");
                        socket.Close();
                        var eof = peer.GetStream().ReadAsync(bytes, 0, 1);
                        Require(await Task.WhenAny(eof, Task.Delay(2000)) == eof && await eof == 0, "Socket disposal");
                    }
                }
                finally { listener.Stop(); }

                var elapsed = Stopwatch.StartNew();
                using (var cancellation = new CancellationTokenSource(300))
                {
                    try { using (await DebugTcpConnection.ConnectAsync(port, cancellation.Token)) { throw new Exception("Connected without a listener"); } }
                    catch (OperationCanceledException) { }
                }
                Require(elapsed.Elapsed < TimeSpan.FromSeconds(5), "Cancellation responsiveness");
                Console.WriteLine("PASS: port validation, delayed listener retry, stream transfer, socket disposal, cancellation.");
                return 0;
            }
            catch (Exception error) { Console.Error.WriteLine(error); return 1; }
        }

        private static void Require(bool condition, string message)
        {
            if (!condition) throw new Exception(message);
        }
    }
}
