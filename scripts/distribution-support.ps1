# Shared by package creation and verification; requires only Windows PowerShell/.NET.
$ErrorActionPreference = 'Stop'
function Write-DistributionJson($Value, [string]$Path) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding $false))
}
function Get-DistributionFiles([string]$Directory) {
    $base = [IO.Path]::GetFullPath($Directory).TrimEnd('\') + '\'
    if (@(Get-ChildItem -LiteralPath $Directory -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count) { throw 'Package may not contain reparse points' }
    @(Get-ChildItem -LiteralPath $Directory -Recurse -Force -File | Sort-Object FullName | ForEach-Object {
        [pscustomobject]@{Path=$_.FullName.Substring($base.Length).Replace('\','/'); Bytes=$_.Length; Sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}
if (-not ('DistributionPe' -as [type])) {
    Add-Type @'
using System;
using System.IO;
using System.Collections.Generic;
public static class DistributionPe {
    public static string[] Imports(string path) {
        using (var r = new BinaryReader(File.OpenRead(path))) {
            if (r.ReadUInt16() != 0x5a4d) throw new Exception("Not a PE file: " + path);
            r.BaseStream.Position = 0x3c;
            uint pe = r.ReadUInt32(); r.BaseStream.Position = pe;
            if (r.ReadUInt32() != 0x4550 || r.ReadUInt16() != 0x8664) throw new Exception("Expected Windows x64 PE: " + path);
            int count = r.ReadUInt16(); r.BaseStream.Position = pe + 20;
            int optionalSize = r.ReadUInt16(); long optional = pe + 24;
            r.BaseStream.Position = optional;
            if (r.ReadUInt16() != 0x20b) throw new Exception("Expected PE32+");
            // Standard and delay-load import data directories.
            var directories = new List<uint>();
            foreach (int index in new int[] { 1, 13 }) {
                r.BaseStream.Position = optional + 112 + index * 8;
                directories.Add(r.ReadUInt32());
            }
            r.BaseStream.Position = optional + optionalSize;
            var sections = new List<uint[]>();
            for (int i = 0; i < count; i++) {
                r.ReadBytes(8); uint size = r.ReadUInt32(), address = r.ReadUInt32();
                uint rawSize = r.ReadUInt32(), offset = r.ReadUInt32(); r.ReadBytes(16);
                sections.Add(new uint[] { address, Math.Max(size, rawSize), offset });
            }
            Func<uint,long> map = delegate(uint address) {
                foreach (var s in sections) if (address >= s[0] && address - s[0] < s[1]) return s[2] + (long)(address - s[0]);
                throw new Exception("Unmapped PE address");
            };
            var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            for (int kind = 0; kind < 2; kind++) {
                if (directories[kind] == 0) continue;
                long start = map(directories[kind]); int stride = kind == 0 ? 20 : 32;
                for (int i = 0; i < 4096; i++) {
                    r.BaseStream.Position = start + i * stride;
                    uint first = r.ReadUInt32();
                    r.BaseStream.Position = start + i * stride + (kind == 0 ? 12 : 4);
                    uint name = r.ReadUInt32(); if (name == 0) break;
                    if (kind == 1 && (first & 1) == 0) throw new Exception("Unsupported VA delay imports");
                    r.BaseStream.Position = map(name); var chars = new List<byte>(); byte b;
                    while ((b = r.ReadByte()) != 0) { if (chars.Count > 256) throw new Exception("Invalid import name"); chars.Add(b); }
                    names.Add(System.Text.Encoding.ASCII.GetString(chars.ToArray()).ToLowerInvariant());
                }
            }
            var result = new List<string>(names); result.Sort(StringComparer.Ordinal); return result.ToArray();
        }
    }
}
'@
}
function Get-DistributionImports([string]$Directory) {
    # Explicit Windows components, not arbitrary DLLs installed on the development PC.
    $system = @('kernel32.dll','kernelbase.dll','advapi32.dll','oleaut32.dll','user32.dll','gdi32.dll','ole32.dll','dcomp.dll','dwmapi.dll','dwrite.dll','shell32.dll','dxgi.dll','d3d11.dll','combase.dll','imm32.dll','icuuc.dll','ntdll.dll','winmm.dll','comctl32.dll','uiautomationcore.dll','bcryptprimitives.dll','gdiplus.dll','userenv.dll','ws2_32.dll','crypt32.dll','bcrypt.dll','rpcrt4.dll','version.dll','shlwapi.dll','secur32.dll','setupapi.dll','wintrust.dll','normaliz.dll','ucrtbase.dll')
    $base = [IO.Path]::GetFullPath($Directory).TrimEnd('\') + '\'
    @(Get-ChildItem -LiteralPath $Directory -Recurse -Force -File | Where-Object Extension -In @('.exe','.dll') | Sort-Object FullName | ForEach-Object {
        $binary = $_
        foreach ($name in [DistributionPe]::Imports($binary.FullName)) {
            $local = Join-Path $binary.DirectoryName $name
            $appLocal = Join-Path $Directory $name
            $resolution = if (Test-Path -LiteralPath $local) { $local.Substring($base.Length).Replace('\','/') }
                elseif (Test-Path -LiteralPath $appLocal) { $name }
                elseif ($name -in $system -or $name -match '^(api-ms-|ext-ms-)') { 'windows-system' }
                else { throw "Unbundled dependency: $($binary.Name) -> $name" }
            [pscustomobject]@{Binary=$binary.FullName.Substring($base.Length).Replace('\','/'); Import=$name; Resolution=$resolution}
        }
    })
}
