using System.IO;
using System.Linq;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Xml.Linq;
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
        var source = XDocument.Load(path);
        XNamespace presentation = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
        XNamespace xaml = "http://schemas.microsoft.com/winfx/2006/xaml";
        // The real app supplies this ambient accent through App.xaml's
        // ThemeResources. Supply it locally without booting application
        // services or changing the copied theme or its control template.
        source.Root!.AddFirst(new XElement(presentation + "SolidColorBrush",
            new XAttribute(xaml + "Key", "SystemAccentColorLight1Brush"),
            new XAttribute("Color", "#FD662B")));
        var dictionary = (ResourceDictionary)XamlReader.Parse(source.ToString());
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
