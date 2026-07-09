$port = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue
if ($port) {
    $p = Get-Process -Id $port.OwningProcess -ErrorAction SilentlyContinue
    Write-Output "port 1420 IN USE: PID $($port.OwningProcess) ($($p.ProcessName))"
} else {
    Write-Output "port 1420 free"
}

$exe = Get-Process -Name suvarix -ErrorAction SilentlyContinue
if ($exe) {
    Write-Output "suvarix.exe RUNNING: PID $($exe.Id)"
} else {
    Write-Output "suvarix.exe not running"
}
