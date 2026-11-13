# Windows SDKとVelopackをつなぐ開発用の梱包入口。
#
# -Sign は Sigilune (https://github.com/1llum1n4t1s/Sigilune) で Certum SimplySign のクラウド鍵を使って署名する。
# SimplySign Desktop は不要。手元は `sigilune setup` で登録済みであること、GitHub Actions
# (.github/workflows/release-windows-installer.yml) は Environment "release" の Secrets を
# SIGILUNE_EMAIL / SIGILUNE_TOTP_URI で渡す。
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $WorkDirectory,
    [switch] $Sign,
    # 省略時は作業ディレクトリ内の target。CIは署名用Secretsを渡さない前段のstepで同じ条件の
    # cargo build を済ませ、その出力先を渡す（このスクリプトのcargo buildは再コンパイルしない）。
    [string] $CargoTargetDirectory
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
# GitHub Actions では vpk が色付け用のエスケープシーケンスを出し、`\b` の照合が外れるため取り除いてから比べる。
$vpkHelp = (vpk --help | Out-String) -replace "`e\[[0-9;]*m", ''
if ($vpkHelp -notmatch '\bVelopack CLI 1\.2\.161\b') {
    throw 'Velopack CLI 1.2.161 is required.'
}

$signing = @()
# Taskmetryと同じコード署名証明書。thumbprintは証明書の年次更新で変わるため、署名者はCNで照合する。
$signerCommonName = 'Open Source Developer Yuichiro Shinozaki'
if ($Sign) {
    . (Join-Path $PSScriptRoot 'Sigilune.Common.ps1')
    $sigilune = Resolve-SigiluneExe
    # ビルド前にログインし、認証の失敗を早く検出する（有効なセッションは再利用する）。
    Connect-Sigilune -SigiluneExe $sigilune
    # Velopackはファイルごとに署名テンプレートを実行する。1ファイルずつ直列に署名し、
    # Sigiluneが署名後に有効性・署名者・タイムスタンプを検証する。--replaceは旧signtoolと同じく
    # 既存の署名を置き換える（Sigiluneは既定で署名済みファイルを拒否するため）。
    $signing = @(
        '--signParallel', '1',
        '--signTemplate', "`"$sigilune`" sign --replace {{file}}"
    )
}

$repo = Split-Path -Parent $PSScriptRoot
$build = if ($CargoTargetDirectory) {
    if (![IO.Path]::IsPathFullyQualified($CargoTargetDirectory)) { throw 'Specify an absolute path for the Cargo target directory.' }
    $CargoTargetDirectory
} else {
    Join-Path $work.FullName 'target'
}
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
        if ($signature.Status -ne 'Valid' -or
            $signature.SignerCertificate.Subject -notlike "CN=$signerCommonName*" -or
            !$signature.TimeStamperCertificate) {
            throw 'Setup failed Authenticode verification (signer and timestamp).'
        }
    }
    Get-FileHash -LiteralPath $setup -Algorithm SHA256
} finally {
    $env:TEMP = $previousTemp
    $env:TMP = $previousTmp
    Pop-Location
}
