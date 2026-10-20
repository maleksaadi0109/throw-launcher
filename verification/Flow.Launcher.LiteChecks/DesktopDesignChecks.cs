using System.Security.Cryptography;
using System.Text.RegularExpressions;
using System.Xml.Linq;

// Portable contract checks only. These do not instantiate or render WPF.
internal static class DesktopDesignChecks
{
    internal static void Run(string workspace)
    {
        var app = Path.Combine(workspace, "desktop", "Flow.Launcher", "Flow.Launcher");
        XNamespace x = "http://schemas.microsoft.com/winfx/2006/xaml";
        string[] files =
        [
            "App.xaml", "MainWindow.xaml", "SettingWindow.xaml", "Resources/SettingWindowStyle.xaml",
            "SettingPages/Views/SettingsPaneGeneral.xaml", "SettingPages/Views/SettingsPaneHotkey.xaml",
            "SettingPages/Views/SettingsPaneAbout.xaml", "SettingPages/Views/SettingsPaneTheme.xaml", "Themes/Throw.xaml"
        ];
        foreach (var file in files) XDocument.Load(Path.Combine(app, file));
        Check(true, "Throw views and theme are well-formed XML (not WPF compilation).");

        var window = XDocument.Load(Path.Combine(app, "SettingWindow.xaml"));
        var menu = window.Descendants().Single(e => e.Name.LocalName == "NavigationView.MenuItems");
        var names = menu.Elements().Select(e => (string?)e.Attribute(x + "Name")).ToArray();
        Check(names.SequenceEqual(new[] { "General", "Theme", "Hotkey", "About" }),
            "Settings exposes only Essentials, Themes, Shortcuts and About.");
        var navigationCode = File.ReadAllText(Path.Combine(app, "SettingWindow.xaml.cs"));
        Check(!navigationCode.Contains("nameof(Plugins)") && !navigationCode.Contains("nameof(PluginStore)") &&
              !navigationCode.Contains("nameof(Proxy)"),
            "Navigation has no references to removed menu elements.");
        foreach (var (page, index) in new[] { ("General", 0), ("Theme", 1), ("Hotkey", 2), ("About", 3) })
            Check(navigationCode.Contains($"nameof(SettingsPane{page}) => {index}"),
                $"{page} page selection matches its visible navigation index.");

        var general = File.ReadAllText(Path.Combine(app, "SettingPages/Views/SettingsPaneGeneral.xaml"));
        foreach (var binding in new[] { "Settings.LiteMode", "Settings.ResourceRestartRequired",
                     "StartFlowLauncherOnSystemStartup", "Settings.HideOnStartup", "Settings.HideWhenDeactivated",
                     "RefreshPerformanceCommand", "CopyPerformanceReportCommand", "ExportPerformanceReportCommand" })
            Check(general.Contains($"{{Binding {binding}"), $"Essentials retains {binding} binding.");

        var theme = XDocument.Load(Path.Combine(app, "Themes/Throw.xaml"));
        var dictionaries = new[] { theme, XDocument.Load(Path.Combine(app, "Themes/Midnight.xaml")),
            XDocument.Load(Path.Combine(app, "Themes/Base.xaml")) };
        var keys = dictionaries.SelectMany(doc => doc.Descendants())
            .Select(e => (string?)e.Attribute(x + "Key")).Where(key => key != null).ToHashSet();
        foreach (var match in Regex.Matches(theme.ToString(), @"\{StaticResource ([^}]+)\}").Cast<Match>())
            Check(keys.Contains(match.Groups[1].Value),
                $"Throw theme reference resolves in its theme chain: {match.Groups[1].Value}.");

        var project = XDocument.Load(Path.Combine(app, "Flow.Launcher.csproj"));
        var iconPath = project.Descendants().Single(e => e.Name.LocalName == "ApplicationIcon").Value;
        var brandIcon = File.ReadAllBytes(Path.Combine(workspace, "branding", "throw-launcher.ico"));
        foreach (var relative in new[] { iconPath, "Images/app.ico" })
            Check(SHA256.HashData(brandIcon).SequenceEqual(SHA256.HashData(
                    File.ReadAllBytes(Path.Combine(app, relative.Replace('\\', Path.DirectorySeparatorChar))))),
                $"Throw icon matches executable/window source: {relative}.");
        Check(!File.ReadAllText(Path.Combine(app, "SettingPages/Views/SettingsPaneAbout.xaml"))
                  .Contains("UpdateAppCommand"),
            "Simplified About does not offer to replace the fork with upstream updates.");
    }

    private static void Check(bool condition, string description)
    {
        if (!condition) throw new Exception(description);
        Console.WriteLine($"PASS {description}");
    }
}
