use std::mem;
use std::ptr;
use std::thread;

use winapi::shared::{
    minwindef::{LOWORD, LPARAM, LRESULT, UINT, WPARAM},
    windef::{HWND, POINT},
};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::shellapi::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use winapi::um::wingdi::{CreateSolidBrush, RGB};
use winapi::um::winuser::{
    CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetCursorPos, GetMessageW, InsertMenuW, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterClassExW, SendMessageW, SetFocus, SetForegroundWindow,
    SetMenuDefaultItem, TrackPopupMenu, TranslateMessage, LR_DEFAULTCOLOR, MB_ICONINFORMATION,
    MB_OK, MF_BYPOSITION, MF_STRING, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_INITMENUPOPUP, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
    WNDCLASSEXW, WS_EX_NOACTIVATE,
};

use crate::Message;
use crate::CHANNEL;

const ID_ABOUT: u16 = 2000;
const ID_EXIT: u16 = 2001;
static mut MODAL_SHOWN: bool = false;

pub unsafe fn spawn_sys_tray() {
    thread::spawn(|| {
        let hInstance = GetModuleHandleW(ptr::null());

        let class_name = "Grout Tray";
        let mut class_name = class_name.encode_utf16().collect::<Vec<_>>();
        class_name.push(0);

        let mut class = mem::zeroed::<WNDCLASSEXW>();
        class.cbSize = mem::size_of::<WNDCLASSEXW>() as u32;
        class.lpfnWndProc = Some(callback);
        class.hInstance = hInstance;
        class.lpszClassName = class_name.as_ptr();
        class.hbrBackground = CreateSolidBrush(RGB(0, 77, 128));

        RegisterClassExW(&class);

        CreateWindowExW(
            WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            ptr::null(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            hInstance,
            ptr::null_mut(),
        );

        let mut msg = mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe fn add_icon(hwnd: HWND) {
    let icon_bytes = include_bytes!("../assets/icon_32.png");

    let icon_handle = CreateIconFromResourceEx(
        icon_bytes.as_ptr() as *mut _,
        icon_bytes.len() as u32,
        1,
        0x00_030_000,
        32,
        32,
        LR_DEFAULTCOLOR,
    );

    let mut tooltip_array = [0u16; 128];
    let tooltip = "Grout";
    let mut tooltip = tooltip.encode_utf16().collect::<Vec<_>>();
    tooltip.extend(vec![0; 128 - tooltip.len()]);
    tooltip_array.swap_with_slice(&mut tooltip[..]);

    let mut icon_data: NOTIFYICONDATAW = mem::zeroed();
    icon_data.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    icon_data.hWnd = hwnd;
    icon_data.uID = 1;
    icon_data.uCallbackMessage = WM_APP;
    icon_data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    icon_data.hIcon = icon_handle;
    icon_data.szTip = tooltip_array;

    Shell_NotifyIconW(NIM_ADD, &mut icon_data);
}

unsafe fn remove_icon(hwnd: HWND) {
    let mut icon_data: NOTIFYICONDATAW = mem::zeroed();
    icon_data.hWnd = hwnd;
    icon_data.uID = 1;

    Shell_NotifyIconW(NIM_DELETE, &mut icon_data);
}

unsafe fn show_popup_menu(hwnd: HWND) {
    if MODAL_SHOWN {
        return;
    }

    let menu = CreatePopupMenu();

    let about = "About...";
    let mut about = about.encode_utf16().collect::<Vec<_>>();
    about.push(0);

    let exit = "Exit";
    let mut exit = exit.encode_utf16().collect::<Vec<_>>();
    exit.push(0);

    InsertMenuW(
        menu,
        0,
        MF_BYPOSITION | MF_STRING,
        ID_ABOUT as usize,
        about.as_mut_ptr(),
    );

    InsertMenuW(
        menu,
        1,
        MF_BYPOSITION | MF_STRING,
        ID_EXIT as usize,
        exit.as_mut_ptr(),
    );

    SetMenuDefaultItem(menu, ID_ABOUT as u32, 0);
    SetFocus(hwnd);
    SendMessageW(hwnd, WM_INITMENUPOPUP, menu as usize, 0);

    let mut point: POINT = mem::zeroed();
    GetCursorPos(&mut point);

    let cmd = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
        point.x,
        point.y,
        0,
        hwnd,
        ptr::null_mut(),
    );

    SendMessageW(hwnd, WM_COMMAND, cmd as usize, 0);

    DestroyMenu(menu);
}

unsafe fn show_about() {
    let title = "About";
    let mut title = title.encode_utf16().collect::<Vec<_>>();
    title.push(0);

    let msg = format!(
        "Grout - v{}\n\nCopyright © 2020 Cory Forsstrom",
        env!("CARGO_PKG_VERSION")
    );
    let mut msg = msg.encode_utf16().collect::<Vec<_>>();
    msg.push(0);

    MessageBoxW(
        ptr::null_mut(),
        msg.as_mut_ptr(),
        title.as_mut_ptr(),
        MB_ICONINFORMATION | MB_OK,
    );
}

unsafe extern "system" fn callback(
    hWnd: HWND,
    Msg: UINT,
    wParam: WPARAM,
    lParam: LPARAM,
) -> LRESULT {
    match Msg {
        WM_CREATE => {
            add_icon(hWnd);
            return 0;
        }
        WM_CLOSE => {
            remove_icon(hWnd);
            PostQuitMessage(0);
            let _ = &CHANNEL.0.clone().send(Message::Exit);
        }
        WM_COMMAND => {
            if MODAL_SHOWN {
                return 1;
            }

            match LOWORD(wParam as u32) {
                ID_ABOUT => {
                    MODAL_SHOWN = true;

                    show_about();

                    MODAL_SHOWN = false;
                }
                ID_EXIT => {
                    PostMessageW(hWnd, WM_CLOSE, 0, 0);
                }
                _ => {}
            }

            return 0;
        }
        WM_APP => {
            match lParam as u32 {
                WM_LBUTTONDBLCLK => show_about(),
                WM_RBUTTONUP => {
                    SetForegroundWindow(hWnd);
                    show_popup_menu(hWnd);
                    PostMessageW(hWnd, WM_APP + 1, 0, 0);
                }
                _ => {}
            }

            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hWnd, Msg, wParam, lParam)
}