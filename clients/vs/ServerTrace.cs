using System;
using System.Diagnostics;

namespace LspfAnalysis
{
    public enum ServerTrace { Off, Messages, Verbose }

    internal sealed class ServerTraceListener : TraceListener
    {
        private readonly Action<string> log;
        internal ServerTraceListener(Action<string> log) => this.log = log;
        public override void Write(string message) => log("LSP: " + message);
        public override void WriteLine(string message) => Write(message);
    }
}
