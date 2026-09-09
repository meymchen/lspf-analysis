using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace LspfAnalysis
{
    // Owns the OS process only. Visual Studio owns the LSP connection and protocol.
    internal sealed class ServerProcess : IDisposable
    {
        private static readonly object processStartGate = new object();
        private readonly Action<string> log;
        private Process process;

        internal ServerProcess(Action<string> log) => this.log = log;
        internal bool IsRunning => process != null && !process.HasExited;
        internal Stream Reader => process.StandardOutput.BaseStream;
        internal Stream Writer => process.StandardInput.BaseStream;

        internal void Start(string executable, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (IsRunning)
                throw new InvalidOperationException("Language server is already running.");
            Dispose();
            if (!Path.IsPathRooted(executable) || !File.Exists(executable))
                throw new FileNotFoundException("Language server executable was not found. Check Server executable in Options, or rebuild/reinstall the VSIX.", executable);

            log($"Launching: \"{executable}\" serve --stdio");
            var start = new ProcessStartInfo(executable, "serve --stdio")
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                WorkingDirectory = Path.GetDirectoryName(executable),
            };
            start.EnvironmentVariables.Remove("LSPF_ANALYSIS_LOG_FILE");
            if (string.IsNullOrEmpty(start.EnvironmentVariables["RUST_LOG"]))
                start.EnvironmentVariables["RUST_LOG"] = "info";
            process = new Process { StartInfo = start };
            process.ErrorDataReceived += OnErrorDataReceived;
            try
            {
                if (!StartWithoutInputPreamble(process))
                    throw new InvalidOperationException("Could not start the language server.");
                process.BeginErrorReadLine();
                cancellationToken.ThrowIfCancellationRequested();
                log($"Starting language server (PID {process.Id}).");
            }
            catch
            {
                Dispose();
                throw;
            }
        }

        internal async Task WaitForExitAsync()
        {
            var child = process;
            if (child == null)
                return;
            try
            {
                if (!await Task.Run(() => child.WaitForExit(5000)).ConfigureAwait(false))
                    throw new TimeoutException("Language server did not exit within five seconds after Visual Studio stopped it.");
                log($"Language server stopped (exit code {child.ExitCode}).");
            }
            finally
            {
                Dispose();
            }
        }

        private void OnErrorDataReceived(object sender, DataReceivedEventArgs e)
        {
            if (e.Data != null)
                log(e.Data);
        }

        private static bool StartWithoutInputPreamble(Process child)
        {
            // Framework auto-flushes a StreamWriter during Process.Start, even
            // though the VS client only uses BaseStream. Suppress its UTF-8 BOM
            // without changing the console's code page, then restore the setting.
            lock (processStartGate)
            {
                var original = Console.InputEncoding;
                if (original.CodePage != Encoding.UTF8.CodePage || original.GetPreamble().Length == 0)
                    return child.Start();
                try
                {
                    Console.InputEncoding = new UTF8Encoding(false);
                    return child.Start();
                }
                finally
                {
                    Console.InputEncoding = original;
                }
            }
        }

        public void Dispose()
        {
            if (process == null)
                return;
            try
            {
                process.ErrorDataReceived -= OnErrorDataReceived;
                try
                {
                    if (!process.HasExited)
                    {
                        log("Terminating language server after an incomplete LSP session.");
                        process.Kill();
                        process.WaitForExit(5000);
                    }
                }
                catch (InvalidOperationException) { } // Already exited, or Start failed.
            }
            finally
            {
                process.Dispose();
                process = null;
            }
        }
    }
}
