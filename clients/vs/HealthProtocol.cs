using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json.Linq;

namespace LspfAnalysis
{
    // Validate independently versioned server payloads before using them in the UI.
    internal static class HealthProtocol
    {
        internal const string FileHealthMethod = "lspfAnalysis/fileHealth";
        internal const string FunctionHealthMethod = "lspfAnalysis/functionHealth";

        internal static string Language(string path)
        {
            switch (Path.GetExtension(path).ToLowerInvariant())
            {
                case ".cpp": case ".cc": case ".cxx": case ".h": case ".hpp": case ".hxx": return "cpp";
                case ".java": return "java";
                case ".js": case ".mjs": case ".cjs": return "javascript";
                case ".jsx": return "javascriptreact";
                case ".ts": case ".mts": case ".cts": return "typescript";
                case ".tsx": return "typescriptreact";
                case ".py": case ".pyw": return "python";
                case ".rs": return "rust";
                default: return null;
            }
        }

        internal static bool IsLocalUri(string text) =>
            Uri.TryCreate(text, UriKind.Absolute, out var uri) && uri.IsFile && !uri.IsUnc;

        private static bool Text(JToken token) => token?.Type == JTokenType.String;
        internal static bool Number(JToken token) =>
            (token?.Type == JTokenType.Float || token?.Type == JTokenType.Integer) &&
            !double.IsNaN((double)token) && !double.IsInfinity((double)token);
        private static bool Score(JToken token) => Number(token) && (double)token >= 0 && (double)token <= 100;
        private static bool Count(JToken token, int minimum = 0) => token?.Type == JTokenType.Integer &&
            (double)token >= minimum && (double)token <= int.MaxValue;
        private static bool Grade(JToken token) => Text(token) &&
            new[] { "excellent", "good", "fair", "poor" }.Contains((string)token);

        internal static JObject FileHealth(JToken token)
        {
            if (!(token is JObject obj) || !Text(obj["uri"]) || !IsLocalUri((string)obj["uri"]) ||
                !Score(obj["quality"]) || !Grade(obj["grade"]) || !Count(obj["functions"]) || !Count(obj["below"]) ||
                !(obj["bands"] is JObject bands) || !(obj["worst"] is JArray worst)) return null;
            if (!new[] { "excellent", "good", "fair", "poor" }.All(b => Count(bands[b]))) return null;
            if (worst.Any(w => !(w is JObject) || !Text(w["name"]) || !Score(w["quality"]) ||
                !Grade(w["grade"]) || !Count(w["line"], 1) || !Text(w["weakestPillar"]) || !Text(w["weakestMetric"]))) return null;
            return obj;
        }

        internal static JObject FunctionHealth(JToken token)
        {
            if (!(token is JObject obj) || !Text(obj["uri"]) || !IsLocalUri((string)obj["uri"]) ||
                !(obj["functions"] is JArray functions)) return null;
            foreach (var f in functions)
            {
                if (!(f is JObject) || !Text(f["name"]) || !Score(f["quality"]) || !Grade(f["grade"]) ||
                    !Count(f["startLine"], 1) || !Count(f["endLine"], 1) || (int)f["endLine"] < (int)f["startLine"] ||
                    !Text(f["weakestPillar"]) || !Text(f["weakestMetric"]) || !(f["pillars"] is JArray pillars)) return null;
                foreach (var p in pillars)
                {
                    if (!(p is JObject) || !Text(p["name"]) || !Score(p["score"]) || !(p["measures"] is JArray measures)) return null;
                    if (measures.Any(m => !(m is JObject) || !Text(m["name"]) || !Number(m["value"]) ||
                        !Number(m["threshold"]) || (double)m["threshold"] <= 0 || !Score(m["score"]))) return null;
                }
            }
            return obj;
        }

        internal static IEnumerable<JToken> Sort(JObject health, bool byPosition)
        {
            var functions = (JArray)health["functions"];
            return byPosition ? functions.OrderBy(f => (int)f["startLine"]) :
                functions.OrderBy(f => (double)f["quality"]).ThenBy(f => (int)f["startLine"]);
        }

        internal static string Percent(JToken score) => Math.Round((double)score, MidpointRounding.AwayFromZero) + "%";
    }
}
