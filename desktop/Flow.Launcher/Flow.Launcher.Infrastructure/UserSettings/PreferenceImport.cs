using System;
using System.Collections.Generic;
using System.Text;
using System.Text.Json;

namespace Flow.Launcher.Infrastructure.UserSettings;

/// <summary>
/// Read-only, allow-listed import. Never instantiate Settings from another
/// application's JSON: it could contain paths, executable commands or secrets.
/// </summary>
public static class PreferenceImport
{
    public const int MaximumBytes = 1024 * 1024;
    private static readonly HashSet<string> Allowed = new(StringComparer.Ordinal)
    {
        "HideOnStartup", "HideWhenDeactivated", "UseAnimation", "IgnoreAccents"
    };

    public static IReadOnlyDictionary<string, bool> Parse(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        if (Encoding.UTF8.GetByteCount(json) > MaximumBytes)
            throw new FormatException("Settings file must be at most 1 MB.");
        using var document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 16 });
        if (document.RootElement.ValueKind != JsonValueKind.Object)
            throw new FormatException("Select a Settings.json file containing a JSON object.");
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var values = new Dictionary<string, bool>(StringComparer.Ordinal);
        foreach (var property in document.RootElement.EnumerateObject())
        {
            if (!seen.Add(property.Name))
                throw new FormatException("Duplicate setting names are not accepted.");
            if (!Allowed.Contains(property.Name))
                continue;
            if (property.Value.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
                throw new FormatException($"Setting {property.Name} must be true or false.");
            values.Add(property.Name, property.Value.GetBoolean());
        }
        if (values.Count == 0)
            throw new FormatException("No supported general preferences were found in this file.");
        return values;
    }
}
