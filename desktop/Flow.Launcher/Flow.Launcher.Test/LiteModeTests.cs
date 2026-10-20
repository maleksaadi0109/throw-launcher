using System.Text.Json;
using Flow.Launcher.Infrastructure.Image;
using Flow.Launcher.Infrastructure.UserSettings;
using NUnit.Framework;

namespace Flow.Launcher.Test;

[TestFixture]
public class LiteModeTests
{
    [Test]
    public void RestartNoticeTracksChangesAndClearsWhenTheyAreReverted()
    {
        var settings = new Settings { LiteMode = true };
        settings.CaptureActiveResourceProfile();
        Assert.That(settings.ResourceRestartRequired, Is.False);
        settings.LiteIconCacheCapacity = 100;
        Assert.That(settings.ResourceRestartRequired, Is.True);
        settings.LiteIconCacheCapacity = 50;
        Assert.That(settings.ResourceRestartRequired, Is.False);
        settings.PluginSettings.Plugins["example"] = new Infrastructure.UserSettings.Plugin { LoadAtStartup = true };
        Assert.That(settings.ResourceRestartRequired, Is.True);
        settings.PluginSettings.Plugins["example"].LoadAtStartup = false;
        Assert.That(settings.ResourceRestartRequired, Is.False);
    }

    [Test]
    public void NormalModeRetainsOriginalResourcePolicy()
    {
        var settings = new Settings { LiteMode = false, UseAnimation = true };
        Assert.Multiple(() =>
        {
            Assert.That(settings.EffectiveIconCacheCapacity, Is.EqualTo(150));
            Assert.That(settings.EffectivePreloadImages, Is.True);
            Assert.That(settings.EffectiveUseAnimation, Is.True);
        });
    }

    [Test]
    public void LiteModeDoesNotOverwriteNormalAnimationPreference()
    {
        var settings = new Settings { UseAnimation = true, LiteMode = true };
        Assert.That(settings.EffectiveUseAnimation, Is.False);
        Assert.That(settings.UseAnimation, Is.True);
        settings.LiteMode = false;
        Assert.That(settings.EffectiveUseAnimation, Is.True);
        settings.UseAnimation = false;
        settings.LiteMode = true;
        settings.LiteMode = false;
        Assert.That(settings.EffectiveUseAnimation, Is.False);
    }

    [TestCase(-1, 16)]
    [TestCase(0, 16)]
    [TestCase(50, 50)]
    [TestCase(150, 150)]
    [TestCase(int.MaxValue, 150)]
    public void CacheCapacityIsBounded(int input, int expected)
    {
        var settings = new Settings { LiteMode = true, LiteIconCacheCapacity = input };
        Assert.That(settings.EffectiveIconCacheCapacity, Is.EqualTo(expected));
        Assert.That(new ImageCache(input).Capacity, Is.EqualTo(expected));
    }

    [Test]
    public void SerializationPreservesNormalProfileRatherThanEffectiveOverrides()
    {
        var original = new Settings { LiteMode = true, UseAnimation = true };
        var json = JsonSerializer.Serialize(original);
        Assert.That(json, Does.Not.Contain("EffectiveUseAnimation"));
        var restored = JsonSerializer.Deserialize<Settings>(json);
        Assert.That(restored.UseAnimation, Is.True);
        Assert.That(restored.LiteMode, Is.True);
        Assert.That(restored.EffectiveUseAnimation, Is.False);
    }

    [Test]
    public void PreloadingIsOptionalOnlyInLiteMode()
    {
        var settings = new Settings { LiteMode = true, LitePreloadImages = false };
        Assert.That(settings.EffectivePreloadImages, Is.False);
        settings.LitePreloadImages = true;
        Assert.That(settings.EffectivePreloadImages, Is.True);
        settings.LiteMode = false;
        settings.LitePreloadImages = false;
        Assert.That(settings.EffectivePreloadImages, Is.True);
    }
}
