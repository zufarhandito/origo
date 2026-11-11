# Independent native check for environments where the Rust linker is unavailable.
# This is NOT a replacement for the Rust smoke binary or GPUI acceptance.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$qpdf = Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
$fixture = Join-Path $root 'tests/fixtures/two-pages.pdf'
$outputDirectory = Join-Path $root ('.research/probe-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $outputDirectory | Out-Null
$output = Join-Path $outputDirectory 'result.pdf'
$unicodeName = ([string][char]0x65E5) + [char]0x672C + ' & input.pdf'
$secondSource = Join-Path $outputDirectory $unicodeName
Copy-Item -LiteralPath $fixture -Destination $secondSource
$before = (Get-FileHash -LiteralPath $fixture).Hash
& $qpdf --version
if ($LASTEXITCODE -ne 0) { throw 'QPDF unavailable' }
& $qpdf --check $fixture
if ($LASTEXITCODE -ne 0) { throw 'Invalid synthetic fixture' }
& $qpdf --empty --rotate=+90:1 --pages $fixture 2 $secondSource 1 -- $output
if ($LASTEXITCODE -ne 0) { throw 'Synthetic transform failed' }
& $qpdf --check $output
if ($LASTEXITCODE -ne 0) { throw 'Invalid output' }
$library = (Join-Path $root 'sidecars/pdfium/bin/pdfium.dll').Replace('\', '\\')
$typeName = 'NativeProbe' + [Guid]::NewGuid().ToString('N')
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class $typeName {
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDF_InitLibrary();
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDF_DestroyLibrary();
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern IntPtr FPDF_LoadDocument(byte[] file, IntPtr password);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDF_CloseDocument(IntPtr doc);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern int FPDF_GetPageCount(IntPtr doc);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern IntPtr FPDF_LoadPage(IntPtr doc, int page);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDF_ClosePage(IntPtr page);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern int FPDFPage_GetRotation(IntPtr page);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern IntPtr FPDFBitmap_Create(int width, int height, int alpha);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDFBitmap_Destroy(IntPtr bitmap);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern void FPDF_RenderPageBitmap(IntPtr bitmap, IntPtr page, int x, int y, int width, int height, int rotation, int flags);
    [DllImport("$library", CallingConvention = CallingConvention.Cdecl)] public static extern IntPtr FPDFBitmap_GetBuffer(IntPtr bitmap);
    public static int[] Sample(string path) {
        FPDF_InitLibrary();
        try {
            IntPtr doc = FPDF_LoadDocument(System.Text.Encoding.UTF8.GetBytes(path + "\0"), IntPtr.Zero);
            if(doc == IntPtr.Zero) throw new Exception("PDFium load failed");
            try {
                if(FPDF_GetPageCount(doc) != 2) throw new Exception("Page count mismatch");
                int[] samples = new int[4];
                for(int i=0; i<2; i++) {
                    IntPtr page = FPDF_LoadPage(doc, i);
                    if(page == IntPtr.Zero) throw new Exception("Missing page");
                    try {
                        samples[i*2] = FPDFPage_GetRotation(page);
                        IntPtr bitmap = FPDFBitmap_Create(120, 160, 0);
                        if(bitmap == IntPtr.Zero) throw new Exception("Bitmap allocation failed");
                        try {
                            FPDF_RenderPageBitmap(bitmap, page, 0, 0, 120, 160, 0, 0);
                            // BGRA pixel away from text. Both fixtures have solid backgrounds.
                            samples[i*2+1] = Marshal.ReadInt32(FPDFBitmap_GetBuffer(bitmap)) & 0xffffff;
                        } finally { FPDFBitmap_Destroy(bitmap); }
                    } finally { FPDF_ClosePage(page); }
                }
                return samples;
            } finally { FPDF_CloseDocument(doc); }
        } finally { FPDF_DestroyLibrary(); }
    }
}
"@
$type = $typeName -as [type]
$sourceSamples = $type::Sample($fixture)
$outputSamples = $type::Sample($output)
if ($sourceSamples[1] -ne 0xff0000 -or $sourceSamples[3] -ne 0x0000ff) { throw 'Source pixel color mismatch' }
if ($outputSamples[0] -ne 1 -or $outputSamples[2] -ne 0) { throw 'Output rotation mismatch' }
if ($outputSamples[1] -ne $sourceSamples[3] -or $outputSamples[3] -ne $sourceSamples[1]) { throw 'Output page order mismatch' }
if ((Get-FileHash -LiteralPath $fixture).Hash -ne $before) { throw 'Source bytes changed' }
if ((Get-FileHash -LiteralPath $secondSource).Hash -ne $before) { throw 'Second source bytes changed' }
Write-Host 'PASS: native QPDF + PDFium count, raster color order, rotation, unchanged source.'
Write-Host "Probe artifacts: $outputDirectory"
