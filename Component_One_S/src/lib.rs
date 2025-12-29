use std::cell::RefCell;
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_int};
use serde_json::Value;

use native_api_1c::{
    native_api_1c_core::ffi::connection::Connection,
    native_api_1c_macro::AddIn,
};

use windows::core::s;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GetProcAddress,
    LoadLibraryA,
    FreeLibrary,  // <-- Тепер буде видно!
};

#[repr(C)]
struct SocketResult {
    data: [c_char; 8192],
    error_code: c_int,
}

type SendToSocketFn = extern "system" fn(
    *const c_char,
    c_int,
    *const c_char,
    c_int,
    c_int,
) -> SocketResult;

#[derive(Debug, Clone)]
enum ErrEn {
    InvalidIP,
    InvalidPort,
    ComponentNoInit,
    ConnectError,
    ServerDropConnection,
    JsonParseError,
    DllError,
    Other,
}

#[derive(AddIn)]
pub struct MyAddIn {
    #[add_in_con]
    connection: Arc<Option<&'static Connection>>,

    #[add_in_prop(name = "MyProp", name_ru = "МоеСвойство", readable, writable)]
    pub some_prop: i32,

    #[add_in_func(name = "SendToSocket", name_ru = "ОтправитьВСокет")]
    #[arg(Str)]
    #[returns(Str, result)]
    pub send_to_socket: fn(&Self, String) -> Result<String, ()>,

    #[add_in_func(name = "GetResponse", name_ru = "ПолучитьОтвет")]
    #[arg(Str)]
    #[returns(Str)]
    pub get_resp: fn(&Self, String) -> String,

    #[add_in_func(name = "InitSocket", name_ru = "ИнициализироватьСокет")]
    #[arg(Str)]
    #[arg(Int)]
    #[returns(Str, result)]
    pub socket_init: fn(&Self, String, i32) -> Result<String, ()>,

    target_ip: RefCell<Option<String>>,
    target_port: RefCell<Option<i32>>,
    dll_handle: RefCell<Option<HMODULE>>,
    send_fn: RefCell<Option<SendToSocketFn>>,

    #[add_in_func(name = "LastErr", name_ru = "ПоследняяОшибка")]
    #[returns(Str)]
    pub last_err: fn(&Self) -> String,
    last_error_str: RefCell<ErrEn>,
}

impl MyAddIn {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(None),
            some_prop: 0,
            send_to_socket: Self::send_to_socket_inner,
            get_resp: Self::get_resp_inner,
            socket_init: Self::socket_init_inner,
            target_ip: RefCell::new(None),
            target_port: RefCell::new(None),
            dll_handle: RefCell::new(None),
            send_fn: RefCell::new(None),
            last_err: Self::last_err_inner,
            last_error_str: RefCell::new(ErrEn::Other),
        }
    }

    fn set_err(&self, error: ErrEn) {
        *self.last_error_str.borrow_mut() = error;
    }

    fn last_err_inner(&self) -> String {
        match *self.last_error_str.borrow() {
            ErrEn::InvalidIP => "E1\r\n".to_string(),
            ErrEn::InvalidPort => "E2\r\n".to_string(),
            ErrEn::ComponentNoInit => "E3\r\n".to_string(),
            ErrEn::ConnectError => "E4\r\n".to_string(),
            ErrEn::ServerDropConnection => "E5\r\n".to_string(),
            ErrEn::JsonParseError => "E6\r\n".to_string(),
            ErrEn::DllError => "E7\r\n".to_string(),
            _ => "E0\r\n".to_string(),
        }
    }

    fn load_dll_and_fn(&self) -> Result<(), ()> {
        let mut handle = self.dll_handle.borrow_mut();
        let mut fn_ptr = self.send_fn.borrow_mut();

        if handle.is_none() {
            unsafe {
                // Фіксований шлях до DLL
                let path = s!("D:\\one_c\\socket_wrapper.dll");
                let h = LoadLibraryA(path);

                match h {
                    Ok(h) if !h.0.is_null() => *handle = Some(h),
                    _ => {
                        self.set_err(ErrEn::DllError);
                        return Err(());
                    }
                }
            }
        }

        if fn_ptr.is_none() {
            unsafe {
                let name = s!("SendToSocket");
                let proc = GetProcAddress(*handle.unwrap(), name);

                if let Some(proc) = proc {
                    *fn_ptr = Some(std::mem::transmute(proc));
                } else {
                    self.set_err(ErrEn::DllError);
                    return Err(());
                }
            }
        }

        Ok(())
    }

    fn send_to_socket_inner(&self, message: String) -> Result<String, ()> {
        // Парсимо JSON
        let v: Value = serde_json::from_str(&message).map_err(|_| {
            self.set_err(ErrEn::JsonParseError);
        })?;

        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let postfix = if method == "PingDevice" { "\0" } else { "" };
        let full_message = message + postfix;

        // Перевіряємо ініціалізацію
        let ip = self.target_ip.borrow().clone().ok_or_else(|| {
            self.set_err(ErrEn::ComponentNoInit);
        })?;

        let port = self.target_port.borrow().copied().ok_or_else(|| {
            self.set_err(ErrEn::ComponentNoInit);
        })?;

        // Завантажуємо DLL та функцію (один раз)
        self.load_dll_and_fn().map_err(|_| ())?;

        let send_fn = *self.send_fn.borrow().unwrap();

        unsafe {
            let c_ip = CString::new(ip).unwrap();
            let c_msg = CString::new(full_message).unwrap();

            let result = send_fn(
                c_ip.as_ptr(),
                port as c_int,
                c_msg.as_ptr(),
                c_msg.as_bytes().len() as c_int,
                2000, // timeout
            );

            match result.error_code {
                0 => {
                    let c_str = CStr::from_ptr(result.data.as_ptr());
                    Ok(c_str.to_string_lossy().into_owned())
                }
                103 => {
                    self.set_err(ErrEn::ConnectError);
                    Err(())
                }
                105 => {
                    self.set_err(ErrEn::ServerDropConnection);
                    Err(())
                }
                _ => {
                    self.set_err(ErrEn::Other);
                    Err(())
                }
            }
        }
    }

    fn get_resp_inner(&self, message: String) -> String {
        match self.send_to_socket_inner(message) {
            Ok(s) => s,
            Err(_) => "ERROR".to_string(),
        }
    }

    fn socket_init_inner(&self, addr: String, port: i32) -> Result<String, ()> {
        if port <= 0 || port > 65535 {
            self.set_err(ErrEn::InvalidPort);
            return Err(());
        }
        if addr.is_empty() {
            self.set_err(ErrEn::InvalidIP);
            return Err(());
        }

        *self.target_ip.borrow_mut() = Some(addr);
        *self.target_port.borrow_mut() = Some(port);

        Ok("Ok\r\n".to_string())
    }
}

impl Drop for MyAddIn {
    fn drop(&mut self) {
        if let Some(handle) = self.dll_handle.borrow_mut().take() {
            unsafe { let _ = FreeLibrary(handle); }
        }
    }
}