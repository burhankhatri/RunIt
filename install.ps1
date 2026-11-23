# Install or update RunIt on Windows:
#   irm https://raw.githubusercontent.com/burhankhatri/RunIt/main/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
& {
    $repo = if ($env:RUNIT_REPO) { $env:RUNIT_REPO } else { 'burhankhatri/RunIt' }
    if (-not [Environment]::Is64BitOperatingSystem) { throw 'RunIt needs 64-bit Windows 10 or 11.' }
    $file = 'RunIt-windows-x64.zip'
    $base = if ($env:RUNIT_BASE) { $env:RUNIT_BASE } else { "https://github.com/$repo/releases/latest/download" }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("runit-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $tmp | Out-Null
    try {
        Write-Host 'Downloading RunIt'
        $zip = Join-Path $tmp $file
        Invoke-WebRequest -UseBasicParsing "$base/$file" -OutFile $zip
        $sums = Join-Path $tmp 'SHA256SUMS'
        Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile $sums
        $line = Get-Content $sums | Where-Object { $_ -match [regex]::Escape($file) + '$' } | Select-Object -First 1
        if (-not $line) { throw 'The checksum list does not mention this file.' }
        if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne ($line -split '\s+')[0]) { throw 'The download does not match its checksum.' }

        $dir = Join-Path $env:LOCALAPPDATA 'Programs\RunIt'
        Get-Process RunIt -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($dir, 'OrdinalIgnoreCase') } | ForEach-Object { $_.CloseMainWindow() | Out-Null; if (-not $_.WaitForExit(5000)) { $_.Kill(); $_.WaitForExit() } }
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
        New-Item -ItemType Directory $dir -Force | Out-Null
        Expand-Archive $zip $dir -Force
        $exe = Join-Path $dir 'RunIt.exe'
        if (-not (Test-Path $exe)) { throw 'The download is missing RunIt.exe.' }

        $link = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\RunIt.lnk'))
        $link.TargetPath = $exe
        $link.WorkingDirectory = $dir
        $link.Save()

        $un = Join-Path $dir 'uninstall.ps1'
        @'
Get-Process RunIt -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$PSScriptRoot*" } | Stop-Process -Force
Remove-Item (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\RunIt.lnk') -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RunIt' -Recurse -ErrorAction SilentlyContinue
Set-Location $env:TEMP
Start-Process cmd -WindowStyle Hidden -ArgumentList "/c timeout /t 2 >nul & rmdir /s /q `"$PSScriptRoot`""
'@ | Set-Content $un -Encoding UTF8
        $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RunIt'
        New-Item $key -Force | Out-Null
        Set-ItemProperty $key DisplayName 'RunIt'
        Set-ItemProperty $key DisplayIcon $exe
        Set-ItemProperty $key Publisher 'Burhanuddin Khatri'
        Set-ItemProperty $key InstallLocation $dir
        Set-ItemProperty $key UninstallString "powershell -NoProfile -ExecutionPolicy Bypass -File `"$un`""
        Write-Host "Installed $dir"
        if (-not $env:RUNIT_NO_OPEN) { Start-Process $exe }
    } finally {
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
