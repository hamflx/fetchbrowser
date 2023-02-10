$repo = "hamflx/fetchbrowser"
$file = "fb.exe"

$releases = "https://api.github.com/repos/$repo/releases"

Write-Host Determining latest release
$tag = (Invoke-WebRequest $releases | ConvertFrom-Json)[0].tag_name

$download = "https://github.com/$repo/releases/download/$tag/$file"
$fb_dir = "$HOME/.fb"
$fb_bin_dir = "$fb_dir/bin"
$fb_bin_path = "$fb_bin_dir/$file"

New-Item "$fb_bin_dir" -ItemType Directory -Force

Write-Host Dowloading latest release
Invoke-WebRequest $download -Out $fb_bin_path

$old_path = [System.Environment]::GetEnvironmentVariable("PATH", "User")
if ($old_path -notcontains $fb_bin_path) {
  $new_path = $old_path + [IO.Path]::PathSeparator + $fb_bin_path
  [Environment]::SetEnvironmentVariable("PATH", $new_path, "User")
}
