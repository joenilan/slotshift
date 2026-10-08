# Slotshift's local, opt-in UI Automation driver. No network listener or credential access.
# Run from PowerShell: .\scripts\drive-slotshift.ps1 -Action smoke
[CmdletBinding()]
param(
    [ValidateSet('smoke','start-demo','inspect','click','set','wait','screenshot','diagnose')]
    [string]$Action='smoke',
    [int]$ProcessId=0,
    [string]$Control='',
    [string]$Value='',
    [int]$Index=0,
    [ValidateRange(1,60)][int]$TimeoutSeconds=10,
    [switch]$Absent,
    [string]$Output='',
    [switch]$AllowLive,
    [switch]$AllowSensitiveActions
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
$repo=Split-Path -Parent $PSScriptRoot
$binary=@(
    (Join-Path $repo 'target\debug\slotshift.exe'),
    (Join-Path $repo 'target\release\slotshift.exe'),
    (Join-Path $PSScriptRoot 'slotshift.exe'),
    (Join-Path $repo 'slotshift.exe')
) | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf} | Select-Object -First 1
if(-not $binary){throw 'Slotshift executable not found. Build it or place this driver beside slotshift.exe.'}
$desktop=[System.Windows.Automation.AutomationElement]::RootElement
$scope=[System.Windows.Automation.TreeScope]::Descendants
$noCondition=[System.Windows.Automation.Condition]::TrueCondition
function Get-Window([int]$id) {
    $condition=New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,$id)
    for($n=0;$n -lt 70;$n++){
        $window=$desktop.FindFirst([System.Windows.Automation.TreeScope]::Children,$condition)
        if($window){return $window}
        Start-Sleep -Milliseconds 120
    }
    throw "No Slotshift window found for PID $id."
}
function Get-Target([int]$id) {
    if($id -eq 0) {
        $candidates=@(Get-CimInstance Win32_Process -Filter "Name='slotshift.exe'" |
            Where-Object {$_.CommandLine -match '--demo'})
        if($candidates.Count -ne 1){
            throw "Specify -ProcessId. Exactly one demo process is needed for automatic detection; found $($candidates.Count)."
        }
        $id=[int]$candidates[0].ProcessId
    }
    $proc=Get-CimInstance Win32_Process -Filter "ProcessId=$id"
    if(!$proc -or $proc.Name -ne 'slotshift.exe'){throw "PID $id is not Slotshift."}
    $isDemo=[bool]($proc.CommandLine -match '--demo')
    if(!$isDemo -and !$AllowLive){throw 'Live app access is disabled. Pass -AllowLive explicitly, or drive a --demo instance.'}
    [pscustomobject]@{Process=$proc; Demo=$isDemo; Window=(Get-Window $id)}
}
function Redact([string]$text) {
    if(!$text){return ''}
    return [regex]::Replace($text,'(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}','***@***')
}
function Elements($window) {
    $window.FindAll($scope,$noCondition)
}
function Find-Control($window,[string]$name,[string]$type='ControlType.Button',[int]$index=0) {
    $matching=@(Elements $window | Where-Object {
        $_.Current.Name -eq $name -and $_.Current.ControlType.ProgrammaticName -eq $type
    })
    if($matching.Count -eq 0){throw "UI control not found: $(Redact $name) [$type]"}
    $selected=if($index -lt 0){$matching.Count+$index}else{$index}
    if($selected -lt 0 -or $selected -ge $matching.Count){throw 'Requested control index is unavailable.'}
    return $matching[$selected]
}
function Click-Control($window,[string]$name,[int]$index=0) {
    $item=Find-Control $window $name 'ControlType.Button' $index
    if(!$item.Current.IsEnabled){throw "UI control is disabled: $(Redact $name)"}
    $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Start-Sleep -Milliseconds 160
}
function Set-Control($window,[string]$name,[string]$value,[int]$index=0) {
    $item=Find-Control $window $name 'ControlType.Edit' $index
    $item.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($value)
    Start-Sleep -Milliseconds 180
}
function Has-Control($window,[string]$name,[string]$type='ControlType.Button') {
    @((Elements $window | Where-Object {
        $_.Current.Name -eq $name -and $_.Current.ControlType.ProgrammaticName -eq $type
    })).Count -gt 0
}
function Screenshot($window,[string]$path) {
    if([string]::IsNullOrWhiteSpace($path)){throw 'A screenshot requires -Output <absolute .png path>.'}
    if(-not[IO.Path]::IsPathRooted($path) -or [IO.Path]::GetExtension($path) -ne '.png'){
        throw 'Screenshot output must be an absolute PNG filename.'
    }
    $signature=@'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int width, int height, uint flags);
'@
    if(-not('SlotshiftUI.Win32' -as [type])){
        Add-Type -Namespace SlotshiftUI -Name Win32 -MemberDefinition $signature
    }
    $handle=[IntPtr]$window.Current.NativeWindowHandle
    [void][SlotshiftUI.Win32]::ShowWindow($handle,9)
    [void][SlotshiftUI.Win32]::SetWindowPos($handle,[IntPtr](-1),0,0,0,0,[uint32]0x13)
    try {
        [void][SlotshiftUI.Win32]::SetForegroundWindow($handle)
        Start-Sleep -Milliseconds 450
        $rect=$window.Current.BoundingRectangle
        if($rect.Width -lt 100 -or $rect.Height -lt 100){throw 'Window is minimized or has invalid bounds.'}
        $bitmap=New-Object Drawing.Bitmap ([int]$rect.Width),([int]$rect.Height)
        try {
            $graphics=[Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.CopyFromScreen([int]$rect.Left,[int]$rect.Top,0,0,$bitmap.Size)
                $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
            } finally {$graphics.Dispose()}
        } finally {$bitmap.Dispose()}
    } finally {
        [void][SlotshiftUI.Win32]::SetWindowPos($handle,[IntPtr](-2),0,0,0,0,[uint32]0x13)
    }
    Write-Output "Screenshot saved: $path"
}
function Start-Demo {
    $process=Start-Process -FilePath $binary -ArgumentList @('--demo') -PassThru
    try {
        $window=Get-Window $process.Id
        for($n=0;$n -lt 60;$n++){
            if((Has-Control $window 'Add account') -and (Has-Control $window 'Rename Workbench')){
                return [pscustomobject]@{Process=$process;Window=$window}
            }
            Start-Sleep -Milliseconds 150
        }
        throw 'Demo window opened but account controls did not become ready.'
    } catch {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw
    }
}
function Smoke {
    $instance=Start-Demo
    $w=$instance.Window
    try {
        if(-not(Has-Control $w 'Rename Workbench')){throw 'Sidebar rename button missing.'}
        Click-Control $w 'Add account'
        Set-Control $w 'e.g. Personal, Work, Side projects' 'QA Temporary'
        Click-Control $w 'Add account' -index -1
        if(-not(Has-Control $w 'Select QA Temporary')){throw 'New account did not appear.'}
        Click-Control $w 'Rename QA Temporary'
        Set-Control $w 'e.g. Personal, Work, Side projects' 'QA Renamed'
        Click-Control $w 'Save name'
        if(-not(Has-Control $w 'Select QA Renamed')){throw 'Newly added account rename did not persist.'}
        Click-Control $w 'Rename Workbench'
        Set-Control $w 'e.g. Personal, Work, Side projects' 'Workbench Targeted'
        Click-Control $w 'Save name'
        if(-not(Has-Control $w 'Select Workbench Targeted')){throw 'Non-selected account rename failed.'}
        if(-not(Has-Control $w 'Select QA Renamed')){throw 'Renaming another account unexpectedly removed the selection.'}
        Click-Control $w 'Remove QA Renamed'
        if(-not(Has-Control $w 'Cancel')){throw 'Removal confirmation dialog missing.'}
        Click-Control $w 'Cancel'
        if(-not(Has-Control $w 'Select QA Renamed')){throw 'Cancel unexpectedly removed account.'}
        if($Output){Screenshot $w $Output}
        Write-Output 'PASS: add account; rename added account; rename non-selected account; remove confirmation cancel.'
        Write-Output 'PASS: no real Codex accounts used and no model requests sent.'
    } finally {
        if(Get-Process -Id $instance.Process.Id -ErrorAction SilentlyContinue){
            Stop-Process -Id $instance.Process.Id -Force -ErrorAction SilentlyContinue
        }
    }
}
if($Action -eq 'smoke'){Smoke;return}
if($Action -eq 'start-demo'){
    $instance=Start-Demo
    Write-Output "Demo PID: $($instance.Process.Id)"
    return
}
$target=Get-Target $ProcessId
if($Action -eq 'inspect' -or $Action -eq 'diagnose'){
    $controls=@(Elements $target.Window | Where-Object {
        $_.Current.ControlType.ProgrammaticName -in @('ControlType.Button','ControlType.Edit')
    } | Select-Object -First 200 | ForEach-Object {
        [pscustomobject]@{
            Type=$_.Current.ControlType.ProgrammaticName
            Name=(Redact $_.Current.Name)
            Enabled=$_.Current.IsEnabled
        }
    })
    if($Action -eq 'diagnose'){
        $p=Get-Process -Id $target.Process.ProcessId
        [pscustomobject]@{
            ProcessId=$p.Id;IsDemo=$target.Demo;Responding=$p.Responding;
            WorkingSetMB=[math]::Round($p.WorkingSet64/1MB,1)
            UIControls=$controls.Count
            AppTitle=(Redact $target.Window.Current.Name)
        }|ConvertTo-Json -Depth 3
    } else {$controls|ConvertTo-Json -Depth 4}
    return
}
if($Action -eq 'screenshot'){Screenshot $target.Window $Output;return}
if($Action -eq 'wait'){
    if(!$Control){throw 'A wait action needs -Control <exact automation name>.'}
    $end=(Get-Date).AddSeconds($TimeoutSeconds)
    do {
        $present=Has-Control $target.Window $Control
        if($present -ne [bool]$Absent){Write-Output ('UI wait satisfied: '+(Redact $Control));return}
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $end)
    throw ('UI wait timed out after '+$TimeoutSeconds+'s: '+(Redact $Control))
}
if($Action -in @('click','set')) {
    if(!$Control){throw "An action needs -Control <exact UI automation name>."}
    if(-not $target.Demo -and -not $AllowSensitiveActions -and $Action -eq 'click' -and
        $Control -match '^(?:Remove account|Remove .+|Sign in(?: again)?|Device sign-in|Launch session|Resume session|Continue as .+|Save name|Add account)$'){
        throw 'This live action can change account state or start a session. Pass -AllowSensitiveActions to confirm.'
    }
    if($Action -eq 'click'){Click-Control $target.Window $Control $Index}
    else{Set-Control $target.Window $Control $Value $Index}
    Write-Output ("UI action completed: "+$Action+" ["+(Redact $Control)+"]")
    return
}
throw "Unsupported action: $Action"
