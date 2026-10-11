# 打包免安装版：先运行 pnpm tauri build，再执行本脚本
# 产物：src-tauri/target/release/bundle/portable/VenvDock_<版本>_x64-portable.zip
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$exe = "$root\src-tauri\target\release\venvdock.exe"
if (-not (Test-Path $exe)) { throw "找不到 $exe，请先运行 pnpm tauri build" }

$out = "$root\src-tauri\target\release\bundle\portable"
$stage = "$out\VenvDock"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$stage\data" | Out-Null
Copy-Item $exe "$stage\VenvDock.exe"
# data 目录存在即为便携模式；放一个说明文件，避免压缩时空目录被丢弃
Set-Content "$stage\data\说明.txt" -Encoding utf8 -Value @"
此文件夹存在时，VenvDock 以便携模式运行：配置、日志和界面缓存都保存在这里。
删除此文件夹则改用 %APPDATA%\com.venvdock.app。
"@
$zip = "$out\VenvDock_${version}_x64-portable.zip"
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item -Recurse -Force $stage
"已生成 $zip"
