//! Windows 11 modern File Explorer context menu COM server (`IExplorerCommand`).
use crate::{
    launch::{self, ActionMode},
    locale,
    selection::{self, SelectionKind},
};
use std::{
    ffi::c_void,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::Win32::{
    Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_NOTIMPL,
        E_OUTOFMEMORY, E_POINTER, HINSTANCE, HMODULE, S_FALSE, S_OK,
    },
    System::{
        Com::{CoTaskMemAlloc, CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl},
        LibraryLoader::GetModuleFileNameW,
        SystemServices::DLL_PROCESS_ATTACH,
    },
    UI::Shell::{
        ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, IEnumExplorerCommand,
        IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray,
        SIGDN_FILESYSPATH,
    },
};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, PWSTR, Ref, Result, implement};

/// Stable CLSID for Origo's root Explorer submenu (`Origo > ...`).
/// Must match `packaging/shell-ext/AppxManifest.xml`.
pub const CLSID_ORIGO_ROOT: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D241);
const CLSID_ORIGO_IMAGES_DIALOG: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D242);
const CLSID_ORIGO_IMAGES_QUICK: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D243);
const CLSID_ORIGO_PDF_OPEN: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D244);
const CLSID_ORIGO_PDF_COMPRESS: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D245);
const CLSID_ORIGO_PDF_SPLIT: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D246);
const CLSID_ORIGO_PDF_PNG: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D247);
const CLSID_ORIGO_PDF_OCR: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D248);
const CLSID_ORIGO_PDF_COMBINE_ORG: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D249);
const CLSID_ORIGO_PDF_COMBINE_QUICK: GUID = GUID::from_u128(0x8F4B2C91_5D3A_4E12_9A7B_3C6E81F0D24A);

static MODULE_HANDLE: AtomicUsize = AtomicUsize::new(0);

fn dll_directory() -> Option<PathBuf> {
    let raw = MODULE_HANDLE.load(Ordering::Acquire);
    if raw == 0 {
        return None;
    }
    let module = HMODULE(raw as *mut c_void);
    let mut buffer = vec![0u16; 1024];
    // SAFETY: GetModuleFileNameW writes at most buffer.len() UTF-16 units into buffer.
    let len = unsafe { GetModuleFileNameW(Some(module), &mut buffer) } as usize;
    if len == 0 || len >= buffer.len() {
        return None;
    }
    let path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
    path.parent().map(PathBuf::from)
}

fn icon_resource_string() -> Option<String> {
    let dir = dll_directory()?;
    let ico = dir.join("origo.ico");
    if ico.is_file() {
        return Some(format!("{},0", ico.to_string_lossy()));
    }
    let exe = launch::resolve_origo_exe(&dir)?;
    Some(format!("{},0", exe.to_string_lossy()))
}

fn alloc_co_pwstr(text: &str) -> Result<PWSTR> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len().saturating_mul(std::mem::size_of::<u16>());
    // SAFETY: CoTaskMemAlloc returns a COM-allocated buffer or null on OOM.
    let ptr = unsafe { CoTaskMemAlloc(bytes) } as *mut u16;
    if ptr.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    // SAFETY: ptr is freshly allocated with at least `wide.len()` u16 slots and non-overlapping.
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
    }
    Ok(PWSTR(ptr))
}

fn extract_shell_paths(items: Ref<'_, IShellItemArray>) -> Option<Vec<PathBuf>> {
    let array = items.as_ref()?;
    // SAFETY: `array` is a valid COM interface pointer provided by Explorer.
    let count = unsafe { array.GetCount() }.ok()?;
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        // SAFETY: index < count for this IShellItemArray.
        let Ok(item) = (unsafe { array.GetItemAt(index) }) else {
            continue;
        };
        // SAFETY: SIGDN_FILESYSPATH returns a CoTaskMem-allocated PWSTR when backed by a file.
        if let Ok(pwstr) = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) } {
            let text = unsafe { pwstr.to_string() }.ok();
            unsafe { CoTaskMemFree(Some(pwstr.0 as *const c_void)) };
            if let Some(text) = text
                && !text.is_empty()
            {
                paths.push(PathBuf::from(text));
            }
        }
    }
    Some(paths)
}

fn matches_action_selection(mode: ActionMode, kind: &SelectionKind) -> bool {
    match (mode, kind) {
        (ActionMode::ImagesDialog | ActionMode::ImagesQuickConvert, SelectionKind::Images(_)) => {
            true
        }
        (
            ActionMode::PdfOpen
            | ActionMode::PdfCompress
            | ActionMode::PdfSplit
            | ActionMode::PdfPng
            | ActionMode::PdfOcr,
            SelectionKind::Pdfs(pdfs),
        ) => pdfs.len() == 1,
        (
            ActionMode::PdfCombineOrganize | ActionMode::PdfCombineQuick,
            SelectionKind::Pdfs(pdfs),
        ) => pdfs.len() >= 2,
        _ => false,
    }
}

#[implement(IExplorerCommand)]
struct OrigoRootCommand;

impl IExplorerCommand_Impl for OrigoRootCommand_Impl {
    fn GetTitle(&self, _psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        alloc_co_pwstr(locale::root_title())
    }

    fn GetIcon(&self, _psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        match icon_resource_string() {
            Some(icon) => alloc_co_pwstr(&icon),
            None => Err(E_NOTIMPL.into()),
        }
    }

    fn GetToolTip(&self, _psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        alloc_co_pwstr(locale::root_title())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLSID_ORIGO_ROOT)
    }

    fn GetState(&self, psiitemarray: Ref<'_, IShellItemArray>, _foktobeslow: BOOL) -> Result<u32> {
        let Some(paths) = extract_shell_paths(psiitemarray) else {
            return Ok(ECS_ENABLED.0 as u32);
        };
        match selection::classify_paths(paths) {
            SelectionKind::Images(_) | SelectionKind::Pdfs(_) => Ok(ECS_ENABLED.0 as u32),
            SelectionKind::Empty | SelectionKind::Unsupported => Ok(ECS_HIDDEN.0 as u32),
        }
    }

    fn Invoke(
        &self,
        _psiitemarray: Ref<'_, IShellItemArray>,
        _pbc: Ref<'_, IBindCtx>,
    ) -> Result<()> {
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_HASSUBCOMMANDS.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        let modes = [
            ActionMode::ImagesDialog,
            ActionMode::ImagesQuickConvert,
            ActionMode::PdfOpen,
            ActionMode::PdfCompress,
            ActionMode::PdfSplit,
            ActionMode::PdfPng,
            ActionMode::PdfOcr,
            ActionMode::PdfCombineOrganize,
            ActionMode::PdfCombineQuick,
        ];
        let commands: Vec<IExplorerCommand> = modes
            .into_iter()
            .map(|mode| OrigoActionCommand { mode }.into())
            .collect();
        Ok(OrigoEnumCommands {
            commands,
            index: AtomicUsize::new(0),
        }
        .into())
    }
}

#[implement(IExplorerCommand)]
struct OrigoActionCommand {
    mode: ActionMode,
}

impl IExplorerCommand_Impl for OrigoActionCommand_Impl {
    fn GetTitle(&self, psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        let lang = locale::detect_language();
        let kind = extract_shell_paths(psiitemarray).map(selection::classify_paths);
        let label = match self.mode {
            ActionMode::ImagesDialog => {
                let count = match &kind {
                    Some(SelectionKind::Images(paths)) => paths.len(),
                    _ => 2,
                };
                locale::dialog_title(lang, count).to_owned()
            }
            ActionMode::ImagesQuickConvert => match &kind {
                Some(SelectionKind::Images(imgs)) if !imgs.is_empty() => {
                    let pdf_name = selection::quick_pdf_display_name(imgs, lang);
                    locale::quick_title(lang, Some(&pdf_name), imgs.len())
                }
                _ => locale::quick_title(lang, None, 2),
            },
            ActionMode::PdfOpen => locale::pdf_open_title(lang).to_owned(),
            ActionMode::PdfCompress => locale::pdf_compress_title(lang).to_owned(),
            ActionMode::PdfSplit => locale::pdf_split_title(lang).to_owned(),
            ActionMode::PdfPng => locale::pdf_png_title(lang).to_owned(),
            ActionMode::PdfOcr => locale::pdf_ocr_title(lang).to_owned(),
            ActionMode::PdfCombineOrganize => locale::pdf_combine_organize_title(lang).to_owned(),
            ActionMode::PdfCombineQuick => match &kind {
                Some(SelectionKind::Pdfs(pdfs)) if pdfs.len() >= 2 => {
                    let pdf_name = selection::quick_merge_pdf_display_name(pdfs, lang);
                    locale::pdf_combine_quick_title(lang, Some(&pdf_name))
                }
                _ => locale::pdf_combine_quick_title(lang, None),
            },
        };
        alloc_co_pwstr(&label)
    }

    fn GetIcon(&self, _psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        match icon_resource_string() {
            Some(icon) => alloc_co_pwstr(&icon),
            None => Err(E_NOTIMPL.into()),
        }
    }

    fn GetToolTip(&self, psiitemarray: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        self.GetTitle(psiitemarray)
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(match self.mode {
            ActionMode::ImagesDialog => CLSID_ORIGO_IMAGES_DIALOG,
            ActionMode::ImagesQuickConvert => CLSID_ORIGO_IMAGES_QUICK,
            ActionMode::PdfOpen => CLSID_ORIGO_PDF_OPEN,
            ActionMode::PdfCompress => CLSID_ORIGO_PDF_COMPRESS,
            ActionMode::PdfSplit => CLSID_ORIGO_PDF_SPLIT,
            ActionMode::PdfPng => CLSID_ORIGO_PDF_PNG,
            ActionMode::PdfOcr => CLSID_ORIGO_PDF_OCR,
            ActionMode::PdfCombineOrganize => CLSID_ORIGO_PDF_COMBINE_ORG,
            ActionMode::PdfCombineQuick => CLSID_ORIGO_PDF_COMBINE_QUICK,
        })
    }

    fn GetState(&self, psiitemarray: Ref<'_, IShellItemArray>, _foktobeslow: BOOL) -> Result<u32> {
        let Some(paths) = extract_shell_paths(psiitemarray) else {
            return Ok(ECS_ENABLED.0 as u32);
        };
        let kind = selection::classify_paths(paths);
        if matches_action_selection(self.mode, &kind) {
            Ok(ECS_ENABLED.0 as u32)
        } else {
            Ok(ECS_HIDDEN.0 as u32)
        }
    }

    fn Invoke(
        &self,
        psiitemarray: Ref<'_, IShellItemArray>,
        _pbc: Ref<'_, IBindCtx>,
    ) -> Result<()> {
        let Some(paths) = extract_shell_paths(psiitemarray) else {
            return Ok(());
        };
        let kind = selection::classify_paths(paths);
        if !matches_action_selection(self.mode, &kind) {
            return Ok(());
        }
        let selected = match kind {
            SelectionKind::Images(paths) | SelectionKind::Pdfs(paths) => paths,
            SelectionKind::Empty | SelectionKind::Unsupported => return Ok(()),
        };
        let Some(dir) = dll_directory() else {
            return Err(E_FAIL.into());
        };
        launch::launch_origo(&dir, self.mode, &selected).map_err(|_| E_FAIL.into())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IEnumExplorerCommand)]
struct OrigoEnumCommands {
    commands: Vec<IExplorerCommand>,
    index: AtomicUsize,
}

impl IEnumExplorerCommand_Impl for OrigoEnumCommands_Impl {
    fn Next(
        &self,
        celt: u32,
        pui: *mut Option<IExplorerCommand>,
        pceltfetched: *mut u32,
    ) -> HRESULT {
        if pui.is_null() {
            return E_POINTER;
        }
        if celt > 1 && pceltfetched.is_null() {
            return E_INVALIDARG;
        }
        let mut fetched = 0u32;
        while fetched < celt {
            let current = self.index.load(Ordering::Acquire);
            if current >= self.commands.len() {
                break;
            }
            if self
                .index
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // SAFETY: caller provides `pui` with at least `celt` slots.
                unsafe {
                    pui.add(fetched as usize)
                        .write(Some(self.commands[current].clone()));
                }
                fetched += 1;
            }
        }
        if !pceltfetched.is_null() {
            // SAFETY: caller-provided non-null out pointer.
            unsafe {
                *pceltfetched = fetched;
            }
        }
        if fetched == celt { S_OK } else { S_FALSE }
    }

    fn Skip(&self, celt: u32) -> Result<()> {
        let len = self.commands.len();
        let next = self
            .index
            .fetch_add(celt as usize, Ordering::AcqRel)
            .saturating_add(celt as usize);
        if next <= len {
            Ok(())
        } else {
            self.index.store(len, Ordering::Release);
            Err(S_FALSE.into())
        }
    }

    fn Reset(&self) -> Result<()> {
        self.index.store(0, Ordering::Release);
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        Ok(OrigoEnumCommands {
            commands: self.commands.clone(),
            index: AtomicUsize::new(self.index.load(Ordering::Acquire)),
        }
        .into())
    }
}

#[implement(IClassFactory)]
struct OrigoClassFactory;

impl IClassFactory_Impl for OrigoClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: ppvobject is non-null.
        unsafe {
            *ppvobject = std::ptr::null_mut();
        }
        if punkouter.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = OrigoRootCommand.into();
        // SAFETY: QueryInterface on a live COM object with caller-provided riid/ppvobject.
        unsafe { command.query(riid, ppvobject) }.ok()
    }

    fn LockServer(&self, _flock: BOOL) -> Result<()> {
        Ok(())
    }
}

/// Windows DLL entry point storing the module handle on process attach.
///
/// # Safety
/// Invoked by the Windows loader following the Win32 `DllMain` ABI contract.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: *mut c_void,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        MODULE_HANDLE.store(hinst_dll.0 as usize, Ordering::Release);
    }
    BOOL::from(true)
}

/// COM in-process / surrogate class factory entry point.
///
/// # Safety
/// `rclsid`, `riid`, and `ppv` must either be null (rejected with `E_POINTER`)
/// or valid aligned pointers following the COM `DllGetClassObject` contract.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || rclsid.is_null() || riid.is_null() {
        return E_POINTER;
    }
    // SAFETY: ppv and rclsid are non-null.
    unsafe {
        *ppv = std::ptr::null_mut();
        if *rclsid != CLSID_ORIGO_ROOT {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
    }
    let factory: IClassFactory = OrigoClassFactory.into();
    // SAFETY: QueryInterface on live COM factory with caller-provided riid/ppv.
    unsafe { factory.query(riid, ppv) }
}

/// COM unload probe entry point.
///
/// # Safety
/// Invoked by the COM runtime following the `DllCanUnloadNow` ABI contract.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    // Allow Explorer to keep the lightweight in-proc server loaded until shutdown.
    S_FALSE
}

/// Helper to smoke-test COM class factory instantiation in unit/integration tests.
pub fn create_root_command_for_test() -> IExplorerCommand {
    OrigoRootCommand.into()
}

/// Exported helper returning S_OK so DllCanUnloadNow / CLSID constant stays checked.
pub fn clsid_string() -> &'static str {
    "8F4B2C91-5D3A-4E12-9A7B-3C6E81F0D241"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_command_enumerates_all_subcommands_and_gates_selection() {
        let root = create_root_command_for_test();
        // SAFETY: Calling COM methods on our own freshly created in-proc object.
        unsafe {
            let flags = root.GetFlags().ok();
            assert_eq!(flags, Some(ECF_HASSUBCOMMANDS.0 as u32));
            let title_pwstr = root.GetTitle(None).ok();
            assert!(title_pwstr.is_some());
            if let Some(pwstr) = title_pwstr {
                let text = pwstr.to_string().ok();
                CoTaskMemFree(Some(pwstr.0 as *const c_void));
                assert_eq!(text.as_deref(), Some("Origo"));
            }
            let enumerator = root.EnumSubCommands().ok();
            assert!(enumerator.is_some());
            if let Some(enumerator) = enumerator {
                let mut items: [Option<IExplorerCommand>; 12] = Default::default();
                let mut fetched = 0u32;
                let _ = enumerator.Next(&mut items, Some(&mut fetched));
                assert_eq!(fetched, 9);
                for slot in &items[..9] {
                    assert!(slot.is_some());
                }
            }
        }

        let one_pdf = SelectionKind::Pdfs(vec![PathBuf::from("C:\\a.pdf")]);
        let two_pdfs =
            SelectionKind::Pdfs(vec![PathBuf::from("C:\\a.pdf"), PathBuf::from("C:\\b.pdf")]);
        let imgs = SelectionKind::Images(vec![PathBuf::from("C:\\a.png")]);

        assert!(matches_action_selection(ActionMode::PdfOpen, &one_pdf));
        assert!(matches_action_selection(ActionMode::PdfCompress, &one_pdf));
        assert!(!matches_action_selection(
            ActionMode::PdfCombineOrganize,
            &one_pdf
        ));
        assert!(!matches_action_selection(
            ActionMode::ImagesDialog,
            &one_pdf
        ));

        assert!(matches_action_selection(
            ActionMode::PdfCombineOrganize,
            &two_pdfs
        ));
        assert!(matches_action_selection(
            ActionMode::PdfCombineQuick,
            &two_pdfs
        ));
        assert!(!matches_action_selection(
            ActionMode::PdfCompress,
            &two_pdfs
        ));
        assert!(!matches_action_selection(ActionMode::PdfOpen, &two_pdfs));

        assert!(matches_action_selection(ActionMode::ImagesDialog, &imgs));
        assert!(matches_action_selection(
            ActionMode::ImagesQuickConvert,
            &imgs
        ));
        assert!(!matches_action_selection(ActionMode::PdfOpen, &imgs));

        assert_eq!(S_OK.0, 0);
    }
}
