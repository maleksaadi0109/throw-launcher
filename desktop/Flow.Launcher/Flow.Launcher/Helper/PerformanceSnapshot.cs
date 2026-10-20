using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Flow.Launcher.Helper;

/// <summary>
/// On-demand measurements only. Enumerates descendants, including external
/// plugin runtimes, without keeping Process objects or a sampling timer alive.
/// </summary>
internal static class PerformanceSnapshot
{
    public static string Capture()
    {
        using var current = Process.GetCurrentProcess();
        current.Refresh();
        var descendants = GetDescendants((uint)current.Id);
        long workingSet = current.WorkingSet64;
        long privateBytes = current.PrivateMemorySize64;
        var measured = 0;
        var unavailable = 0;
        foreach (var id in descendants)
        {
            try
            {
                using var child = Process.GetProcessById(checked((int)id));
                child.Refresh();
                workingSet += child.WorkingSet64;
                privateBytes += child.PrivateMemorySize64;
                measured++;
            }
            catch (Exception exception) when (exception is ArgumentException or InvalidOperationException or Win32Exception)
            {
                // Processes may exit, or deny access, between enumeration and measurement.
                unavailable++;
            }
        }
        static string MiB(long bytes) => $"{bytes / (1024.0 * 1024):F1} MiB";
        return $"Sample: {DateTimeOffset.Now:g}{Environment.NewLine}" +
            $"Launcher working set: {MiB(current.WorkingSet64)}; private bytes: {MiB(current.PrivateMemorySize64)}.{Environment.NewLine}" +
            $"Launcher + {measured} descendant processes: working set {MiB(workingSet)}; private bytes {MiB(privateBytes)}.{Environment.NewLine}" +
            $"Managed heap (launcher only): {MiB(GC.GetTotalMemory(false))}. Unavailable/exited descendants: {unavailable}.{Environment.NewLine}" +
            "Descendants may include apps opened by the launcher, not just plugins.";
    }

    private static HashSet<uint> GetDescendants(uint root)
    {
        const uint snapshotProcesses = 0x00000002;
        using var snapshot = CreateToolhelp32Snapshot(snapshotProcesses, 0);
        if (snapshot.IsInvalid)
            throw new Win32Exception(Marshal.GetLastWin32Error());

        var parents = new Dictionary<uint, uint>();
        var entry = new ProcessEntry { Size = (uint)Marshal.SizeOf<ProcessEntry>() };
        if (!Process32First(snapshot, ref entry))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        do
        {
            parents[entry.ProcessId] = entry.ParentProcessId;
        } while (Process32Next(snapshot, ref entry));
        var lastError = Marshal.GetLastWin32Error();
        if (lastError != 18) // ERROR_NO_MORE_FILES
            throw new Win32Exception(lastError);

        var found = new HashSet<uint> { root };
        bool changed;
        do
        {
            changed = false;
            foreach (var (id, parent) in parents)
                if (found.Contains(parent))
                    changed |= found.Add(id);
        } while (changed);
        found.Remove(root);
        return found;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct ProcessEntry
    {
        public uint Size;
        public uint Usage;
        public uint ProcessId;
        public UIntPtr DefaultHeapId;
        public uint ModuleId;
        public uint Threads;
        public uint ParentProcessId;
        public int Priority;
        public uint Flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string Executable;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern SafeFileHandle CreateToolhelp32Snapshot(uint flags, uint processId);
    [DllImport("kernel32.dll", EntryPoint = "Process32FirstW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool Process32First(SafeFileHandle snapshot, ref ProcessEntry entry);
    [DllImport("kernel32.dll", EntryPoint = "Process32NextW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool Process32Next(SafeFileHandle snapshot, ref ProcessEntry entry);
}
