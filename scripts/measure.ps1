param(
    [ValidateRange(1, 3600)]
    [int]$Seconds = 60,
    [switch]$Tray,
    [string]$Executable = (Join-Path $PSScriptRoot "..\target\release\peakcore.exe")
)

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path
$argument = if ($Tray) { "--tray" } else { "--measure-idle" }
$process = Start-Process -FilePath $resolvedExecutable -ArgumentList $argument -PassThru
try {
    $start = Get-Process -Id $process.Id -ErrorAction Stop
    $initialCpu = $start.TotalProcessorTime.TotalSeconds
    $startTime = [DateTime]::UtcNow
    Start-Sleep -Seconds $Seconds
    $process.Refresh()
    if ($process.HasExited) {
        throw "PeakCore exited during the measurement (exit code $($process.ExitCode))."
    }
    $sample = Get-Process -Id $process.Id -ErrorAction Stop
    $elapsed = ([DateTime]::UtcNow - $startTime).TotalSeconds
    $cpuSeconds = $sample.TotalProcessorTime.TotalSeconds - $initialCpu
    $cpuPercent = if ($elapsed -gt 0) { 100 * $cpuSeconds / $elapsed } else { 0 }

    [pscustomobject]@{
        ProcessId       = $sample.Id
        ElapsedSeconds  = [math]::Round($elapsed, 1)
        WorkingSetMB    = [math]::Round($sample.WorkingSet64 / 1MB, 2)
        PrivateBytesMB  = [math]::Round($sample.PrivateMemorySize64 / 1MB, 2)
        CpuPercentOneCore = [math]::Round($cpuPercent, 3)
    } | Format-List
}
finally {
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}
