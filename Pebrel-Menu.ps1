Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$runner = Join-Path $PSScriptRoot 'pebrel-local.ps1'

function Show-PebrelMenu([string] $Status) {
    $form = New-Object Windows.Forms.Form
    try {
        $form.Text = 'Pebrel 本地运行与打包'
        $form.StartPosition = 'CenterScreen'
        $form.AutoScaleMode = 'Dpi'
        $form.AutoSize = $true
        $form.AutoSizeMode = 'GrowAndShrink'
        $form.FormBorderStyle = 'FixedDialog'
        $form.MaximizeBox = $false
        $form.MinimizeBox = $false
        $form.Font = [Drawing.SystemFonts]::MessageBoxFont

        $panel = New-Object Windows.Forms.FlowLayoutPanel
        $panel.FlowDirection = 'TopDown'
        $panel.WrapContents = $false
        $panel.AutoSize = $true
        $panel.Padding = [Windows.Forms.Padding]::new(16)
        $form.Controls.Add($panel)

        foreach ($choice in @(
            @{ Text = '启动 Release 版'; Action = 'Run'; Profile = 'release' }
            @{ Text = '启动 Debug 版'; Action = 'Run'; Profile = 'debug' }
            @{ Text = 'Debug 打包（本机使用）'; Action = 'Debug'; Profile = 'debug' }
            @{ Text = 'Release 打包（便携 ZIP）'; Action = 'Release'; Profile = 'release' }
            @{ Text = '生成安装包（需要 Inno Setup 6）'; Action = 'Installer'; Profile = 'release' }
        )) {
            $button = New-Object Windows.Forms.Button
            $button.Text = $choice.Text
            $button.Tag = $choice
            $button.Size = [Drawing.Size]::new(320, 36)
            if ($choice.Action -eq 'Run' -and -not (Test-Path -LiteralPath (
                Join-Path $PSScriptRoot "target\$($choice.Profile)\pebrel.exe"
            ) -PathType Leaf)) {
                $button.Text += '（待编译）'
                $button.Enabled = $false
            }
            $button.Add_Click({
                param($sender, $eventArgs)
                $dialog = $sender.FindForm()
                $dialog.Tag = $sender.Tag
                $dialog.DialogResult = 'OK'
            })
            $panel.Controls.Add($button)
        }

        $overwrite = New-Object Windows.Forms.CheckBox
        $overwrite.Text = '覆盖 dist 中已有的同名包'
        $overwrite.AutoSize = $true
        $panel.Controls.Add($overwrite)

        if ($Status) {
            $label = New-Object Windows.Forms.Label
            $label.Text = $Status
            $label.AutoSize = $true
            $panel.Controls.Add($label)
        }

        if ($form.ShowDialog() -eq [Windows.Forms.DialogResult]::OK) {
            return @{ Choice = $form.Tag; Force = $overwrite.Checked }
        }
    } finally {
        $form.Dispose()
    }
}

$status = ''
while ($selection = Show-PebrelMenu $status) {
    $choice = $selection.Choice
    try {
        & $runner -Action $choice.Action -Profile $choice.Profile -Force:$selection.Force
        if ($choice.Action -eq 'Run') { return }
        $status = '打包完成，产物位于项目的 dist 目录。'
    } catch {
        $status = '操作失败。详细错误见控制台。'
        Write-Host $_.Exception.Message -ForegroundColor Red
        [Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Pebrel 操作失败', 'OK', 'Error') | Out-Null
    }
}
