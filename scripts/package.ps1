# 收集打包产物：由 pnpm package 在 tauri build 之后调用
# 产物统一放到项目根目录 release/ 下：安装版 exe + 免安装版单文件 exe 及各自的更新签名 .sig，旧版本保留
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$exe = "$root\src-tauri\target\release\venvdock.exe"
$setup = "$root\src-tauri\target\release\bundle\nsis\VenvDock_${version}_x64-setup.exe"
foreach ($f in $exe, $setup) {
  if (-not (Test-Path $f)) { throw "找不到 $f，请先运行 pnpm tauri build" }
}

$out = "$root\release"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $setup $out -Force

# 免安装版：前端资源已编译进 exe，单文件即可运行；数据默认放在 %APPDATA%\com.venvdock.app
$portable = "$out\VenvDock_${version}_x64-portable.exe"
Copy-Item $exe $portable -Force

# 更新签名：程序下载新版本后用内置公钥校验 .sig（见 src-tauri/src/updater.rs）。
# 私钥放在与项目同级的 ProjectKey\VenvDock\venvdock.key（不在仓库里）；CI 中由 secret 提供环境变量
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
  $key = [IO.Path]::GetFullPath("$root\..\ProjectKey\VenvDock\venvdock.key")
  if (-not (Test-Path $key)) { throw "找不到更新签名私钥 $key，见 README 中的「发布新版本」" }
  $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $key -Raw).Trim()
}
$password = if ($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD } else { "" }
$setupOut = "$out\VenvDock_${version}_x64-setup.exe"
Push-Location $root
try {
  foreach ($f in $setupOut, $portable) {
    # 用 --password=… 单个参数：空密码时 -p "" 的空参数在 CI 上会被丢掉
    pnpm tauri signer sign "--password=$password" $f | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "签名失败：$f" }
    if (-not (Test-Path "$f.sig")) { throw "找不到 $f.sig" }
  }
} finally {
  Pop-Location
}
"已生成："
"  $setupOut(.sig)"
"  $portable(.sig)"
