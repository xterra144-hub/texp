#![allow(non_camel_case_types, non_snake_case)]
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;

use crate::state::OpenWithEntry;

type HRESULT = i32;
type HWND = *mut c_void;
type HMENU = *mut c_void;
type BOOL = i32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}
macro_rules! guid {
    ($d1:expr, $d2:expr, $d3:expr, $($d4:expr),+) => {
        GUID { data1: $d1, data2: $d2, data3: $d3, data4: [$($d4),+] }
    };
}

const IID_ICONTEXTMENU: GUID = guid!(0x000214E4, 0x0000, 0x0000, 0xC0,0x00,0x00,0x00,0x00,0x00,0x00,0x46);

#[repr(C)]
pub struct ITEMIDLIST {_opaque: [u8;0]}

#[repr(C)]
pub struct IUnknownVTbl {
    pub QueryInterface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(*mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
pub struct IShellFolderVtbl {
    pub parent: IUnknownVTbl,
    pub ParseDisplayName: unsafe extern "system" fn(
        this: *mut c_void,
        hwnd: HWND,
        pbc: *mut c_void,
        pszDisplayName: *const u16,
        pchEaten: *mut u32,
        ppidl: *mut *mut ITEMIDLIST,
        pdwAttributes: *mut u32,
    ) -> HRESULT,
    pub EnumObjects: unsafe extern "system" fn(*mut c_void, HWND, u32, *mut *mut c_void) -> HRESULT,
    pub BindToObject: unsafe extern "system" fn(
        this: *mut c_void,
        pidl: *const ITEMIDLIST,
        pbc: *mut c_void,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT,
    pub BindToStorage: unsafe extern "system" fn(*mut c_void, *const ITEMIDLIST, *mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub CompareIDs: unsafe extern "system" fn(*mut c_void, isize, *const ITEMIDLIST, *const ITEMIDLIST) -> HRESULT,
    pub CreateViewObject: unsafe extern "system" fn(*mut c_void, HWND, *const GUID, *mut *mut c_void) -> HRESULT,
    pub GetAttributesOf: unsafe extern "system" fn(*mut c_void, u32, *const *const ITEMIDLIST, *mut u32) -> HRESULT,
    pub GetUIObjectOf: unsafe extern "system" fn(
        this: *mut c_void,
        hwndOwner: HWND,
        cidl: u32,
        apidl: *const *const ITEMIDLIST,
        riid: *const GUID,
        rgfReserved: *mut u32,
        ppv: *mut *mut c_void,
    ) -> HRESULT,
    pub GetDisplayNameOf: unsafe extern "system" fn(*mut c_void, *const ITEMIDLIST, u32, *mut c_void) -> HRESULT,
    pub SetNameOf: unsafe extern "system" fn(*mut c_void, HWND, *const ITEMIDLIST, *const u16, u32, *mut *mut ITEMIDLIST) -> HRESULT,
}
#[repr(C)]
pub struct IShellFolder {
    pub lpVtbl: *const IShellFolderVtbl,
}

#[repr(C)]
pub struct CMINVOKECOMMANDINFO {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: *const i8,
    pub lpParameters: *const i8,
    pub lpDirectory: *const i8,
    pub nShow: i32,
    pub dwHotKey: u32,
    pub hIcon: *mut c_void,
}

#[repr(C)]
pub struct IContextMenuVtbl {
    pub parent: IUnknownVTbl,
    pub QueryContextMenu: unsafe extern "system" fn(
        *mut c_void, HMENU, u32, u32, u32, u32,
    ) -> HRESULT,
    pub InvokeCommand: unsafe extern "system" fn(*mut c_void, *const CMINVOKECOMMANDINFO) -> HRESULT,
    pub GetCommandString: unsafe extern "system" fn(*mut c_void, usize, u32, *mut u32, *mut i8, u32) -> HRESULT,
}

#[repr(C)]
pub struct IContextMenu { pub lpVtbl: *const IContextMenuVtbl }

#[repr(C)]
pub struct ContextMenuEntry {
    pub label: String,
    pub verb: String,
    pub is_separator: bool,
    pub indent: u32,
    pub cmd_id: u32,
}

#[repr(C)]
pub struct IEnumAssocHandlersVtbl {
    pub parent: IUnknownVTbl,
    pub Next: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, *mut u32) -> HRESULT,
    pub Skip: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub Reset: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub Clone: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
pub struct IEnumAssocHandlers { pub lpVtbl: *const IEnumAssocHandlersVtbl }

#[repr(C)]
pub struct IAssocHandlerVtbl {
    pub parent: IUnknownVTbl,
    pub GetUIName: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    pub GetName: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    pub IsRecommended: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub MakeDefault: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    pub Invoke: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    pub CreateInvoker: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
pub struct IAssocHandler { pub lpVtbl: *const IAssocHandlerVtbl }

const ASSOC_FILTER_RECOMMENDED: u32 = 0x00000001;
const IID_IENUMASSOCHANDLERS: GUID = guid!(0x2E7F653D, 0xF762, 0x4878, 0x91,0x3D,0x5E,0xF7,0x5F,0x3D,0xE0,0x0B);
const IID_IASSOCHANDLER: GUID = guid!(0x2E7F653E, 0xF762, 0x4878, 0x91,0x3D,0x5E,0xF7,0x5F,0x3D,0xE0,0x0B);

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoTaskMemFree(pv: *mut c_void);
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetDesktopFolder(ppshf: *mut *mut IShellFolder) -> HRESULT;
    fn ShellExecuteExW(pExecInfo: *mut SHELLEXECUTEINFOW) -> BOOL;
    fn SHAssocEnumHandlers(pszExtra: *const u16, afFilter: u32, ppEnumHandler: *mut *mut IEnumAssocHandlers) -> HRESULT;
}

#[repr(C)]
pub struct SHELLEXECUTEINFOW {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: *const u16,
    pub lpFile: *const u16,
    pub lpParameters: *const u16,
    pub lpDirectory: *const u16,
    pub nShow: i32,
    pub hInstApp: *mut c_void,
    pub lpIDList: *mut c_void,
    pub lpClass: *const u16,
    pub hKeyClass: *mut c_void,
    pub dwHotKey: u32,
    pub hMonitor: *mut c_void,
    pub hProcess: *mut c_void,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CreatePopupMenu() -> HMENU;
    fn DestroyMenu(hmenu: HMENU) -> BOOL;
    fn GetMenuItemCount(hMenu: HMENU) -> i32;
    fn GetMenuItemID(hMenu: HMENU, nPos: i32) -> u32;
    fn GetMenuStringW(hMenu: HMENU, uIDItem: u32, lpString: *mut u16, cchMax: i32, uFlags: u32) -> i32;
    fn GetSubMenu(hMenu: HMENU, nPos: i32) -> HMENU;
    fn GetConsoleWindow() -> HWND;
}

const MF_BYPOSITION: u32 = 0x0400;
const GCS_VERBW: u32 = 0x0004;
const CMF_EXPLORE: u32 = 0x0004;
const CMIC_MASK_UNICODE: u32 = 0x4000;
const SW_SHOWNORMAL: i32 = 1;
const SEE_MASK_NO_CONSOLE: u32 = 0x00008000;
const SEE_MASK_FLAG_NO_UI: u32 = 0x00000400;

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn strip_mnemonic(label: &str) -> String {
    label.replace("&", "")
}

pub fn query_context_menu_entries(paths: &[PathBuf]) -> Result<Vec<ContextMenuEntry>, String> {
    if paths.is_empty() {
        return Err("No files selected".into());
    }
    unsafe {
        let hr = CoInitializeEx(ptr::null_mut(), 2);
        if hr < 0 { return Err(format!("COM init failed: 0x{:08X}", hr as u32)); }

        let hwnd = GetConsoleWindow();
        let mut desktop: *mut IShellFolder = ptr::null_mut();
        let hr = SHGetDesktopFolder(&mut desktop);
        if hr < 0 { CoUninitialize(); return Err(format!("SHGetDesktopFolder failed: 0x{:08X}", hr as u32)); }

        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(paths.len());
        for path in paths {
            let wide = utf16(&path.to_string_lossy());
            let mut pidl: *mut ITEMIDLIST = ptr::null_mut();
            let hr = ((*(*desktop).lpVtbl).ParseDisplayName)(
                desktop as *mut c_void, hwnd, ptr::null_mut(),
                wide.as_ptr(), ptr::null_mut(), &mut pidl, ptr::null_mut(),
            );
            if hr < 0 {
                for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
                release_desktop(desktop);
                CoUninitialize();
                return Err(format!("ParseDisplayName failed: 0x{:08X}", hr as u32));
            }
            pidls.push(pidl);
        }

        let mut cm_ptr: *mut c_void = ptr::null_mut();
        let apidl: Vec<*const ITEMIDLIST> = pidls.iter().map(|&p| p as *const ITEMIDLIST).collect();
        let hr = ((*(*desktop).lpVtbl).GetUIObjectOf)(
            desktop as *mut c_void, hwnd,
            apidl.len() as u32, apidl.as_ptr(),
            &IID_ICONTEXTMENU, ptr::null_mut(), &mut cm_ptr,
        );
        if hr < 0 {
            for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
            release_desktop(desktop);
            CoUninitialize();
            return Err(format!("GetUIObjectOf failed: 0x{:08X}", hr as u32));
        }
        let cm = cm_ptr as *mut IContextMenu;

        let hmenu = CreatePopupMenu();
        if hmenu.is_null() {
            release_context_menu(cm); for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
            release_desktop(desktop); CoUninitialize();
            return Err("CreatePopupMenu failed".into());
        }

        ((*(*cm).lpVtbl).QueryContextMenu)(cm as *mut c_void, hmenu, 0, 1, 0x7FFF, CMF_EXPLORE);

        let mut entries = Vec::new();
        enumerate_hmenu(hmenu, cm, 0, &mut entries);

        // Sort: common actions (cut/copy/paste/delete/rename/link/properties/open) first
        fn verb_rank(entry: &ContextMenuEntry) -> u32 {
            match entry.verb.as_str() {
                "open" => 1,
                "cut" | "paste" => 2,
                "copy" => 3,
                "delete" => 4,
                "link" => 5,
                "rename" | "properties" => 6,
                _ => 10,
            }
        }
        if entries.len() > 1 {
            let mut sorted: Vec<ContextMenuEntry> = Vec::with_capacity(entries.len());
            let mut rest: Vec<ContextMenuEntry> = Vec::with_capacity(entries.len());
            for e in entries.drain(..) {
                if verb_rank(&e) < 10 { sorted.push(e); } else { rest.push(e); }
            }
            sorted.sort_by_key(verb_rank);
            sorted.extend(rest);
            entries = sorted;
        }

        DestroyMenu(hmenu);
        release_context_menu(cm);
        for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
        release_desktop(desktop);
        CoUninitialize();
        Ok(entries)
    }
}

unsafe fn enumerate_hmenu(hmenu: HMENU, cm: *mut IContextMenu, indent: u32, out: &mut Vec<ContextMenuEntry>) {
    let count = GetMenuItemCount(hmenu);
    let id_cmd_first: usize = 1;
    for i in 0..count {
        // Check if submenu
        let sub = GetSubMenu(hmenu, i);
        if !sub.is_null() {
            // Get label for this submenu header
            let mut buf = [0u16; 512];
            let len = GetMenuStringW(hmenu, i as u32, buf.as_mut_ptr(), 512, MF_BYPOSITION);
            if len > 0 {
                let label = strip_mnemonic(&String::from_utf16_lossy(&buf[..len as usize]));
                out.push(ContextMenuEntry {
                    label,
                    verb: String::new(),
                    is_separator: false,
                    indent,
                    cmd_id: 0,
                });
                enumerate_hmenu(sub, cm, indent + 1, out);
            }
            continue;
        }

        let id = GetMenuItemID(hmenu, i);
        if id == u32::MAX {
            // Separator or invalid
            out.push(ContextMenuEntry {
                label: String::new(),
                verb: String::new(),
                is_separator: true,
                indent,
                cmd_id: 0,
            });
            continue;
        }

        // Get label
        let mut buf = [0u16; 512];
        let len = GetMenuStringW(hmenu, i as u32, buf.as_mut_ptr(), 512, MF_BYPOSITION);
        if len <= 0 { continue; }
        let label = strip_mnemonic(&String::from_utf16_lossy(&buf[..len as usize]));

        // Get verb string
        let cmd_offset = (id as usize).wrapping_sub(id_cmd_first);
        let mut verb_buf = [0u16; 256];
        let verb_hr = ((*(*cm).lpVtbl).GetCommandString)(
            cm as *mut c_void, cmd_offset, GCS_VERBW, ptr::null_mut(),
            verb_buf.as_mut_ptr() as *mut i8, 256,
        );
        let verb = if verb_hr >= 0 {
            let end = verb_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&verb_buf[..end])
        } else {
            String::new()
        };

        out.push(ContextMenuEntry {
            label,
            verb,
            is_separator: false,
            indent,
            cmd_id: id,
        });
    }
}

pub fn invoke_verb(paths: &[PathBuf], verb: &str, cmd_id: u32) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Invalid arguments".into());
    }
    unsafe {
        let hr = CoInitializeEx(ptr::null_mut(), 2);
        if hr < 0 { return Err(format!("COM init failed: 0x{:08X}", hr as u32)); }

        let hwnd = GetConsoleWindow();
        let mut desktop: *mut IShellFolder = ptr::null_mut();
        let hr = SHGetDesktopFolder(&mut desktop);
        if hr < 0 { CoUninitialize(); return Err(format!("SHGetDesktopFolder failed: 0x{:08X}", hr as u32)); }

        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(paths.len());
        for path in paths {
            let wide = utf16(&path.to_string_lossy());
            let mut pidl: *mut ITEMIDLIST = ptr::null_mut();
            let hr = ((*(*desktop).lpVtbl).ParseDisplayName)(
                desktop as *mut c_void, hwnd, ptr::null_mut(),
                wide.as_ptr(), ptr::null_mut(), &mut pidl, ptr::null_mut(),
            );
            if hr < 0 {
                for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
                release_desktop(desktop); CoUninitialize();
                return Err(format!("ParseDisplayName failed: 0x{:08X}", hr as u32));
            }
            pidls.push(pidl);
        }

        let mut cm_ptr: *mut c_void = ptr::null_mut();
        let apidl: Vec<*const ITEMIDLIST> = pidls.iter().map(|&p| p as *const ITEMIDLIST).collect();
        let hr = ((*(*desktop).lpVtbl).GetUIObjectOf)(
            desktop as *mut c_void, hwnd,
            apidl.len() as u32, apidl.as_ptr(),
            &IID_ICONTEXTMENU, ptr::null_mut(), &mut cm_ptr,
        );
        if hr < 0 {
            for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
            release_desktop(desktop); CoUninitialize();
            return Err(format!("GetUIObjectOf failed: 0x{:08X}", hr as u32));
        }
        let cm = cm_ptr as *mut IContextMenu;

        let result = if !verb.is_empty() {
            // Use ShellExecuteExW for verb-based invocation (more reliable)
            let verb_w = utf16(verb);
            let file_w = utf16(&paths[0].to_string_lossy());
            let mut sei = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NO_CONSOLE | SEE_MASK_FLAG_NO_UI,
                hwnd,
                lpVerb: verb_w.as_ptr(),
                lpFile: file_w.as_ptr(),
                lpParameters: ptr::null(),
                lpDirectory: ptr::null(),
                nShow: SW_SHOWNORMAL,
                hInstApp: ptr::null_mut(),
                lpIDList: ptr::null_mut(),
                lpClass: ptr::null(),
                hKeyClass: ptr::null_mut(),
                dwHotKey: 0,
                hMonitor: ptr::null_mut(),
                hProcess: ptr::null_mut(),
            };
            let ok = ShellExecuteExW(&mut sei);
            release_context_menu(cm);
            for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
            release_desktop(desktop);
            CoUninitialize();
            if ok == 0 {
                Err(format!("ShellExecuteExW failed: last error={}", std::io::Error::last_os_error()))
            } else {
                Ok(())
            }
        } else {
            let hmenu = CreatePopupMenu();
            if hmenu.is_null() {
                release_context_menu(cm); for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
                release_desktop(desktop); CoUninitialize();
                return Err("CreatePopupMenu failed".into());
            }
            ((*(*cm).lpVtbl).QueryContextMenu)(cm as *mut c_void, hmenu, 0, 1, 0x7FFF, CMF_EXPLORE);
            let info = CMINVOKECOMMANDINFO {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                fMask: 0,
                hwnd,
                lpVerb: cmd_id as usize as *const i8,
                lpParameters: ptr::null(),
                lpDirectory: ptr::null(),
                nShow: 1,
                dwHotKey: 0,
                hIcon: ptr::null_mut(),
            };
            let invoke_hr = ((*(*cm).lpVtbl).InvokeCommand)(cm as *mut c_void, &info);
            DestroyMenu(hmenu);
            release_context_menu(cm);
            for &p in &pidls { CoTaskMemFree(p as *mut c_void); }
            release_desktop(desktop);
            CoUninitialize();
            if invoke_hr < 0 {
                Err(format!("InvokeCommand failed: 0x{:08X}", invoke_hr as u32))
            } else {
                Ok(())
            }
        };
        result
    }
}
pub fn query_open_with_entries(path: &Path) -> Vec<OpenWithEntry> {
    let mut entries: Vec<OpenWithEntry> = Vec::new();

    entries.push(OpenWithEntry {
        name: "Open in Explorer".into(),
        exe_path: String::new(),
    });

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext_w = utf16(&format!(".{}", if ext.is_empty() { "_folder_" } else { ext }));

    unsafe {
        let hr = CoInitializeEx(ptr::null_mut(), 2);
        if hr < 0 { return entries; }

        let mut enum_handler: *mut IEnumAssocHandlers = ptr::null_mut();
        let hr_enum = SHAssocEnumHandlers(
            if ext.is_empty() { ptr::null() } else { ext_w.as_ptr() },
            0, // ASSOC_FILTER_NONE — includes non-recommended handlers too
            &mut enum_handler,
        );

        if hr_enum >= 0 && !enum_handler.is_null() {
            loop {
                let mut handler: *mut c_void = ptr::null_mut();
                let mut fetched: u32 = 0;
                let hr_next = ((*(*enum_handler).lpVtbl).Next)(enum_handler as *mut c_void, 1, &mut handler, &mut fetched);
                if hr_next < 0 || fetched == 0 { break; }
                if handler.is_null() { break; }

                let ah = handler as *mut IAssocHandler;

                let mut name_ptr: *mut u16 = ptr::null_mut();
                if ((*(*ah).lpVtbl).GetUIName)(ah as *mut c_void, &mut name_ptr) >= 0 && !name_ptr.is_null() {
                    let name_len = (0..).find(|&i| *name_ptr.add(i) == 0).unwrap_or(0);
                    let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, name_len));
                    CoTaskMemFree(name_ptr as *mut c_void);

                    let mut exe_ptr: *mut u16 = ptr::null_mut();
                    let exe_path = if ((*(*ah).lpVtbl).GetName)(ah as *mut c_void, &mut exe_ptr) >= 0 && !exe_ptr.is_null() {
                        let exe_len = (0..).find(|&i| *exe_ptr.add(i) == 0).unwrap_or(0);
                        let exe = String::from_utf16_lossy(std::slice::from_raw_parts(exe_ptr, exe_len));
                        CoTaskMemFree(exe_ptr as *mut c_void);
                        exe
                    } else {
                        String::new()
                    };

                    let already = entries.iter().any(|e| e.name.eq_ignore_ascii_case(&name));
                    if !already {
                        entries.push(OpenWithEntry { name, exe_path });
                    }
                }

                ((*(*ah).lpVtbl).parent.Release)(ah as *mut c_void);
            }
            ((*(*enum_handler).lpVtbl).parent.Release)(enum_handler as *mut c_void);
        }
        CoUninitialize();
    }

    // Registry fallback: scan multiple shell subkey locations for additional programs
    use winreg::enums::*;
    let skip_verbs = ["open", "explore", "find", "runas", "pin", "Open", "Explore", "Find", "RunAs"];
    let reg_bases = [
        (HKEY_CLASSES_ROOT, ""),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Classes"),
        (HKEY_CURRENT_USER, "SOFTWARE\\Classes"),
    ];

    // Scan ProgID-based registrations: *\shell\name\command and Directory\shell\name\command
    for root_key in ["*", "Directory"] {
        for &(hkey, subpath) in &reg_bases {
            let path = if subpath.is_empty() {
                format!("{}\\shell", root_key)
            } else {
                format!("{}\\{}\\shell", subpath, root_key)
            };
            let Ok(base) = winreg::RegKey::predef(hkey).open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY) else { continue };
            for name in base.enum_keys().filter_map(|k| k.ok()) {
                if skip_verbs.contains(&name.as_str()) { continue; }
                // Read display name from the subkey itself (default value)
                let Ok(shell_entry) = base.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY) else { continue };
                let display_name = if let Ok(dflt) = shell_entry.get_value::<String, _>("") {
                    if dflt.is_empty() { name.clone() } else if dflt.starts_with('@') { name.clone() } else { dflt }
                } else {
                    name.clone()
                };
                if entries.iter().any(|e| e.name.eq_ignore_ascii_case(&display_name)) { continue; }
                // Check for DelegateExecute — skip COM-only handlers
                let has_delegate = shell_entry.get_value::<String, _>("DelegateExecute").is_ok();
                drop(shell_entry);
                if has_delegate { continue; }
                if let Ok(cmd_key) = base.open_subkey_with_flags(&format!("{}\\command", name), KEY_READ | KEY_WOW64_64KEY) {
                    if let Ok(cmd) = cmd_key.get_value::<String, _>("") {
                        let cmd = cmd.trim();
                        let exe = if cmd.starts_with('"') {
                            cmd[1..].split('"').next().unwrap_or("").to_string()
                        } else {
                            cmd.split_whitespace().next().unwrap_or("").to_string()
                        };
                        if !exe.is_empty() {
                            if !entries.iter().any(|e| e.name.eq_ignore_ascii_case(&display_name)) {
                                entries.push(OpenWithEntry { name: display_name, exe_path: exe });
                            }
                        }
                    }
                }
            }
        }
    }

    // Scan Applications registrations: Applications\name\shell\open\command
    // (used by VS Code, Zed, etc.)
    for &(hkey, subpath) in &reg_bases {
        let apps_path = if subpath.is_empty() {
            "Applications".to_string()
        } else {
            format!("{}\\Applications", subpath)
        };
            let Ok(apps_key) = winreg::RegKey::predef(hkey).open_subkey_with_flags(&apps_path, KEY_READ | KEY_WOW64_64KEY) else { continue };
            for app_name in apps_key.enum_keys().filter_map(|k| k.ok()) {
                let cmd_path = format!("{}\\shell\\open\\command", app_name);
                let Ok(cmd_key) = apps_key.open_subkey_with_flags(&cmd_path, KEY_READ | KEY_WOW64_64KEY) else { continue };
                if let Ok(cmd) = cmd_key.get_value::<String, _>("") {
                    let cmd = cmd.trim();
                    let exe = if cmd.starts_with('"') {
                        cmd[1..].split('"').next().unwrap_or("").to_string()
                    } else {
                        cmd.split_whitespace().next().unwrap_or("").to_string()
                    };
                    if !exe.is_empty() {
                        let display_name = app_name.trim_end_matches(".exe").to_string();
                        if !entries.iter().any(|e| e.name.eq_ignore_ascii_case(&display_name)) {
                            entries.push(OpenWithEntry { name: display_name, exe_path: exe });
                        }
                    }
                }
        }
    }

    entries
}

unsafe fn release_desktop(desktop: *mut IShellFolder) {
    if !desktop.is_null() {
        unsafe { ((*(*desktop).lpVtbl).parent.Release)(desktop as *mut c_void); }
    }
}

unsafe fn release_context_menu(cm: *mut IContextMenu) {
    if !cm.is_null() {
        unsafe { ((*(*cm).lpVtbl).parent.Release)(cm as *mut c_void); }
    }
}
