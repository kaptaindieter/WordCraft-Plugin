param(
    [Parameter(Mandatory=$true)]
    [string]$TargetDir,
    [switch]$Build
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ArgsList = @((Join-Path $ScriptDir "install_plugin.py"), $TargetDir)
if ($Build) {
    $ArgsList += "--build"
}
python @ArgsList
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
