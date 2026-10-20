using System.Reflection;
using System.Runtime.InteropServices;

#if DEBUG

[assembly: AssemblyConfiguration("Debug")]
[assembly: AssemblyDescription("Throw Launcher debug build — based on Flow Launcher")]
#else
[assembly: AssemblyConfiguration("Release")]
[assembly: AssemblyDescription("Throw Launcher — based on Flow Launcher")]
#endif

[assembly: AssemblyTitle("Throw Launcher")]
[assembly: AssemblyCompany("Throw Launcher contributors")]
[assembly: AssemblyProduct("Throw Launcher")]
[assembly: AssemblyCopyright("The MIT License (MIT)")]
[assembly: AssemblyTrademark("")]
[assembly: AssemblyCulture("")]
[assembly: ComVisible(false)]
[assembly: AssemblyVersion("1.0.0")]
[assembly: AssemblyFileVersion("1.0.0")]
[assembly: AssemblyInformationalVersion("1.0.0")]
