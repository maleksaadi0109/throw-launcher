using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows.Forms;
using CommunityToolkit.Mvvm.Input;
using Flow.Launcher.Core;
using Flow.Launcher.Core.Configuration;
using Flow.Launcher.Core.Resource;
using Flow.Launcher.Helper;
using Flow.Launcher.Core.Plugin;
using Flow.Launcher.Infrastructure.Image;
using Flow.Launcher.Infrastructure;
using Flow.Launcher.Infrastructure.DialogJump;
using Flow.Launcher.Infrastructure.UserSettings;
using Flow.Launcher.Plugin;
using Flow.Launcher.Plugin.SharedModels;

namespace Flow.Launcher.SettingPages.ViewModels;

public partial class SettingsPaneGeneralViewModel : BaseModel
{
    public Settings Settings { get; }

    public int[] LiteIconCacheSizes { get; } = [25, 50, 100, 150];
    private string _performanceSummary = "No sample collected. Refresh to measure current usage; no background monitor is running.";
    public string PerformanceSummary
    {
        get => _performanceSummary;
        private set
        {
            _performanceSummary = value;
            OnPropertyChanged();
        }
    }

    public bool HasPerformanceSnapshot { get; private set; }
    public string ReportActionStatus { get; private set; } = string.Empty;
    public string ImportStatus { get; private set; } =
        "Optional: copy four general preferences from Flow. No plugins, secrets, paths, themes or shortcuts are imported.";

    [RelayCommand]
    private async System.Threading.Tasks.Task ImportPreferencesAsync()
    {
        try
        {
            var dialog = new Microsoft.Win32.OpenFileDialog
            {
                Title = "Import general preferences from Flow Launcher",
                Filter = "Settings JSON (*.json)|*.json",
                CheckFileExists = true,
                Multiselect = false,
                InitialDirectory = System.IO.Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                    "FlowLauncher", "Settings")
            };
            if (dialog.ShowDialog(System.Windows.Application.Current.MainWindow) != true)
                return;
            var target = System.IO.Path.Combine(DataLocation.SettingsDirectory, "Settings.json");
            if (string.Equals(System.IO.Path.GetFullPath(dialog.FileName),
                    System.IO.Path.GetFullPath(target), StringComparison.OrdinalIgnoreCase))
                throw new FormatException("Choose Flow's Settings.json, not the active Throw profile.");
            IReadOnlyDictionary<string, bool> values;
            await using (var source = new System.IO.FileStream(dialog.FileName,
                System.IO.FileMode.Open, System.IO.FileAccess.Read, System.IO.FileShare.Read))
            {
                if (source.Length > PreferenceImport.MaximumBytes)
                    throw new FormatException("Settings file must be at most 1 MB.");
                using var reader = new System.IO.StreamReader(source);
                values = PreferenceImport.Parse(await reader.ReadToEndAsync());
            }
            var confirmation = System.Windows.MessageBox.Show(
                $"Import {values.Count} general preferences into Throw?{Environment.NewLine}" +
                "Hide on startup, hide on focus loss, animation preference and accent matching are supported. " +
                "Current Throw settings will be backed up first. Flow files are read-only. " +
                "Lite mode stays enabled; shortcuts, credentials, themes and plugins are not copied.",
                "Import into Throw Launcher", System.Windows.MessageBoxButton.YesNo,
                System.Windows.MessageBoxImage.Question);
            if (confirmation != System.Windows.MessageBoxResult.Yes)
                return;
            if (System.IO.File.Exists(target))
                System.IO.File.Copy(target, target + $".before-import-{Guid.NewGuid():N}.bak", false);
            var previous = (Settings.HideOnStartup, Settings.HideWhenDeactivated,
                Settings.UseAnimation, Settings.IgnoreAccents, Settings.LiteMode);
            // Parse/validate the whole file before applying any supported values.
            try
            {
                if (values.TryGetValue("HideOnStartup", out var hide)) Settings.HideOnStartup = hide;
                if (values.TryGetValue("HideWhenDeactivated", out var focus)) Settings.HideWhenDeactivated = focus;
                if (values.TryGetValue("UseAnimation", out var animation)) Settings.UseAnimation = animation;
                if (values.TryGetValue("IgnoreAccents", out var accents)) Settings.IgnoreAccents = accents;
                Settings.LiteMode = true;
                Settings.SaveChecked();
            }
            catch
            {
                (Settings.HideOnStartup, Settings.HideWhenDeactivated,
                    Settings.UseAnimation, Settings.IgnoreAccents, Settings.LiteMode) = previous;
                throw;
            }
            ImportStatus = $"Imported {values.Count} preferences. Source unchanged; Lite mode remains on. Throw's previous saved profile was backed up if present.";
        }
        catch (Exception exception)
        {
            App.API.LogException(nameof(SettingsPaneGeneralViewModel), "Preference import failed", exception);
            ImportStatus = $"Import failed: {exception.Message}";
        }
        OnPropertyChanged(nameof(ImportStatus));
    }

    [RelayCommand(CanExecute = nameof(HasPerformanceSnapshot))]
    private void CopyPerformanceReport()
    {
        try
        {
            System.Windows.Clipboard.SetText(GetPerformanceReport());
            ReportActionStatus = "Performance report copied. It contains resource statistics, not full logs.";
        }
        catch (Exception exception)
        {
            App.API.LogException(nameof(SettingsPaneGeneralViewModel), "Unable to copy performance report", exception);
            ReportActionStatus = $"Copy failed: {exception.Message}";
        }
        OnPropertyChanged(nameof(ReportActionStatus));
    }

    [RelayCommand(CanExecute = nameof(HasPerformanceSnapshot))]
    private async System.Threading.Tasks.Task ExportPerformanceReportAsync()
    {
        var dialog = new Microsoft.Win32.SaveFileDialog
        {
            Title = "Export Throw Launcher performance report",
            FileName = $"throw-launcher-resources-{DateTime.Now:yyyyMMdd-HHmmss}.txt",
            Filter = "Text reports (*.txt)|*.txt",
            DefaultExt = ".txt",
            AddExtension = true,
            OverwritePrompt = true
        };
        if (dialog.ShowDialog(System.Windows.Application.Current.MainWindow) != true)
            return;
        try
        {
            await System.IO.File.WriteAllTextAsync(dialog.FileName, GetPerformanceReport(),
                new System.Text.UTF8Encoding(false));
            ReportActionStatus = "Performance report exported. No logs or settings files were included.";
        }
        catch (Exception exception)
        {
            App.API.LogException(nameof(SettingsPaneGeneralViewModel), "Unable to export performance report", exception);
            ReportActionStatus = $"Export failed: {exception.Message}";
        }
        OnPropertyChanged(nameof(ReportActionStatus));
    }

    private string GetPerformanceReport() =>
        $"Throw Launcher — resource snapshot{Environment.NewLine}{Environment.NewLine}" +
        $"{PerformanceSummary}{Environment.NewLine}{Environment.NewLine}" +
        $"Requested Lite mode: {Settings.LiteMode}. Restart required: {Settings.ResourceRestartRequired}.{Environment.NewLine}" +
        "A snapshot is not evidence of a performance improvement. Compare identical Windows workloads.";

    [RelayCommand]
    private async System.Threading.Tasks.Task RefreshPerformanceAsync()
    {
        try
        {
            var summary = await System.Threading.Tasks.Task.Run(PerformanceSnapshot.Capture);
            PerformanceSummary = $"{summary}{Environment.NewLine}" +
                $"Icon cache: {ImageLoader.CachedImageCount} / {ImageLoader.ImageCacheCapacity} entries.{Environment.NewLine}" +
                $"Plugins: {PluginManager.GetAllInitializedPlugins(false).Count} initialized, " +
                $"{PluginManager.DeferredPluginCount} waiting for first use.{Environment.NewLine}" +
                "Working sets can include shared pages. This snapshot is not a benchmark or per-plugin memory attribution.";
            HasPerformanceSnapshot = true;
            ReportActionStatus = string.Empty;
        }
        catch (Exception exception)
        {
            App.API.LogException(nameof(SettingsPaneGeneralViewModel), "Unable to collect performance snapshot", exception);
            PerformanceSummary = $"Unable to collect sample: {exception.Message}";
            HasPerformanceSnapshot = false;
        }
        OnPropertyChanged(nameof(HasPerformanceSnapshot));
        OnPropertyChanged(nameof(ReportActionStatus));
        CopyPerformanceReportCommand.NotifyCanExecuteChanged();
        ExportPerformanceReportCommand.NotifyCanExecuteChanged();
    }

    private readonly Updater _updater;
    private readonly Portable _portable;
    private readonly Internationalization _translater;

    public SettingsPaneGeneralViewModel(Settings settings, Updater updater, Portable portable, Internationalization translater)
    {
        Settings = settings;
        _updater = updater;
        _portable = portable;
        _translater = translater;
        UpdateEnumDropdownLocalizations();
    }

    public class SearchWindowScreenData : DropdownDataGeneric<SearchWindowScreens> { }
    public class SearchWindowAlignData : DropdownDataGeneric<SearchWindowAligns> { }
    public class SearchPrecisionData : DropdownDataGeneric<SearchPrecisionScore> { }
    public class LastQueryModeData : DropdownDataGeneric<LastQueryMode> { }
    public class DoublePinyinSchemaData : DropdownDataGeneric<DoublePinyinSchemas> { }

    public bool StartFlowLauncherOnSystemStartup
    {
        get => Settings.StartFlowLauncherOnSystemStartup;
        set
        {
            Settings.StartFlowLauncherOnSystemStartup = value;

            try
            {
                if (value)
                {
                    if (UseLogonTaskForStartup)
                    {
                        AutoStartup.ChangeToViaLogonTask();
                    }
                    else
                    {
                        AutoStartup.ChangeToViaRegistry();
                    }
                }
                else
                {
                    AutoStartup.DisableViaLogonTaskAndRegistry();
                }  
            }
            catch (Exception e)
            {
                App.API.ShowMsgError(Localize.setAutoStartFailed(), e.Message);
            }
        }
    }

    public bool UseLogonTaskForStartup
    {
        get => Settings.UseLogonTaskForStartup;
        set
        {
            Settings.UseLogonTaskForStartup = value;

            if (StartFlowLauncherOnSystemStartup)
            {
                try
                {
                    if (value)
                    {
                        AutoStartup.ChangeToViaLogonTask();
                    }
                    else
                    {
                        AutoStartup.ChangeToViaRegistry();
                    }
                }
                catch (Exception e)
                {
                    App.API.ShowMsgError(Localize.setAutoStartFailed(), e.Message);
                }
            } 
        }
    }

    public List<SearchWindowScreenData> SearchWindowScreens { get; } =
        DropdownDataGeneric<SearchWindowScreens>.GetValues<SearchWindowScreenData>("SearchWindowScreen");

    public List<SearchWindowAlignData> SearchWindowAligns { get; } =
        DropdownDataGeneric<SearchWindowAligns>.GetValues<SearchWindowAlignData>("SearchWindowAlign");

    public List<SearchPrecisionData> SearchPrecisionScores { get; } =
        DropdownDataGeneric<SearchPrecisionScore>.GetValues<SearchPrecisionData>("SearchPrecision");

    public List<int> ScreenNumbers
    {
        get
        {
            var screens = MonitorInfo.GetDisplayMonitors();
            var screenNumbers = new List<int>();
            for (int i = 1; i <= screens.Count; i++)
            {
                screenNumbers.Add(i);
            }

            return screenNumbers;
        }
    }

    // This is only required to set at startup. When portable mode enabled/disabled a restart is always required
    private static readonly bool _portableMode = DataLocation.PortableDataLocationInUse();

    public bool PortableMode
    {
        get => _portableMode;
        set
        {
            if (!_portable.CanUpdatePortability())
                return;

            if (DataLocation.PortableDataLocationInUse())
            {
                _portable.DisablePortableMode();
            }
            else
            {
                _portable.EnablePortableMode();
            }
        }
    }

    public List<LastQueryModeData> LastQueryModes { get; } =
        DropdownDataGeneric<LastQueryMode>.GetValues<LastQueryModeData>("LastQuery");

    public List<HistoryStyleLocalized> HistoryStyles { get; } = HistoryStyleLocalized.GetValues();

    public bool EnableDialogJump
    {
        get => Settings.EnableDialogJump;
        set
        {
            if (Settings.EnableDialogJump != value)
            {
                Settings.EnableDialogJump = value;
                DialogJump.SetupDialogJump(value);
                if (Settings.EnableDialogJump)
                {
                    HotKeyMapper.SetHotkey(new(Settings.DialogJumpHotkey), DialogJump.OnToggleHotkey);
                }
                else
                {
                    HotKeyMapper.RemoveHotkey(Settings.DialogJumpHotkey);
                }
            }
        }
    }

    public class DialogJumpWindowPositionData : DropdownDataGeneric<DialogJumpWindowPositions> { }
    public class DialogJumpResultBehaviourData : DropdownDataGeneric<DialogJumpResultBehaviours> { }
    public class DialogJumpFileResultBehaviourData : DropdownDataGeneric<DialogJumpFileResultBehaviours> { }

    public List<DialogJumpWindowPositionData> DialogJumpWindowPositions { get; } =
        DropdownDataGeneric<DialogJumpWindowPositions>.GetValues<DialogJumpWindowPositionData>("DialogJumpWindowPosition");

    public List<DialogJumpResultBehaviourData> DialogJumpResultBehaviours { get; } =
        DropdownDataGeneric<DialogJumpResultBehaviours>.GetValues<DialogJumpResultBehaviourData>("DialogJumpResultBehaviour");

    public List<DialogJumpFileResultBehaviourData> DialogJumpFileResultBehaviours { get; } =
        DropdownDataGeneric<DialogJumpFileResultBehaviours>.GetValues<DialogJumpFileResultBehaviourData>("DialogJumpFileResultBehaviour");

    public int SearchDelayTimeValue
    {
        get => Settings.SearchDelayTime;
        set
        {
            if (Settings.SearchDelayTime != value)
            {
                Settings.SearchDelayTime = value;
                OnPropertyChanged();
            }
        }
    }


    public bool IgnoreAccents
    {
        get => Settings.IgnoreAccents;
        set
        {
            if(Settings.IgnoreAccents != value)
            {
                Settings.IgnoreAccents = value;
                OnPropertyChanged();
            }
        }
    }

    public int MaxHistoryResultsToShowValue
    {
        get => Settings.MaxHistoryResultsToShowForHomePage;
        set
        {
            if (Settings.MaxHistoryResultsToShowForHomePage != value)
            {
                Settings.MaxHistoryResultsToShowForHomePage = value;
                OnPropertyChanged();
            }
        }
    }

    private void UpdateEnumDropdownLocalizations()
    {
        DropdownDataGeneric<SearchWindowScreens>.UpdateLabels(SearchWindowScreens);
        DropdownDataGeneric<SearchWindowAligns>.UpdateLabels(SearchWindowAligns);
        DropdownDataGeneric<SearchPrecisionScore>.UpdateLabels(SearchPrecisionScores);
        DropdownDataGeneric<LastQueryMode>.UpdateLabels(LastQueryModes);
        HistoryStyleLocalized.UpdateLabels(HistoryStyles);
        DropdownDataGeneric<DoublePinyinSchemas>.UpdateLabels(DoublePinyinSchemas);
        DropdownDataGeneric<DialogJumpWindowPositions>.UpdateLabels(DialogJumpWindowPositions);
        DropdownDataGeneric<DialogJumpResultBehaviours>.UpdateLabels(DialogJumpResultBehaviours);
        DropdownDataGeneric<DialogJumpFileResultBehaviours>.UpdateLabels(DialogJumpFileResultBehaviours);
        // Since we are using Binding instead of DynamicResource, we need to manually trigger the update
        OnPropertyChanged(nameof(AlwaysPreviewToolTip));
        Settings.CustomExplorer.OnDisplayNameChanged();
        Settings.CustomBrowser.OnDisplayNameChanged();
    }

    public string Crowdin => Constant.CrowdinProjectUrl;

    public string Language
    {
        get => Settings.Language;
        set
        {
            _translater.ChangeLanguage(value);

            if (_translater.PromptShouldUsePinyin(value))
                ShouldUsePinyin = true;

            if (_translater.PromptShouldIgnoreAccents(value))
                IgnoreAccents = true;

            UpdateEnumDropdownLocalizations();
        }
    }

    #region Korean IME

    // The new Korean IME used in Windows 11 has compatibility issues with WPF. This issue is difficult to resolve within
    // WPF itself, but it can be avoided by having the user switch to the legacy IME at the system level. Therefore,
    // we provide guidance and a direct button for users to make this change themselves. If the relevant registry key does
    // not exist (i.e., the Korean IME is not installed), this setting will not be shown at all.

    public bool LegacyKoreanIMEEnabled
    {
        get => Win32Helper.IsLegacyKoreanIMEEnabled();
        set
        {
            if (Win32Helper.SetLegacyKoreanIMEEnabled(value))
            {
                OnPropertyChanged();
                OnPropertyChanged(nameof(KoreanIMERegistryValueIsZero));
            }
            else
            {
                // Since this is rarely seen text, language support is not provided.
                App.API.ShowMsgError(Localize.KoreanImeSettingChangeFailTitle(), Localize.KoreanImeSettingChangeFailSubTitle());
            }
        }
    }

    public bool KoreanIMERegistryKeyExists
    {
        get
        {
            var registryKeyExists = Win32Helper.IsKoreanIMEExist();
            var koreanLanguageInstalled = InputLanguage.InstalledInputLanguages.Cast<InputLanguage>().Any(lang => lang.Culture.Name.StartsWith("ko"));
            var isWindows11 = Win32Helper.IsWindows11();

            // Return true if Windows 11 with Korean IME installed, or if the registry key exists
            return (isWindows11 && koreanLanguageInstalled) || registryKeyExists;
        }
    }

    public bool KoreanIMERegistryValueIsZero
    {
        get
        {
            var value = Win32Helper.GetLegacyKoreanIMERegistryValue();
            if (value is int intValue)
            {
                return intValue == 0;
            }
            else if (value != null && int.TryParse(value.ToString(), out var parsedValue))
            {
                return parsedValue == 0;
            }

            return false;
        }
    }

    [RelayCommand]
    private void OpenImeSettings()
    {
        Win32Helper.OpenImeSettings();
    }

    #endregion

    public bool ShouldUsePinyin
    {
        get => Settings.ShouldUsePinyin;
        set
        {
            if (value == false && UseDoublePinyin == true)
            {
                UseDoublePinyin = false;
            }
            Settings.ShouldUsePinyin = value;
        }
    }

    public bool UseDoublePinyin
    {
        set => Settings.UseDoublePinyin = value;
        get => Settings.UseDoublePinyin;
    }

    public bool UsePolyphonicPhraseOverrides
    {
        set => Settings.UsePolyphonicPhraseOverrides = value;
        get => Settings.UsePolyphonicPhraseOverrides;
    }

    public List<DoublePinyinSchemaData> DoublePinyinSchemas { get; } =
        DropdownDataGeneric<DoublePinyinSchemas>.GetValues<DoublePinyinSchemaData>("DoublePinyinSchemas");

    public List<Language> Languages => _translater.LoadAvailableLanguages();

    public string AlwaysPreviewToolTip => Localize.AlwaysPreviewToolTip(Settings.PreviewHotkey);

    private static string GetFileFromDialog(string title, string filter = "")
    {
        var dlg = new OpenFileDialog
        {
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            Multiselect = false,
            CheckFileExists = true,
            CheckPathExists = true,
            Title = title,
            Filter = filter
        };

        return dlg.ShowDialog() switch
        {
            DialogResult.OK => dlg.FileName,
            _ => string.Empty
        };
    }

    private void UpdateApp()
    {
        _ = _updater.UpdateAppAsync(false);
    }

    public bool AutoUpdates
    {
        get => Settings.AutoUpdates;
        set
        {
            Settings.AutoUpdates = value;

            if (value)
            {
                UpdateApp();
            }
        }
    }

    [RelayCommand]
    private void SelectPython()
    {
        var selectedFile = GetFileFromDialog(
            Localize.selectPythonExecutable(),
            "Python|pythonw.exe"
        );

        if (!string.IsNullOrEmpty(selectedFile))
            Settings.PluginSettings.PythonExecutablePath = selectedFile;
    }

    [RelayCommand]
    private void SelectNode()
    {
        var selectedFile = GetFileFromDialog(
            Localize.selectNodeExecutable(),
            "node|*.exe"
        );

        if (!string.IsNullOrEmpty(selectedFile))
            Settings.PluginSettings.NodeExecutablePath = selectedFile;
    }

    [RelayCommand]
    private void SelectFileManager()
    {
        var fileManagerChangeWindow = new SelectFileManagerWindow();
        fileManagerChangeWindow.ShowDialog();
    }

    [RelayCommand]
    private void SelectBrowser()
    {
        var browserWindow = new SelectBrowserWindow();
        browserWindow.ShowDialog();
    }
}
