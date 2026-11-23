# Build a project-local, offline Tesseract runtime. Run from Windows x64.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $root 'sidecars/ocr'
if (Test-Path -LiteralPath $destination) {
    throw 'sidecars/ocr already exists. Use the existing runtime or move it aside before rebuilding.'
}
. (Join-Path $PSScriptRoot 'use-build-environment.ps1')

$sources = Join-Path $root '.tools/ocr-sources'
$work = Join-Path $root '.tools/ocr-build'
New-Item -ItemType Directory -Force $sources,$work | Out-Null
$models = 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/65727574dfcd264acbb0c3e07860e4e9e9b22185'
$inputs = @(
    @('leptonica-1.87.0.tar.gz', 'https://github.com/DanBloomberg/leptonica/releases/download/1.87.0/leptonica-1.87.0.tar.gz', 'c73363397f96eb1295602bf44d708a994ad42046c791bf03ea0505d829bdb6a7'),
    @('tesseract-5.5.4.tar.gz', 'https://codeload.github.com/tesseract-ocr/tesseract/tar.gz/refs/tags/5.5.4', 'a8dbf8b8c8ee427ff6ad1d99116502feaf59440af1adb893caf60a6576826cf4'),
    @('eng.traineddata', "$models/eng.traineddata", '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2'),
    @('ind.traineddata', "$models/ind.traineddata", '69786901da87ab8766c1ea7fbb10b28f2110c14da3f6c8f2735df131fba95d88'),
    @('tessdata-LICENSE', "$models/LICENSE", 'cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30')
)
foreach ($inputFile in $inputs) {
    $file = Join-Path $sources $inputFile[0]
    if (-not (Test-Path -LiteralPath $file)) {
        Invoke-WebRequest -UseBasicParsing -Uri $inputFile[1] -OutFile "$file.partial"
        if ((Get-FileHash -LiteralPath "$file.partial").Hash -ne $inputFile[2]) {
            throw "Checksum mismatch for $($inputFile[0])"
        }
        Move-Item -LiteralPath "$file.partial" -Destination $file
    }
    if ((Get-FileHash -LiteralPath $file).Hash -ne $inputFile[2]) {
        throw "Checksum mismatch for $($inputFile[0])"
    }
}

$cmakeRoot = Join-Path $root '.tools/vs2022/Common7/IDE/CommonExtensions/Microsoft/CMake'
$cmake = Join-Path $cmakeRoot 'CMake/bin/cmake.exe'
$ninja = Join-Path $cmakeRoot 'Ninja/ninja.exe'
function Invoke-CMake {
    & $cmake @args
    if ($LASTEXITCODE -ne 0) { throw 'OCR sidecar CMake command failed' }
}
foreach ($archive in @('leptonica-1.87.0','tesseract-5.5.4')) {
    & tar -xf (Join-Path $sources "$archive.tar.gz") -C $work
    if ($LASTEXITCODE -ne 0) { throw "Extraction failed for $archive" }
}

$leptonica = Join-Path $work 'leptonica-1.87.0'
$tesseract = Join-Path $work 'tesseract-5.5.4'
$leptBuild = Join-Path $work 'leptonica-build'
$leptInstall = Join-Path $work 'leptonica-install'
$tessBuild = Join-Path $work 'tesseract-build'
$common = @('-G','Ninja',"-DCMAKE_MAKE_PROGRAM=$ninja",'-DCMAKE_BUILD_TYPE=Release',
    '-DBUILD_SHARED_LIBS=OFF','-DCMAKE_POLICY_DEFAULT_CMP0091=NEW',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DCMAKE_C_FLAGS_RELEASE=/O2 /Ob2 /DNDEBUG')
Invoke-CMake -S $leptonica -B $leptBuild @common "-DCMAKE_INSTALL_PREFIX=$leptInstall" `
    -DSW_BUILD=OFF -DENABLE_ZLIB=OFF -DENABLE_PNG=OFF -DENABLE_GIF=OFF -DENABLE_JPEG=OFF `
    -DENABLE_TIFF=OFF -DENABLE_WEBP=OFF -DENABLE_OPENJPEG=OFF
Invoke-CMake --build $leptBuild --target install --parallel 4
Invoke-CMake -S $tesseract -B $tessBuild @common "-DCMAKE_PREFIX_PATH=$leptInstall" `
    -DBUILD_TRAINING_TOOLS=OFF -DGRAPHICS_DISABLED=ON -DOPENMP_BUILD=OFF `
    -DDISABLE_TIFF=ON -DDISABLE_ARCHIVE=ON -DDISABLE_CURL=ON -DENABLE_NATIVE=OFF
Invoke-CMake --build $tessBuild --target tesseract --parallel 4

$stage = Join-Path (Split-Path $destination -Parent) ('.ocr-' + [Guid]::NewGuid().ToString('N'))
$tessdata = Join-Path $stage 'tessdata'
$licenses = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Force (Join-Path $tessdata 'configs'),$licenses | Out-Null
Copy-Item -LiteralPath (Join-Path $tessBuild 'bin/tesseract.exe') -Destination $stage
Copy-Item -LiteralPath (Join-Path $tesseract 'tessdata/pdf.ttf') -Destination $stage
Copy-Item -LiteralPath (Join-Path $tesseract 'tessdata/configs/tsv') -Destination (Join-Path $tessdata 'configs')
foreach ($language in @('eng','ind')) {
    Copy-Item -LiteralPath (Join-Path $sources "$language.traineddata") -Destination $tessdata
}
Copy-Item -LiteralPath (Join-Path $tesseract 'LICENSE') -Destination (Join-Path $licenses 'tesseract.txt')
Copy-Item -LiteralPath (Join-Path $leptonica 'leptonica-license.txt') -Destination (Join-Path $licenses 'leptonica.txt')
Copy-Item -LiteralPath (Join-Path $sources 'tessdata-LICENSE') -Destination (Join-Path $licenses 'tessdata.txt')
$binary = Join-Path $stage 'tesseract.exe'
$version = & $binary --version
if ($LASTEXITCODE -ne 0 -or $version[0] -ne 'tesseract 5.5.4') { throw 'Unexpected OCR engine version' }
$languages = & $binary --tessdata-dir $tessdata --list-langs
if ($LASTEXITCODE -ne 0 -or 'eng' -notin $languages -or 'ind' -notin $languages) { throw 'OCR language check failed' }
Move-Item -LiteralPath $stage -Destination $destination
Write-Host 'Built and verified sidecars/ocr (Tesseract 5.5.4, eng + ind).'
