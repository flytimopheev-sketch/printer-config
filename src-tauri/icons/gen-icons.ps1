Add-Type -AssemblyName System.Drawing

$size = 256
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear([System.Drawing.Color]::Transparent)

# Фон - скруглённый квадрат с градиентом
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$r = 48
$path.AddArc(0, 0, $r*2, $r*2, 180, 90)
$path.AddArc($size - $r*2, 0, $r*2, $r*2, 270, 90)
$path.AddArc($size - $r*2, $size - $r*2, $r*2, $r*2, 0, 90)
$path.AddArc(0, $size - $r*2, $r*2, $r*2, 90, 90)
$path.CloseFigure()
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
  (New-Object System.Drawing.Point(0,0)),
  (New-Object System.Drawing.Point($size,$size)),
  [System.Drawing.Color]::FromArgb(255, 37, 99, 235),
  [System.Drawing.Color]::FromArgb(255, 14, 165, 233))
$g.FillPath($brush, $path)

# Белый "принтер"
$white = [System.Drawing.Brushes]::White
$g.FillRectangle($white, 62, 130, 132, 62)
$g.FillRectangle($white, 88, 78, 80, 60)
$g.FillRectangle((New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(70,255,255,255))), 74, 150, 108, 22)
$g.FillRectangle($white, 88, 176, 80, 34)

$g.Dispose()

$pngPath = Join-Path $PSScriptRoot 'icon.png'
$bmp.Save($pngPath, [System.Drawing.Imaging.ImageFormat]::Png)

# ICO с PNG-данными внутри
$pngBytes = [System.IO.File]::ReadAllBytes($pngPath)
$icoPath = Join-Path $PSScriptRoot 'icon.ico'
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([UInt16]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]1)
$bw.Write([Byte]0)
$bw.Write([Byte]0)
$bw.Write([Byte]0)
$bw.Write([Byte]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]32)
$bw.Write([UInt32]$pngBytes.Length)
$bw.Write([UInt32]22)
$bw.Write($pngBytes)
$bw.Close()
$fs.Close()
$bmp.Dispose()

Get-ChildItem $PSScriptRoot | Select-Object Name, Length
