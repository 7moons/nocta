Add-Type -AssemblyName System.Drawing
$iconDirectory = Join-Path $PSScriptRoot '..\src-tauri\icons'
New-Item -ItemType Directory -Path $iconDirectory -Force | Out-Null
$bitmap = [System.Drawing.Bitmap]::new(256, 256)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.Clear([System.Drawing.Color]::FromArgb(23, 23, 23))
$pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(218, 218, 218), 14)
$pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$graphics.DrawLine($pen, 58, 197, 94, 59)
$graphics.DrawLine($pen, 94, 59, 161, 197)
$graphics.DrawLine($pen, 161, 197, 197, 59)
$graphics.DrawLine($pen, 73, 141, 184, 119)
$bitmap.Save((Join-Path $iconDirectory 'icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$icon = [System.Drawing.Icon]::FromHandle($bitmap.GetHicon())
$stream = [System.IO.File]::Create((Join-Path $iconDirectory 'icon.ico'))
try { $icon.Save($stream) } finally { $stream.Dispose(); $icon.Dispose(); $pen.Dispose(); $graphics.Dispose(); $bitmap.Dispose() }
