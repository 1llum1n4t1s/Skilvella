# 梱包済みのWindowsインストーラー一式を Cloudflare R2 (skilvella.kagayoi.com) へ配信する。
#
# 入力は tools/pack-windows-installer.ps1 の出力 packages\（Setup・releases.win.json・full nupkg など）。
# アプリの自動更新は https://skilvella.kagayoi.com の releases.win.json と full nupkg を読むため、
# Setup だけでなくこれらも置く必要がある。
#
# 方式は同じ Velopack 1.2.161 でR2配信している Shisui / Lumin4ti / Edictora に合わせる
# （同梱の R2Distribution.Common.ps1 / R2Transfer.Core.cs は同一のコピー。Node.js・Wrangler 不要）。
# - 版付きの成果物（*.nupkg、版番号を含む名前）は「新規作成のみ」で配信し、既存オブジェクトを上書きしない。
#   同じ名前が既にあれば内容のハッシュを照合し、一致しなければ失敗する。
# - 固定名（SkilvellaApp-win-Setup.exe、releases.win.json、assets.win.json、RELEASES）は最新を指すため、
#   必ず置き換える。クライアントから未配信の nupkg が見える時間を作らないよう、manifest は最後に置く。
# - 段階（-Stage）: portable/ 側の publish-portable-r2.ps1 と交互に呼び、全体の順序を保つため3段階に分けられる。
#     Immutable = 版付きの不変物（nupkg・版付きSetup）を置き、公開URLから取り直してハッシュを照合する。
#     Fixed     = 固定名（Setup・assets.*.json・RELEASES）を置き換える。
#     Manifest  = releases.*.json を置き、固定名のキャッシュを purge する。
#   既定の All は手元からの一括配信（従来どおり、不変物 → 固定名 → manifest の順）。
# - 失敗は throw で知らせ、成功時は $LASTEXITCODE を 0 に戻す（GitHub Actions の pwsh は step 末尾で
#   $LASTEXITCODE を終了コードにするため、内部のネイティブ呼び出しの値が残ると成功後に誤って失敗扱いになる）。
# - 認証は環境変数 CLOUDFLARE_API_TOKEN（CIは Environment "release" の Secret）。無ければ
#   D:\Dev\Secret\secrets.json の cloudflare.api_token を使う。値はログへ出さない。
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $PackagesDirectory,
    [ValidateSet('All', 'Immutable', 'Fixed', 'Manifest')]
    [string] $Stage = 'All',
    # 配信せず、置く予定のファイルと分類だけを表示する。
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Bucket = 'skilvella-updates'
$BaseUrl = 'https://skilvella.kagayoi.com'
$ZoneName = 'kagayoi.com'
# 本人の他アプリと同じ Cloudflare アカウント（秘密情報ではない識別子）。
$AccountId = '10901bfadbf1005164774a7350082985'
$SecretsPath = 'D:\Dev\Secret\secrets.json'

if (![IO.Path]::IsPathFullyQualified($PackagesDirectory)) { throw 'Specify an absolute path to the packages directory.' }
$packagesRoot = Get-Item -LiteralPath $PackagesDirectory -Force
if (!$packagesRoot.PSIsContainer) { throw 'The packages path must be a directory.' }
$files = @(Get-ChildItem -LiteralPath $packagesRoot.FullName -File -Force)
foreach ($required in 'releases.win.json', 'SkilvellaApp-win-Setup.exe') {
    if ($files.Name -notcontains $required) { throw "Missing required file: $required" }
}
if (!($files | Where-Object { $_.Name -like 'SkilvellaApp-*-full.nupkg' })) { throw 'Missing the full nupkg.' }

# 固定名のメタデータ。manifest (releases.*.json) は最後に置く。
function Get-Order([IO.FileInfo]$File) {
    if ($File.Name -like 'releases.*.json') { return 3 }
    if ($File.Name -like 'RELEASES*') { return 2 }
    if ($File.Name -like 'assets.*.json') { return 1 }
    return 0
}
function Test-Immutable([IO.FileInfo]$File) {
    return $File.Extension -eq '.nupkg' -or $File.Name -match '\d+\.\d+\.\d+'
}
# 段階の振り分け。版付き（不変）→ 固定名 → manifest の順に置く。
function Get-StageName([IO.FileInfo]$File) {
    if (Test-Immutable $File) { return 'Immutable' }
    if ($File.Name -like 'releases.*.json') { return 'Manifest' }
    return 'Fixed'
}
$ordered = @($files | Sort-Object @{ Expression = { Get-Order $_ } }, Name | Where-Object { $Stage -eq 'All' -or (Get-StageName $_) -eq $Stage })
if ($DryRun) {
    foreach ($file in $ordered) {
        $kind = if (Test-Immutable $file) { 'immutable' } else { 'replace' }
        Write-Host ("{0,-10} {1,-10} {2}" -f $kind, (Get-StageName $file), $file.Name)
    }
} else {
    . (Join-Path $PSScriptRoot 'R2Distribution.Common.ps1')
    if (-not $env:CLOUDFLARE_API_TOKEN) {
        if (-not (Test-Path -LiteralPath $SecretsPath)) { throw 'CLOUDFLARE_API_TOKEN is not set and the local secrets file was not found.' }
        $secrets = Get-Content -LiteralPath $SecretsPath -Raw | ConvertFrom-Json
        if (-not $secrets.cloudflare.api_token) { throw 'secrets.json has no cloudflare.api_token.' }
        $env:CLOUDFLARE_API_TOKEN = $secrets.cloudflare.api_token
    }
    $headers = @{ Authorization = "Bearer $($env:CLOUDFLARE_API_TOKEN)" }

    # 配信前に、zone と bucket が存在することを確認して早く失敗する。
    # bucket / ドメインが未作成の場合はここで止まる（作成は本人作業）。
    $zone = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/zones?name=$ZoneName" -Headers $headers -TimeoutSec 30 -MaximumRedirection 0
    if (-not $zone.success -or @($zone.result).Count -eq 0) { throw "Cloudflare zone '$ZoneName' was not found." }
    $zoneId = $zone.result[0].id
    $bucketResponse = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/accounts/$AccountId/r2/buckets/$([Uri]::EscapeDataString($Bucket))" -Headers $headers -TimeoutSec 30 -MaximumRedirection 0
    if (-not $bucketResponse.success -or $bucketResponse.result.name -cne $Bucket) { throw "R2 bucket '$Bucket' was not found." }

    foreach ($file in $ordered) {
        $immutable = Test-Immutable $file
        $cacheControl = if ($immutable) { 'public, max-age=31536000, immutable' } else { 'no-cache, must-revalidate' }
        $contentType = switch ($file.Extension.ToLowerInvariant()) {
            '.json' { 'application/json'; break }
            '.exe' { 'application/vnd.microsoft.portable-executable'; break }
            default { if ($file.Name -like 'RELEASES*') { 'text/plain; charset=utf-8' } else { 'application/octet-stream' } }
        }
        Write-Host "  upload $($file.Name)"
        $null = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $file.FullName `
            -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType $contentType -CacheControl $cacheControl -Immutable:$immutable
    }

    # 版付きの不変成果物は、公開URL（Cloudflare 経由）から取り直してハッシュを照合する。公開ドメインの
    # 未設定・誤設定のまま固定名・manifest だけを進めないための確認（nupkg を置いてから manifest を置く前提の裏付け）。
    $work = Join-Path ([IO.Path]::GetTempPath()) ('skilvella-installer-verify-' + [guid]::NewGuid().ToString('N'))
    try {
        foreach ($file in @($ordered | Where-Object { Test-Immutable $_ })) {
            New-Item -ItemType Directory -Path $work -Force | Out-Null
            $public = Join-Path $work $file.Name
            Invoke-WebRequest -Uri "$BaseUrl/$([Uri]::EscapeDataString($file.Name))" -OutFile $public -MaximumRedirection 0 -TimeoutSec 600
            if ((Get-FileHash -LiteralPath $public -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash) {
                throw "$BaseUrl/$($file.Name) does not serve the uploaded file."
            }
            Remove-Item -LiteralPath $public -Force
            Write-Host "  public URL verified: $($file.Name)"
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    # 固定名のURLだけキャッシュを破棄する（版付きの不変成果物は対象外）。manifest の段階（または一括）で行う。
    $purgeUrls = @()
    if ($Stage -in 'All', 'Manifest') {
        $purgeUrls = @($files | Where-Object { -not (Test-Immutable $_) } | ForEach-Object { "$BaseUrl/$([Uri]::EscapeDataString($_.Name))" })
    }
    if ($purgeUrls.Count -gt 0) {
        $purge = Invoke-RestMethod -Method Post -Uri "https://api.cloudflare.com/client/v4/zones/$zoneId/purge_cache" -Headers $headers `
            -ContentType 'application/json' -Body (@{ files = $purgeUrls } | ConvertTo-Json -Compress) -TimeoutSec 30 -MaximumRedirection 0
        if (-not $purge.success) { throw 'Cloudflare cache purge failed.' }
        Write-Host "  purge requested: $($purgeUrls.Count) URLs"
    }
    Write-Host "Published (stage '$Stage') to $BaseUrl"
}
# 失敗は上の throw で知らせた。内部のネイティブ呼び出しの終了コードを GitHub Actions に誤って拾わせない。
$global:LASTEXITCODE = 0
