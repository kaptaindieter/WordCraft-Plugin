param(
    [Parameter(Mandatory=$true)]
    [string]$TargetDir
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
python (Join-Path $ScriptDir "uninstall_plugin.py") $TargetDir
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
