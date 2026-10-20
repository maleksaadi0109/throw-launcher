using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Flow.Launcher.Plugin;

namespace Flow.Launcher.Core.Plugin;

/// <summary>
/// Metadata-only stand-in. The manager replaces it before querying or opening
/// settings. It deliberately advertises no background-dependent interfaces.
/// </summary>
internal sealed class DeferredPlugin : IAsyncPlugin
{
    public Task InitAsync(PluginInitContext context) =>
        throw new InvalidOperationException("Deferred plugins must be initialized through PluginManager.");

    public Task<List<Result>> QueryAsync(Query query, CancellationToken token) =>
        throw new InvalidOperationException("Deferred plugin was queried before initialization.");
}
