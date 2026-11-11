# Windows SDKとVelopackをつなぐ開発用の梱包入口。
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $WorkDirectory,
    [switch] $Sign
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if (!$IsWindows -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'Pack the installer on Windows x64.'
}
if (![IO.Path]::IsPathFullyQualified($WorkDirectory)) {
    throw 'Specify an absolute path to an empty work directory.'
}
$work = Get-Item -LiteralPath $WorkDirectory -Force
if (!$work.PSIsContainer -or
    (Get-ChildItem -LiteralPath $work.FullName -Force | Select-Object -First 1)) {
    throw 'The work directory must be an empty, ordinary directory.'
}
for ($folder = $work; $null -ne $folder; $folder = $folder.Parent) {
    if ($folder.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "The work path contains a reparse point: $($folder.FullName)"
    }
}
$vpkHelp = vpk --help | Out-String
if ($vpkHelp -notmatch '\bVelopack CLI 1\.2\.161\b') {
    throw 'Velopack CLI 1.2.161 is required.'
}

$signing = @()
if ($Sign) {
    # Taskmetryと同じSimplySign証明書を使い、複数ある場合は有効な証明書を指紋で選ぶ。
    $now = Get-Date
    $certificate = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object {
            $_.Subject -like 'CN=Open Source Developer Yuichiro Shinozaki*' -and
            $_.HasPrivateKey -and $_.NotBefore -le $now -and $_.NotAfter -gt $now
        } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1
    if (!$certificate) { throw 'The code signing certificate is unavailable. Check SimplySign Desktop.' }
    $signing = @(
        '--signParallel', '1',
        '--signParams', "/sha1 $($certificate.Thumbprint) /fd SHA256 /td SHA256 /tr http://time.certum.pl"
    )
}

$repo = Split-Path -Parent $PSScriptRoot
$build = Join-Path $work.FullName 'target'
$payload = Join-Path $work.FullName 'payload'
$packages = Join-Path $work.FullName 'packages'
$temp = Join-Path $work.FullName 'temp'
New-Item -ItemType Directory -Path $payload, $packages, $temp | Out-Null

$previousTemp = $env:TEMP
$previousTmp = $env:TMP
Push-Location -LiteralPath $repo
try {
    # RustとVelopackの一時ファイルを、梱包専用のディレクトリに閉じる。
    $env:TEMP = $temp
    $env:TMP = $temp
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'skilvella').version
    # ZIP配布用のビルドと分離し、Velopackのフックを持つ本体だけを梱包する。
    cargo build --locked --release --target x86_64-pc-windows-msvc --target-dir $build --features windows-installer --bin skilvella
    Copy-Item -LiteralPath (Join-Path $build 'x86_64-pc-windows-msvc/release/skilvella.exe') -Destination $payload
    Copy-Item -LiteralPath (Join-Path $repo 'LICENSE'), (Join-Path $repo 'README.md'), (Join-Path $repo 'CHANGELOG.md') -Destination $payload
    # packIdは既存のデータフォルダー (%LOCALAPPDATA%\Skilvella) と分ける。
    $packOptions = @(
        '--packId', 'SkilvellaApp',
        '--packTitle', 'Skilvella',
        '--packAuthors', 'Kagayoi',
        '--packVersion', $version,
        '--channel', 'win',
        '--runtime', 'win-x64',
        '--instLocation', 'PerUser',
        '--mainExe', 'skilvella.exe',
        '--packDir', $payload,
        '--outputDir', $packages,
        '--icon', (Join-Path $repo 'static/icons/skilvella.ico'),
        '--shortcuts', 'StartMenuRoot',
        '--noPortable',
        '--delta', 'None'
    )
    vpk pack @packOptions @signing
    $setup = Join-Path $packages 'SkilvellaApp-win-Setup.exe'
    if (!(Test-Path -LiteralPath $setup -PathType Leaf)) { throw 'Setup executable was not generated.' }
    if ($Sign) {
        $signature = Get-AuthenticodeSignature -LiteralPath $setup
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint) {
            throw 'Setup failed Authenticode verification with the selected certificate.'
        }
    }
    Get-FileHash -LiteralPath $setup -Algorithm SHA256
} finally {
    $env:TEMP = $previousTemp
    $env:TMP = $previousTmp
    Pop-Location
}
