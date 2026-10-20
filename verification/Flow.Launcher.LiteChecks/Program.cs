using Flow.Launcher.Core.Plugin;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using System.Text.Json;
using System.Xml.Linq;

static void Check(bool condition, string message)
{
    if (!condition) throw new Exception(message);
    Console.WriteLine($"PASS {message}");
}

var starts = 0;
var complete = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
var started = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
var initialization = new DeferredPluginInitialization(async () =>
{
    Interlocked.Increment(ref starts);
    started.TrySetResult();
    return await complete.Task;
});
await initialization.CompletionIfStarted;
Check(starts == 0, "Reporting/shutdown does not initialize an unused plugin.");
using var canceled = new CancellationTokenSource();
canceled.Cancel();
try { await initialization.WaitAsync(canceled.Token); }
catch (OperationCanceledException) { }
Check(starts == 0, "Already-canceled search does not initialize an unused plugin.");

using var canceledWaiter = new CancellationTokenSource();
var first = initialization.WaitAsync(canceledWaiter.Token);
await started.Task.WaitAsync(TimeSpan.FromSeconds(5));
var others = Enumerable.Range(0, 32).Select(_ => initialization.WaitAsync()).ToArray();
canceledWaiter.Cancel();
try
{
    await first;
    throw new Exception("Canceled wait unexpectedly completed.");
}
catch (OperationCanceledException) { }
complete.SetResult(true);
Check((await Task.WhenAll(others)).All(result => result), "Canceling one search does not cancel other callers.");
Check(starts == 1, "Concurrent callers initialize a plugin exactly once.");
Check(await initialization.WaitAsync(), "Subsequent queries reuse successful initialization.");
Check(starts == 1, "Subsequent queries do not reinitialize the plugin.");

var failures = 0;
var failed = new DeferredPluginInitialization(() =>
{
    Interlocked.Increment(ref failures);
    return Task.FromResult(false);
});
Check(!await failed.WaitAsync() && !await failed.WaitAsync(), "Initialization failure is retained.");
Check(failures == 1, "Failed initialization does not retry on every keystroke.");
Console.WriteLine("All first-use lifecycle checks passed.");

var workspace = Directory.GetCurrentDirectory();
while (!Directory.Exists(Path.Combine(workspace, "desktop", "Flow.Launcher")))
    workspace = Directory.GetParent(workspace)?.FullName ?? throw new Exception("Run from this project.");
var sourceRoot = Path.Combine(workspace, "desktop", "Flow.Launcher");
var sourceFiles = Directory.EnumerateFiles(sourceRoot, "*.cs", SearchOption.AllDirectories)
    .Where(path => !path.Split(Path.DirectorySeparatorChar).Any(part => part is "obj" or "bin" or "Output"))
    .ToArray();
var syntaxErrors = sourceFiles.SelectMany(path => CSharpSyntaxTree.ParseText(
        File.ReadAllText(path), new CSharpParseOptions(LanguageVersion.CSharp13), path: path)
    .GetDiagnostics().Where(diagnostic => diagnostic.Severity == DiagnosticSeverity.Error)).ToArray();
foreach (var diagnostic in syntaxErrors) Console.Error.WriteLine(diagnostic);
Check(syntaxErrors.Length == 0, $"C# syntax validation ({sourceFiles.Length} source files).");

string[] changedXml =
[
    "Flow.Launcher/Flow.Launcher.csproj",
    "Flow.Launcher/SettingPages/Views/SettingsPaneGeneral.xaml",
    "Flow.Launcher/Resources/Controls/InstalledPluginDisplayBottomData.xaml"
];
foreach (var path in changedXml) XDocument.Load(Path.Combine(sourceRoot, path));
Check(true, "Modified project and XAML files are well-formed XML (not a WPF render check).");
foreach (var plugin in new[] { "Calculator", "Url" })
{
    using var manifest = JsonDocument.Parse(File.ReadAllText(Path.Combine(sourceRoot,
        "Plugins", $"Flow.Launcher.Plugin.{plugin}", "plugin.json")));
    Check(manifest.RootElement.GetProperty("SupportsLazyInitialization").GetBoolean(),
        $"{plugin} explicitly opts into first-use loading.");
}
DesktopDesignChecks.Run(workspace);
var imported = Flow.Launcher.Infrastructure.UserSettings.PreferenceImport.Parse(
    """{"HideOnStartup":true,"UseAnimation":false,"LiteMode":false,"PythonPath":"evil.exe","Hotkey":"Alt+Space"}""");
Check(imported.Count == 2 && imported["HideOnStartup"] && !imported["UseAnimation"],
    "Preference import allows only safe boolean fields, not Lite overrides, paths or hotkeys.");
foreach (var invalid in new[]
{
    "[]", """{"HideOnStartup":"yes"}""", """{"HideOnStartup":true,"HideOnStartup":false}""",
    """{"PythonPath":"evil.exe"}""", new string(' ', 1024 * 1024 + 1)
})
{
    var rejected = false;
    try { Flow.Launcher.Infrastructure.UserSettings.PreferenceImport.Parse(invalid); }
    catch (Exception e) when (e is FormatException or System.Text.Json.JsonException) { rejected = true; }
    Check(rejected, "Malformed, duplicate, unsupported-only or oversized import is rejected.");
}
Console.WriteLine("Windows semantic compilation, UI tests and memory measurements are still required.");
