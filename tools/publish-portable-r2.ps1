#requires -Version 7.2
<#
.SYNOPSIS
ZIP版・macOS版（ポータブル版）の配布物を Cloudflare R2（skilvella-updates / https://skilvella.kagayoi.com）の
portable/ 配下へ配信する。

.DESCRIPTION
release.yml（版タグ v<版> の push）の publish ジョブから、Windows インストーラー側の
publish-windows-installer-r2.ps1 と段階を交互に呼ぶ。GitHub Release は使わない。

置き場（バケット直下の portable/）:
  portable/<版>/Skilvella-windows-x86_64.zip      版付き・不変（immutable）
  portable/<版>/Skilvella-macos-aarch64.tar.gz    版付き・不変（immutable）
  portable/<版>/SHA256SUMS.txt                    版付き・不変（immutable）。アプリ内更新が使う
  portable/Skilvella-windows-x86_64.zip 等の3つ   固定名。README の導線用で、置き換え・no-cache。アプリは使わない
  portable/latest.json                            {"schema":1,"version":"X.Y.Z"}。最後に置く・no-cache

段階（-Stage。既定 All は手元からの一括配信用）:
  Immutable  入力検査と bucket・zone の確認 → 版付きの3ファイルを置く → 公開 URL から取り直してハッシュ照合
  Fixed      固定名の3ファイルを置き換える
  Latest     latest.json を置く → 固定名だけキャッシュを purge する
安全策:
  - 入力検査（SHA256SUMS.txt の2行が実ファイルのハッシュと一致するか）と権限確認を、何も書く前に行う。
  - 版付きは「新規作成のみ」。同じキーが既にあって内容が同一なら成功（再実行を許す）、内容が違えば上書きせず失敗する
    （署名のタイムスタンプなどで同じ版の別内容を上げない）。
  - 公開中の latest.json の版が今回より新しければ、固定名と latest.json は更新しない（後退防止。版付きは置く）。
  - 失敗は throw で知らせる。成功時は $LASTEXITCODE を 0 に戻す（GitHub Actions の pwsh は step 末尾で
    $LASTEXITCODE を終了コードにするため、内部のネイティブ呼び出しの値が残ると成功後に誤って失敗扱いになる）。

認証: 環境変数 CLOUDFLARE_API_TOKEN（R2 書込み＋キャッシュ purge 権限。CI では Environment の Secret）。
転送実装は同梱の R2Transfer.Core.cs / R2Distribution.Common.ps1（Sigilune・Edictora と同一の共通コピー）。
#>
[CmdletBinding()]
param(
    # 配信する版（例 1.1.9）。Cargo.toml の版と一致していること（呼び出し側で保証する）。
    [Parameter(Mandatory)][string]$Version,
    # 2つのアーカイブと SHA256SUMS.txt を置いた絶対パスのフォルダー（`skilvella-release checksums` の出力）。
    [Parameter(Mandatory)][string]$DistributionDirectory,
    [ValidateSet('All', 'Immutable', 'Fixed', 'Latest')][string]$Stage = 'All',
    [string]$Bucket = 'skilvella-updates',
    [string]$BaseUrl = 'https://skilvella.kagayoi.com',
    # Cloudflare アカウント ID（秘密ではない。他アプリと同じアカウント）。
    [string]$AccountId = '10901bfadbf1005164774a7350082985',
    # 入力検査と置き先・latest.json の生成だけを行い、ネットワークへは一切接続しない（ローカル検証用）。
    [switch]$ValidateOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "version must look like 1.2.3: $Version" }
$BaseUrl = $BaseUrl.TrimEnd('/')
if ($BaseUrl -notmatch '^https://') { throw "BaseUrl must be https: $BaseUrl" }
if (![IO.Path]::IsPathFullyQualified($DistributionDirectory)) { throw 'Specify an absolute path to the distribution directory.' }

# --- 入力検査 ------------------------------------------------------------------------------------
$assets = @('Skilvella-windows-x86_64.zip', 'Skilvella-macos-aarch64.tar.gz')
$sumsName = 'SHA256SUMS.txt'
$directory = Get-Item -LiteralPath $DistributionDirectory -ErrorAction Stop
if (!$directory.PSIsContainer) { throw 'The distribution path must be a directory.' }
$expectedNames = @($assets + $sumsName | Sort-Object)
$actualNames = @(Get-ChildItem -LiteralPath $directory.FullName -File -Force | ForEach-Object Name | Sort-Object)
if (Compare-Object $expectedNames $actualNames) { throw "unexpected distribution contents: $($actualNames -join ', ')" }

# SHA256SUMS.txt はアプリ内更新の checksum() と同じく、64桁のハッシュ、空白2つまたは「空白+*」、ファイル名。
# 行の並びは作らない。$assets の順で期待値を連結すると、skilvella-release checksums の出力順とずれて
# 1.1.9 の初回配信が失敗した。ファイルに書いてある名前のハッシュを、その実ファイルと照合する。
$sumsText = [IO.File]::ReadAllText((Join-Path $directory.FullName $sumsName), [Text.UTF8Encoding]::new($false, $true))
$hashes = @{}
$checksumPattern = '^(?<hash>[0-9A-Fa-f]{64})(?<gap>  | \*)(?<name>[^ \t]+)$'
foreach ($line in @($sumsText.Replace("`r`n", "`n").Trim() -split "`n")) {
    if ($line -notmatch $checksumPattern) { throw "Malformed checksum line in ${sumsName}: $line" }
    $name = $Matches.name
    $hash = $Matches.hash.ToLowerInvariant()
    if ($hashes.ContainsKey($name)) { throw "Duplicate checksum for $name" }
    if ($assets -notcontains $name) { throw "$sumsName names a file that is not a release archive: $name" }
    $actual = (Get-FileHash -LiteralPath (Join-Path $directory.FullName $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -cne $actual) { throw "$sumsName does not match $name" }
    $hashes[$name] = $hash
}
foreach ($asset in $assets) {
    if (-not $hashes.ContainsKey($asset)) { throw "$sumsName is missing $asset" }
}

$contentTypes = @{
    'Skilvella-windows-x86_64.zip'   = 'application/zip'
    'Skilvella-macos-aarch64.tar.gz' = 'application/gzip'
    'SHA256SUMS.txt'                 = 'text/plain; charset=utf-8'
}
$immutableCache = 'public, max-age=31536000, immutable'
$fixedCache = 'no-cache, must-revalidate'
$names = @($assets + $sumsName)
# 版付き（不変）と固定名のキー。固定名は README の導線用で、アプリ内更新は使わない。
$immutableItems = @($names | ForEach-Object { @{ Name = $_; Key = "portable/$Version/$_"; File = (Join-Path $directory.FullName $_) } })
$fixedItems = @($names | ForEach-Object { @{ Name = $_; Key = "portable/$_"; File = (Join-Path $directory.FullName $_) } })
$latestKey = 'portable/latest.json'
$latestJson = ([ordered]@{ schema = 1; version = $Version } | ConvertTo-Json -Compress) + "`n"

$runImmutable = $Stage -in 'All', 'Immutable'
$runFixed = $Stage -in 'All', 'Fixed'
$runLatest = $Stage -in 'All', 'Latest'

if ($ValidateOnly) {
    Write-Host "package ok: version $Version"
    foreach ($name in $names) { Write-Host ("  {0}  sha256={1}" -f $name, $(if ($hashes.ContainsKey($name)) { $hashes[$name] } else { '(list)' })) }
    if ($runImmutable) { foreach ($item in $immutableItems) { Write-Host ("  immutable {0}" -f $item.Key) } }
    if ($runFixed) { foreach ($item in $fixedItems) { Write-Host ("  replace   {0}" -f $item.Key) } }
    if ($runLatest) { Write-Host ("  replace   {0}  {1}" -f $latestKey, $latestJson.Trim()) }
    Write-Host 'ValidateOnly: no network access.'
} else {
    if (-not $env:CLOUDFLARE_API_TOKEN) { throw 'CLOUDFLARE_API_TOKEN is not set' }
    . (Join-Path $PSScriptRoot 'R2Distribution.Common.ps1')
    $api = 'https://api.cloudflare.com/client/v4'
    $headers = @{ Authorization = "Bearer $($env:CLOUDFLARE_API_TOKEN)" }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('skilvella-portable-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        # --- 書く前の権限確認（途中まで公開された状態を防ぐ） -------------------------------------
        $bucketResponse = Invoke-RestMethod -Uri "$api/accounts/$AccountId/r2/buckets/$([uri]::EscapeDataString($Bucket))" `
            -Headers $headers -TimeoutSec 30 -MaximumRedirection 0
        if (-not $bucketResponse.success -or $bucketResponse.result.name -cne $Bucket) { throw "R2 bucket '$Bucket' was not found" }
        # <sub>.kagayoi.com -> kagayoi.com（apex）。キャッシュ purge に zone ID が要る。
        $zoneName = ([uri]$BaseUrl).Host -replace '^[^.]+\.', ''
        $zoneResponse = Invoke-RestMethod -Uri "$api/zones?name=$zoneName" -Headers $headers -TimeoutSec 30 -MaximumRedirection 0
        if (-not $zoneResponse.success -or @($zoneResponse.result).Count -eq 0) { throw "Cloudflare zone '$zoneName' was not found" }
        $zoneId = $zoneResponse.result[0].id

        # --- 後退防止の判定材料: いま公開されている latest.json の版 --------------------------------
        # 固定名は no-cache で配っているため、公開 URL から読む。無い（404）なら初回配信。
        $remote = Invoke-WebRequest -Uri "$BaseUrl/$latestKey" -SkipHttpErrorCheck -MaximumRedirection 0 -TimeoutSec 30
        $remoteVersion = $null
        if ($remote.StatusCode -eq 200) {
            $remoteFeed = $remote.Content | ConvertFrom-Json
            # 壊れたフィードを黙って上書きしない（原因を確かめてから）。
            if (-not ($remoteFeed.PSObject.Properties.Name -contains 'version') -or "$($remoteFeed.version)" -notmatch '^\d+\.\d+\.\d+$') {
                throw "the published latest.json has no valid version; check $BaseUrl/$latestKey"
            }
            $remoteVersion = [version]$remoteFeed.version
        } elseif ($remote.StatusCode -ne 404) {
            throw "could not read $BaseUrl/$latestKey (HTTP $($remote.StatusCode))"
        }
        $stale = $null -ne $remoteVersion -and $remoteVersion -gt [version]$Version

        if ($runImmutable) {
            # Immutable: 既存と同一なら成功（Changed=false）、既存と内容が違えば照合で失敗し上書きしない。
            foreach ($item in $immutableItems) {
                $result = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $item.File -Key $item.Key `
                    -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType $contentTypes[$item.Name] -CacheControl $immutableCache -Immutable
                Write-Host ("  {0}  {1}" -f $(if ($result.Changed) { 'uploaded ' } else { 'unchanged' }), $item.Key)
            }
            # 公開 URL（Cloudflare 経由）から取り直して照合する。公開ドメイン未設定・誤設定のまま
            # 固定名やフィードだけを進めないための確認。
            foreach ($item in $immutableItems) {
                $public = Join-Path $work $item.Name
                Invoke-WebRequest -Uri "$BaseUrl/$($item.Key)" -OutFile $public -MaximumRedirection 0 -TimeoutSec 600
                $publicHash = (Get-FileHash -LiteralPath $public -Algorithm SHA256).Hash
                $localHash = (Get-FileHash -LiteralPath $item.File -Algorithm SHA256).Hash
                if ($publicHash -cne $localHash) { throw "$BaseUrl/$($item.Key) does not serve the uploaded file" }
                Remove-Item -LiteralPath $public -Force
            }
            Write-Host "public URL verified: $BaseUrl/portable/$Version/"
        }

        if ($runFixed -or $runLatest) {
            if ($stale) {
                # 古い版の再実行などで公開中の最新を後退させない。版付きファイルは置いたので配信自体は成功。
                Write-Warning "published latest.json is already $remoteVersion (newer than $Version); fixed names and latest.json were left unchanged."
            } else {
                if ($runFixed) {
                    foreach ($item in $fixedItems) {
                        $null = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $item.File -Key $item.Key `
                            -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType $contentTypes[$item.Name] -CacheControl $fixedCache
                        Write-Host "  replaced   $($item.Key)"
                    }
                }
                if ($runLatest) {
                    # latest.json は最後。クライアントから、まだ置いていない版が見える時間を作らない。
                    $latestFile = Join-Path $work 'latest.json'
                    [IO.File]::WriteAllText($latestFile, $latestJson, [Text.UTF8Encoding]::new($false))
                    $null = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $latestFile -Key $latestKey `
                        -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType 'application/json' -CacheControl $fixedCache
                    Write-Host "  replaced   $latestKey"
                    # --- 固定名だけ purge（版付きは URL が一意なので不要）。失敗は警告にとどめる ---------
                    try {
                        $urls = @(($fixedItems | ForEach-Object { "$BaseUrl/$($_.Key)" }) + "$BaseUrl/$latestKey")
                        $body = @{ files = $urls } | ConvertTo-Json -Compress
                        $purge = Invoke-RestMethod -Method Post -Uri "$api/zones/$zoneId/purge_cache" -Headers $headers `
                            -ContentType 'application/json' -Body $body -TimeoutSec 30 -MaximumRedirection 0
                        if (-not $purge.success) { throw ($purge.errors | ConvertTo-Json -Compress) }
                        Write-Host "  purged     $($urls.Count) URLs"
                    } catch {
                        Write-Warning "uploaded, but the cache purge failed: $($_.Exception.Message)"
                    }
                }
            }
        }
        Write-Host "portable $Version stage '$Stage' done: $BaseUrl/portable/"
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}
# 失敗は上の throw で知らせた。内部のネイティブ呼び出しの終了コードを GitHub Actions に誤って拾わせない。
$global:LASTEXITCODE = 0
