param(
    [string]$Source = (Join-Path $PSScriptRoot '../static/icons/skilvella.png'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../static/icons')
)

$ErrorActionPreference = 'Stop'
$sourcePath = [IO.Path]::GetFullPath($Source)
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
foreach ($path in @($sourcePath, $outputPath)) {
    $current = $path
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            $entry = Get-Item -LiteralPath $current -Force
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point: $current" }
        }
        $current = [IO.Path]::GetDirectoryName($current)
    }
}
if (!(Test-Path -LiteralPath $outputPath -PathType Container)) { throw 'Output directory must exist' }
Add-Type -AssemblyName System.Drawing
$original = [Drawing.Bitmap]::new($sourcePath)
try {
    if ($original.Width -ne $original.Height) { throw 'A square source image is required' }
    $frames = @{}
    foreach ($size in @(16, 20, 24, 32, 48, 64, 128, 180, 256, 512, 1024)) {
        $bitmap = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $buffer = [IO.MemoryStream]::new()
        try {
            $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.DrawImage($original, [Drawing.Rectangle]::new(0, 0, $size, $size))
            $bitmap.Save($buffer, [Drawing.Imaging.ImageFormat]::Png)
            $bytes = $buffer.ToArray()
            [IO.File]::WriteAllBytes((Join-Path $outputPath "icon-$size.png"), $bytes)
            if ($size -le 256 -and $size -ne 180) { $frames[$size] = $bytes }
        } finally {
            $buffer.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
        }
    }
    # ICO の各フレームに同じ透過画像の PNG を格納する。図柄の再生成・編集は行わない。
    $stream = [IO.MemoryStream]::new()
    $writer = [IO.BinaryWriter]::new($stream)
    try {
        $sizes = @($frames.Keys | Sort-Object)
        $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
        $offset = 6 + 16 * $sizes.Count
        foreach ($size in $sizes) {
            $dimension = if ($size -eq 256) { 0 } else { $size }
            $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
            $writer.Write([byte]0); $writer.Write([byte]0)
            $writer.Write([uint16]1); $writer.Write([uint16]32)
            $writer.Write([uint32]$frames[$size].Length); $writer.Write([uint32]$offset)
            $offset += $frames[$size].Length
        }
        foreach ($size in $sizes) { $writer.Write([byte[]]$frames[$size]) }
        $writer.Flush()
        [IO.File]::WriteAllBytes((Join-Path $outputPath 'skilvella.ico'), $stream.ToArray())
    } finally { $writer.Dispose(); $stream.Dispose() }
} finally { $original.Dispose() }
