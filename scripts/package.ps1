[CmdletBinding()]
param([ValidatePattern('^[0-9A-Za-z][0-9A-Za-z._-]*$')][string]$Version='0.2.0-alpha.1',[switch]$SkipBuild)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$cargo=Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if(-not (Test-Path -LiteralPath $cargo)){$cargo=(Get-Command cargo.exe -ErrorAction Stop).Source}
Push-Location $root
try {
 if(-not $SkipBuild){& $cargo build --release --locked;if($LASTEXITCODE -ne 0){throw 'Release build failed.'}}
 $exe=Join-Path $root 'target\release\slotshift.exe'
 if(-not (Test-Path -LiteralPath $exe)){throw 'Release executable is missing.'}
 $notices=Join-Path $root 'dist\THIRD_PARTY_LICENSES.txt'
 if(-not (Test-Path -LiteralPath $notices)){throw 'Run python scripts/collect-licenses.py first.'}
 $stage=Join-Path $env:TEMP ('slotshift-package-'+[Guid]::NewGuid().ToString('N'))
 [void][IO.Directory]::CreateDirectory($stage)
 try {
  $files=@()
  foreach($source in @($exe,$notices,(Join-Path $root 'README.md'),(Join-Path $root 'LICENSE'),(Join-Path $root 'THIRD_PARTY_NOTICES.md'),(Join-Path $root 'SECURITY.md'))){
   $destination=Join-Path $stage ([IO.Path]::GetFileName($source));Copy-Item -LiteralPath $source -Destination $destination;$files+=$destination
  }
  $name='Slotshift-v'+$Version+'-windows-x64.zip';$zip=Join-Path $root ('dist\'+$name)
  Compress-Archive -LiteralPath $files -DestinationPath $zip -Force
  $sha=[Security.Cryptography.SHA256]::Create();$stream=[IO.File]::OpenRead($zip)
  try{$hash=[BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant()}finally{$stream.Dispose();$sha.Dispose()}
  [IO.File]::WriteAllText((Join-Path $root 'dist\SHA256SUMS.txt'),($hash+'  '+$name+"`n"),[Text.Encoding]::ASCII)
  Write-Output $zip;Write-Output ('SHA256='+$hash)
 }finally{Remove-Item -LiteralPath $stage -Recurse -Force}
}finally{Pop-Location}