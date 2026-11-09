param(
    [string]$ProcessName = "window-uia",
    [switch]$InvokeActivate
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::InputEncoding = $utf8
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8

function Get-CodePoints([string]$text) {
    $parts = @()
    foreach ($character in $text.ToCharArray()) {
        $parts += "U+{0:X4}" -f [int]$character
    }
    return $parts -join " "
}

$process = Get-Process $ProcessName | Select-Object -First 1
if ($null -eq $process) {
    throw "process '$ProcessName' is not running"
}

$root = [System.Windows.Automation.AutomationElement]::RootElement
$condition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
    $process.Id)
$window = $root.FindFirst(
    [System.Windows.Automation.TreeScope]::Children,
    $condition)
if ($null -eq $window) {
    throw "UIA did not expose a top-level window for process $($process.Id)"
}

$expectedName = ([char]0x6FC0).ToString() +
    ([char]0x6D3B).ToString() +
    " Activate"
$expectedCodePoints = Get-CodePoints $expectedName

Write-Output (
    "window name='{0}' class='{1}' control_type={2}" -f
    $window.Current.Name,
    $window.Current.ClassName,
    $window.Current.ControlType.ProgrammaticName)
Write-Output "expected_codepoints=$expectedCodePoints"

$elements = $window.FindAll(
    [System.Windows.Automation.TreeScope]::Subtree,
    [System.Windows.Automation.Condition]::TrueCondition)

$seenButton = $false
foreach ($element in $elements) {
    $name = $element.Current.Name
    $controlType = $element.Current.ControlType.ProgrammaticName
    $automationId = $element.Current.AutomationId
    Write-Output (
        "element name='{0}' id='{1}' control_type={2} enabled={3} codepoints={4}" -f
        $name,
        $automationId,
        $controlType,
        $element.Current.IsEnabled,
        (Get-CodePoints $name))

    if ($name -eq $expectedName -and $controlType -eq "ControlType.Button") {
        $seenButton = $true
    }
    if ($InvokeActivate -and $name -eq $expectedName) {
        $pattern = $element.GetCurrentPattern(
            [System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        Start-Sleep -Milliseconds 500
        Write-Output "invoked_codepoints=$expectedCodePoints"
    }
}

if (-not $seenButton) {
    throw "UIA button with codepoints '$expectedCodePoints' was not found"
}
