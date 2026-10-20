using System.IO;
using System.Linq;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using Flow.Launcher.Infrastructure;
using Flow.Launcher.Resources.Controls;
using NUnit.Framework;

namespace Flow.Launcher.Test;

/// <summary>
/// Loose theme templates defer type resolution until instantiated. Compilation
/// and dictionary parsing alone cannot catch an obsolete host assembly name.
/// </summary>
[TestFixture]
[Apartment(ApartmentState.STA)]
public class ThemeRuntimeTests
{
    [Test]
    public void Launcher_result_template_creates_the_standalone_scroll_viewer()
    {
        var path = Path.Combine(Constant.ProgramDirectory, "Themes", "Base.xaml");
        Assert.That(File.Exists(path), Is.True, "Test the actual copied runtime theme.");
        using var stream = File.OpenRead(path);
        var dictionary = (ResourceDictionary)XamlReader.Load(stream);
        var style = (Style)dictionary["BaseListboxStyle"];
        var setter = style.Setters.OfType<Setter>()
            .Single(value => value.Property == Control.TemplateProperty);
        var template = (ControlTemplate)setter.Value;

        // The user's crash occurs here, not while parsing the dictionary.
        var root = template.LoadContent();
        Assert.That(root, Is.TypeOf<CustomScrollViewerEx>());
        Assert.That(root.GetType().Assembly.GetName().Name, Is.EqualTo("Throw.Launcher"));
    }
}
