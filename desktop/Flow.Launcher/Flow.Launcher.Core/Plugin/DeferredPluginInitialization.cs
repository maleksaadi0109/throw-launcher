#nullable enable

using System;
using System.Threading;
using System.Threading.Tasks;

namespace Flow.Launcher.Core.Plugin;

/// <summary>
/// A shared first-use operation. Individual query cancellation does not cancel
/// initialization; failed results are retained rather than retried on each key.
/// </summary>
internal sealed class DeferredPluginInitialization
{
    private readonly object _gate = new();
    private readonly Func<Task<bool>> _initialize;
    private Task<bool>? _initialization;

    public DeferredPluginInitialization(Func<Task<bool>> initialize)
    {
        ArgumentNullException.ThrowIfNull(initialize);
        _initialize = initialize;
    }

    public Task<bool> CompletionIfStarted
    {
        get
        {
            lock (_gate)
                return _initialization ?? Task.FromResult(false);
        }
    }

    public Task<bool> WaitAsync(CancellationToken token = default)
    {
        token.ThrowIfCancellationRequested();
        Task<bool> initialization;
        lock (_gate)
        {
            // Queue the factory without running plugin code under this lock.
            initialization = _initialization ??= Task.Run(_initialize);
        }
        return initialization.WaitAsync(token);
    }
}
