# 生成 OTP code 并发布 8 个 hermes 0.0.0 placeholder
# 从 vmz-framework .env.placeholder.local 加载 TOTP secret

$ErrorActionPreference = 'Stop'

# 1. 加载 TOTP secret
$envFile = 'e:\vmz 全栈开发\vmz-framework\.env.placeholder.local'
$content = Get-Content $envFile -Raw
if ($content -match 'NPM_TOTP_SECRET=([A-Z2-7]+)') {
    $env:NPM_TOTP_SECRET = $Matches[1]
} else {
    throw 'NPM_TOTP_SECRET not found'
}

# 2. 用 PowerShell 生成 TOTP (Base32 解码 → HMAC-SHA1 → 截断 → 6 digits)
function Get-TOTP {
    param([string]$Secret, [int]$Digits = 6)
    $epoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $counter = [Math]::Floor($epoch / 30)
    
    # Base32 decode
    $Secret = $Secret.ToUpper()
    $base32Chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'
    $bits = ''
    foreach ($c in $Secret.ToCharArray()) {
        $idx = $base32Chars.IndexOf($c)
        if ($idx -ge 0) { $bits += [Convert]::ToString($idx, 2).PadLeft(5, '0') }
    }
    while ($bits.Length % 8 -ne 0) { $bits = $bits.Substring(0, $bits.Length - 1) }
    $bytes = for ($i = 0; $i -lt $bits.Length; $i += 8) { [Convert]::ToByte($bits.Substring($i, 8), 2) }
    
    # Counter as big-endian 8 bytes
    $counterBytes = [BitConverter]::GetBytes([long]$counter)
    if ([BitConverter]::IsLittleEndian) { [array]::Reverse($counterBytes) }
    
    # HMAC-SHA1
    $hmac = New-Object System.Security.Cryptography.HMACSHA1($bytes)
    $hash = $hmac.ComputeHash($counterBytes)
    $offset = $hash[-1] -band 0x0F
    $binary = (($hash[$offset] -band 0x7F) -shl 24) -bor
              (($hash[$offset + 1] -band 0xFF) -shl 16) -bor
              (($hash[$offset + 2] -band 0xFF) -shl 8) -bor
              ($hash[$offset + 3] -band 0xFF)
    $otp = $binary % [Math]::Pow(10, $Digits)
    return $otp.ToString().PadLeft($Digits, '0')
}

$otp = Get-TOTP $env:NPM_TOTP_SECRET
Write-Output "OTP code: $otp (valid ~30s)"

# 3. 发布 8 个 stub
$packages = @(
    @{ dir = 'packages/hermes-skills' },
    @{ dir = 'projects/packages/hermes' },
    @{ dir = 'projects/packages/hermes-win32-x64' },
    @{ dir = 'projects/packages/hermes-linux-x64' },
    @{ dir = 'projects/packages/hermes-linux-arm64' },
    @{ dir = 'projects/packages/hermes-darwin-x64' },
    @{ dir = 'projects/packages/hermes-darwin-arm64' },
    @{ dir = 'projects/packages/hermes-unknown-wasm32' }
)

$root = 'e:\vmz 全栈开发\hermes-rpc'
$success = 0; $skip = 0; $fail = 0

foreach ($p in $packages) {
    $pkgDir = Join-Path $root $p.dir
    $pkgJsonPath = Join-Path $pkgDir 'package.json'
    $pkg = Get-Content $pkgJsonPath -Raw | ConvertFrom-Json
    
    Write-Output "`n--- $($pkg.name) ---"
    
    # 改成 0.0.0 stub
    if ($pkg.version -ne '0.0.0') {
        $pkg.version = '0.0.0'
        $pkg | ConvertTo-Json -Depth 10 | Set-Content $pkgJsonPath
    }
    
    # 检查是否已存在
    $check = npm view "$($pkg.name)@0.0.0" version 2>&1
    if ($LASTEXITCODE -eq 0 -and $check -match '0\.0\.0') {
        Write-Output "  already published — skip"
        $skip++; continue
    }
    
    Set-Location $pkgDir
    npm publish --access public --ignore-scripts --otp $otp 2>&1 | ForEach-Object { Write-Output "  $_" }
    if ($LASTEXITCODE -eq 0) {
        Write-Output "  ✓ SUCCESS"
        $success++
    } else {
        Write-Output "  ✗ FAILED (exit=$LASTEXITCODE)"
        $fail++
    }
}

Write-Output "`n=== Summary: success=$success skip=$skip fail=$fail ==="
