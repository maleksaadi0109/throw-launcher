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
    private readonly Lazy<Task<bool>> _initialization;

    public DeferredPluginInitialization(Func<Task<bool>> initialize)
    {
        ArgumentNullException.ThrowIfNull(initialize);
        _initialization = new Lazy<Task<bool>>(
            () => Task.Run(initialize), LazyThreadSafetyMode.ExecutionAndPublication);
    }

    public Task<bool> CompletionIfStarted =>
        _initialization.IsValueCreated ? _initialization.Value : Task.FromResult(false);

    public Task<bool> WaitAsync(CancellationToken token = default)
    {
        token.ThrowIfCancellationRequested();
        return _initialization.Value.WaitAsync(token);
    }
}
