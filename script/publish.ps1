# if ($env:APPVEYOR_REPO_TAG) {
#     Write-Host "Not a tag: skip publishing."
#     exit 0    
# }

# $version = $env:APPVEYOR_REPO_TAG_NAME
$version = "0.4.2"
$github_username = $env:GITHUB_USERNAME
$github_token = $env:GITHUB_TOKEN

if ($env:target -ne "x86_64-pc-windows-msvc") {
    Write-Host "Target $env:target detected: skip publishing."
    exit 0
}

git clone "https://${github_username}:${github_token}@github.com/aerys/gpm-packages.git" 2>&1 | Write-Host
mkdir -p gpm-packages/gpm-windows64
Compress-Archive -Path .\target\release\gpm.exe -DestinationPath .\gpm-packages\gpm-windows64\gpm-windows64.zip
cd gpm-packages/gpm-windows64
git add gpm-windows64.zip 2>&1 | Write-Host
git commit gpm-windows64.zip -m "Publish gpm-windows64 version ${version}." 2>&1 | Write-Host
git tag gpm-windows64/${version} 2>&1 | Write-Host
# git push
# git push --tags
git status 2>&1 | Write-Host
