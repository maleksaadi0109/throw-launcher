$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$output = [IO.Path]::GetFullPath("$PSScriptRoot/../../../generated-artifacts/throw-rust")
New-Item -ItemType Directory -Force $output | Out-Null
$logo = [Drawing.Image]::FromFile([IO.Path]::GetFullPath("$PSScriptRoot/../../../branding/throw-launcher.png"))
try {
    foreach ($size in @(@(328,628,'setup-sidebar'), @(110,110,'setup-small'))) {
        $bitmap = [Drawing.Bitmap]::new([int]$size[0], [int]$size[1])
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
            if ($size[0] -eq 328) {
                $graphics.Clear([Drawing.Color]::FromArgb(244,246,252))
                $graphics.DrawImage($logo, 58, 72, 212, 212)
                $font = [Drawing.Font]::new('Segoe UI', 35, [Drawing.FontStyle]::Bold, [Drawing.GraphicsUnit]::Pixel)
                $small = [Drawing.Font]::new('Segoe UI', 22, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
                $brush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(25,36,64))
                try {
                    $graphics.DrawString('Throw', $font, $brush, 109, 306)
                    $graphics.DrawString("Your apps.`nOne shortcut.", $small, $brush, 80, 474)
                } finally { $font.Dispose(); $small.Dispose(); $brush.Dispose() }
            } else {
                $graphics.Clear([Drawing.Color]::White)
                $graphics.DrawImage($logo, 2, 2, 106, 106)
            }
            $bitmap.Save("$output/$($size[2]).bmp", [Drawing.Imaging.ImageFormat]::Bmp)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
} finally { $logo.Dispose() }
