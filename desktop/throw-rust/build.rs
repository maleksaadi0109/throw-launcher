fn main() {
    println!("cargo:rerun-if-changed=branding/throw-rust.ico");
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("branding/throw-rust.ico")
            .set("ProductName", "Throw Launcher")
            .set("FileDescription", "Throw Launcher — native Rust edition")
            .set("OriginalFilename", "Throw.Rust.exe")
            .set_manifest(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<assemblyIdentity version="0.1.0.0" processorArchitecture="*" name="Throw.Launcher.Rust" type="win32"/>
<description>Throw Launcher</description>
<dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
</dependentAssembly></dependency>
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
<requestedExecutionLevel level="asInvoker" uiAccess="false"/>
</requestedPrivileges></security></trustInfo>
<application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
<dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
</windowsSettings></application>
<compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
<supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
</application></compatibility>
</assembly>"#,
            )
            .compile()
            .expect("Cannot embed Throw icon and Windows manifest");
    }
}
