use std::cell::RefCell;
use std::sync::Arc;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use std::net::IpAddr;
use serde_json::Value;
// use winapi::um::winsock2::{SOCKET, setsockopt, SOL_SOCKET, SO_LINGER, LINGER, closesocket, SD_BOTH, send, recv, shutdown};
use winapi::um::winsock2::{send, recv, shutdown, SD_BOTH, closesocket, setsockopt, SOL_SOCKET, SO_LINGER, LINGER, ioctlsocket, FIONBIO, SOCKET};
use winapi::shared::ws2def::{AF_INET, IPPROTO_TCP, SOCKADDR_IN}; // sockaddr_in часто реекспортується як SOCKADDR_IN

use native_api_1c::{
    native_api_1c_core::ffi::connection::Connection,
    native_api_1c_macro::AddIn,
};

enum ErrEn {
    InvalidIP,
    InvalidPort,
    ComponentNoInit,
    ConnectError,
    ServerDropConnection,
    JsonParseError,
    Other
}

#[derive(AddIn)]
pub struct MyAddIn {
    #[add_in_con]
    connection: Arc<Option<&'static Connection>>,

    #[add_in_prop(name = "MyProp", name_ru = "МоеСвойство", readable, writable)]
    pub some_prop: i32,

    #[add_in_prop(name = "ProtectedProp", name_ru = "ЗащищенноеСвойство", readable)]
    pub protected_prop: i32,

    // Варіант з виключеннями !!!!!
    #[add_in_func(name = "SendToSocket", name_ru = "ОтправитьВСокет")]
    #[arg(Str)]                     // аргумент - рядок (String)
    #[returns(Str, result)]         // повертає Result<String, ()> → відповідь сервера або помилку
    pub send_to_socket: fn(&Self, String) -> Result<String, ()>,


    // Варіант без виключень !!!!!
    #[add_in_func(name = "GetResponse", name_ru = "ПолучитьОтвет")]
    #[arg(Str)]                     // аргумент - рядок (String)
    #[returns(Str)]         // повертає Result<String, ()> → відповідь сервера або помилку
    pub get_resp: fn(&Self, String) -> String,

    #[add_in_func(name = "InitSocket", name_ru = "ИнициализироватьСокет")]
    #[arg(Str)]
    #[arg(Int)]
    #[returns(Str, result)]
    pub socket_init: fn(&Self, String, i32) -> Result<String, ()>,

    #[add_in_func(name = "SocketIsInit", name_ru = "СокетИнициализирован")]
    #[returns(Bool)]
    pub socket_is_init: fn(&Self) -> bool,

    target_addr: RefCell<Option<SocketAddr>>,

    #[add_in_func(name = "LastErr", name_ru = "ПоследняяОшибка")]
    #[returns(Str)]
    pub last_err: fn(&Self) -> String,
    last_error_str: RefCell<ErrEn>,

    #[add_in_func(name = "TestConnect", name_ru = "ПопыткаСоединения")]
    #[returns(Str, result)]
    pub test_connect: fn(&Self) -> Result<String, ()>,

}

impl MyAddIn {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(None),
            some_prop: 0,
            protected_prop: 50,
            send_to_socket: Self::send_to_socket_inner,
            socket_init: Self::socket_init_inner,
            target_addr: RefCell::new(None),
            socket_is_init: Self::socket_is_init_inner,
            last_err: Self::last_err_inner,
            last_error_str: RefCell::new(ErrEn::Other),
            test_connect: Self::test_connect_inner,
            get_resp: Self::get_resp_inner,
        }
    }

    fn test_connect_inner(&self) -> Result<String, ()> {
        let addr = match *self.target_addr.borrow() {
            Some(addr) => addr,
            None => return Err(self.set_err(ErrEn::ComponentNoInit)),
        };

        match TcpStream::connect_timeout(
            &addr,
            Duration::from_secs(5),
        ) {
            Ok(_) => Ok("Ok".to_string()),
            Err(_) => Err(self.set_err(ErrEn::ConnectError))
        }
    }

    fn last_err_inner(&self) -> String {
        match *self.last_error_str.borrow() {
            ErrEn::InvalidIP => "E1\r\n".to_string(),
            ErrEn::InvalidPort => "E2\r\n".to_string(),
            ErrEn::ComponentNoInit => "E3\r\n".to_string(),
            ErrEn::ConnectError => "E4\r\n".to_string(),
            ErrEn::ServerDropConnection => "E5\r\n".to_string(),
            _ => "E0\r\n".to_string(),
        }
    }

    fn set_err(&self, error: ErrEn) {
        *self.last_error_str.borrow_mut() =  error;
    }

    // Варіант з виключеннями
    fn send_to_socket_inner(&self, message: String) -> Result<String, ()> {
        let js_dict = serde_json::from_str::<Value>(&message).unwrap_or_default(); // unwrap може положить викликаючий код. дякую, бл*дь за ах*єнний інструмент, су*и!!!!

        if js_dict.as_object().map(|m| m.is_empty()).unwrap_or(true) { // в рот їб*в я цю ссану мову, су*а
            return Err(self.set_err(ErrEn::JsonParseError));
        }

        let method_str = js_dict.get("method")
            .and_then(|method| method.as_str())
            .unwrap_or("");

        let mut new_data = match method_str {
            "sibling" => r#"{"method":"sibling", "result": "ОК"}"#.to_string(),

            "PingDevice" => self.send_request(message, "\0")?,

            _ =>  self.send_request(message, "")?,
        };

        new_data.push_str("\r\n"); // TODO <------ Можливо це не потрібно
        Ok(new_data)
    }

    // Варіант без виключень
    fn get_resp_inner(&self, message: String) -> String {
        let js_dict = serde_json::from_str::<Value>(&message).unwrap_or_default(); // unwrap може положить викликаючий код. дякую, бл*дь за ах*єнний інструмент, су*и!!!!

        if js_dict.as_object().map(|m| m.is_empty()).unwrap_or(true) { // в рот їб*в я цю ссану мову, су*а
            // return r#"{"method":"sibling", "result": "ОК"}"#.to_string()
            return "Json parse error".to_string();
        }

        let method_str = js_dict.get("method")
            .and_then(|method| method.as_str())
            .unwrap_or("");

        let mut new_data = match method_str {
            "sibling" => r#"{"method":"sibling", "result": "ОК"}"#.to_string(),

            "PingDevice" => self.send_request(message, "\0")
                .unwrap_or(r#"{"method":"sibling", "result":"ERROR"}"#.to_string()),

            _ =>  self.send_request(message, "")
                .unwrap_or(r#"{"method":"sibling", "result":"ERROR"}"#.to_string()),
        };

        new_data.push_str("\r\n"); // TODO <------ Можливо це не потрібно
        new_data
    }

//     fn send_request(&self, message: String, postfix: &str) -> Result<String, ()> {
//         let mut wrapper = self.init_tcp_stream()?;
//         let mut message = message;
//         message.push_str(postfix);
//         // return Ok("sdfsdfsdfsdf".to_string()); // <-- якщо вийти тут, після створення, але до запису в буфер, то сокет закриється!
//         // Це наш новий метод write через winapi send
//         if let Err(_) = wrapper.write(message) {
//             return Err(self.set_err(ErrEn::Other));
//         }
//
//         let mut buffer = [0; 4096];
//         // Це наш новий метод read через winapi recv
//         let data = wrapper.read(&mut buffer);
//
//         // ВІДДАЄМО СОКЕТ У ФОНОВИЙ ПОТІК
//         // move переносить володіння wrapper у замикання
//         // от чисто за для спроби, не працює!!!!
//         std::thread::spawn(move || {
//             // Тут потік володіє сокетом. Викликаємо Drop явно або просто чекаємо
//             // Явний виклик drop(wrapper) спрацює негайно, але ми додамо "життя"
//             // 1. Жорстко рвемо
//             drop(wrapper);
//             // 2. Потік живе ще трохи, імітуючи активність
//             for _ in 0..50 {
//                 std::thread::sleep(std::time::Duration::from_millis(5));
//             }
//             // Тут потік остаточно вмирає
//         });
//
//         match data {
//             Ok(size) if size > 0 => {
//                 let mut res = String::from_utf8_lossy(&buffer[0..size]).to_string();
//                 res.push_str("\r\n");
//                 Ok(res)
//             },
//             _ => Err(self.set_err(ErrEn::ServerDropConnection)),
//         }
//     }

    fn send_request(&self, message: String, postfix: &str) -> Result<String, ()> {
        let mut wrapper = self.init_tcp_stream()?;
        let mut message = message;
        message.push_str(postfix);

        if let Err(_) = wrapper.write(message) {
            return Err(self.set_err(ErrEn::Other));
        }

        let mut buffer = [0; 4096];
        let data = wrapper.read(&mut buffer);

        let result = match data {
            Ok(size) if size > 0 => {
                let mut res = String::from_utf8_lossy(&buffer[0..size]).to_string();
                res.push_str("\r\n");
                Ok(res)
            },
            _ => Err(self.set_err(ErrEn::ServerDropConnection)),
        };

        // Забираємо handle і НЕ викликаємо Drop автоматично
        let handle = wrapper.handle;
        std::mem::forget(wrapper); // <- Це критично!

        // Закриваємо в окремому потоці з затримкою
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300)); // Збільш затримку
            unsafe {
                shutdown(handle, SD_BOTH);
                std::thread::sleep(std::time::Duration::from_millis(100));
                closesocket(handle);
            }
        });

        result
    }

    fn init_tcp_stream(&self) -> Result<TcpStreamWrapper, ()> {
        let addr = match *self.target_addr.borrow() {
            Some(addr) => addr,
            None => return Err(self.set_err(ErrEn::ComponentNoInit)),
        };
        Ok(TcpStreamWrapper::new(&addr))
    }

    fn socket_init_inner(&self, addr: String, port: i32) -> Result<String, ()> {
        if addr.parse::<IpAddr>().is_err() {
            return Err(self.set_err(ErrEn::InvalidIP));
        }
        if port < 0 || port > 65535 {
            return Err(self.set_err(ErrEn::InvalidPort));
        }
        let parsed_addr = format!("{}:{}", addr, port).as_str().parse::<SocketAddr>();

        match parsed_addr {
            Ok(res) => *self.target_addr.borrow_mut() = Some(res),
            Err(_) => return Err(self.set_err(ErrEn::InvalidIP)),
        }
        Ok("Ok\r\n".to_string())
    }

    fn socket_is_init_inner(&self) -> bool {
        match *self.target_addr.borrow(){
            Some(_) => true,
            None => false,
        }
    }
}

struct TcpStreamWrapper {
    handle: SOCKET,
}

unsafe impl Send for TcpStreamWrapper {}

impl TcpStreamWrapper {
    fn new(sock_addr: &SocketAddr) -> Self {
        unsafe {
            // 1. Створюємо сокет (AF_INET = 2, SOCK_STREAM = 1, IPPROTO_TCP = 6)
            let handle = winapi::um::winsock2::socket(
                winapi::shared::ws2def::AF_INET,
                winapi::um::winsock2::SOCK_STREAM,
                6 // IPPROTO_TCP
            );
            let reuse: i32 = 1;
            setsockopt(
                handle,
                SOL_SOCKET,
                winapi::um::winsock2::SO_REUSEADDR,
                &reuse as *const _ as *const i8,
                std::mem::size_of::<i32>() as i32
            );

            if handle == winapi::um::winsock2::INVALID_SOCKET {
                panic!("Socket creation failed: {}", std::io::Error::last_os_error());
            }

            // 2. Налаштування адреси
            let mut addr: winapi::shared::ws2def::SOCKADDR_IN = std::mem::zeroed();
            addr.sin_family = winapi::shared::ws2def::AF_INET as u16;
            addr.sin_port = winapi::um::winsock2::htons(sock_addr.port());

            if let IpAddr::V4(v4) = sock_addr.ip() {
                let octets = v4.octets();
                std::ptr::copy_nonoverlapping(
                    octets.as_ptr(),
                    &mut addr.sin_addr as *mut _ as *mut u8,
                    4
                );
            } else {
                winapi::um::winsock2::closesocket(handle);
                panic!("Only IPv4 supported");
            }

            // 3. Коннект
            // Використовуємо SOCKADDR (великими літерами) з shared::ws2def
            let res = winapi::um::winsock2::connect(
                handle,
                &addr as *const _ as *const winapi::shared::ws2def::SOCKADDR,
                std::mem::size_of::<winapi::shared::ws2def::SOCKADDR_IN>() as i32
            );

            if res != 0 {
                let err = std::io::Error::last_os_error();
                winapi::um::winsock2::closesocket(handle);
                panic!("Connect failed: {}", err);
            }

            // 4. Таймаути (1 секунда)
            let timeout = 1000i32;
            winapi::um::winsock2::setsockopt(
                handle,
                winapi::um::winsock2::SOL_SOCKET,
                winapi::um::winsock2::SO_RCVTIMEO,
                &timeout as *const _ as *const i8,
                std::mem::size_of::<i32>() as i32
            );

            Self { handle }
        }
    }
}
impl TcpStreamWrapper {
    // Запис через чистий WinAPI send (як у C++)
    //fn write(&mut self, data: String) -> std::io::Result<usize> {
    //    let buf = data.as_bytes();
    //    let res = unsafe {
    //        // send повертає кількість відправлених байт або -1 при помилці
    //        send(
    //            self.handle,
    //            buf.as_ptr() as *const i8,
    //            buf.len() as i32,
    //            0
    //        )
    //    };
//
    //    if res < 0 {
    //        Err(std::io::Error::last_os_error())
    //    } else {
    //        Ok(res as usize)
    //    }
    //}
//
    fn write(&mut self, data: String) -> std::io::Result<usize> {
        let buf = data.as_bytes();
        let res = unsafe {
            send(self.handle, buf.as_ptr() as *const i8, buf.len() as i32, 0)
        };

        if res < 0 {
            return Err(std::io::Error::last_os_error());
        }

        // ДОДАЙ ЦЕ: спробуй прочитати хоч щось, щоб "розбудити" сокет
        let mut dummy = [0u8; 1];
        let _ = unsafe {
            recv(self.handle, dummy.as_mut_ptr() as *mut i8, 1, 0)
        };

        Ok(res as usize)
    }
    // Читання через чистий WinAPI recv
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let res = unsafe {
            // recv повертає кількість отриманих байт або -1
            recv(
                self.handle,
                buf.as_mut_ptr() as *mut i8,
                buf.len() as i32,
                0
            )
        };

        if res < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }
}

// impl Drop for TcpStreamWrapper {
//     fn drop(&mut self) {
//         unsafe {
//             // 1. Примусовий неблокуючий режим (FIONBIO)
//             // Це збиває всі внутрішні очікування Windows на цьому сокеті
//             let mut mode: u32 = 1;
//             ioctlsocket(self.handle, FIONBIO, &mut mode);
//
//             // 2. Жорсткий Linger (0 секунд)
//             // Посилає RST пакет замість ввічливого FIN. Це "вбивство" з'єднання.
//             let linger_opt = LINGER {
//                 l_onoff: 1,
//                 l_linger: 0
                    //             };
//             setsockopt(
//                 self.handle,
//                 SOL_SOCKET,
//                 SO_LINGER,
//                 &linger_opt as *const _ as *const i8,
//                 std::mem::size_of::<LINGER>() as i32
    //             );
//
//             // 3. Shutdown SD_BOTH (2)
//             // Офіційно кажемо стеку TCP, що ми закінчили і читати, і писати
//             shutdown(self.handle, SD_BOTH);
//
//             // 4. Фінальний удар
//             closesocket(self.handle);
//         }
            //     }
// }

// impl Drop for TcpStreamWrapper {
//     fn drop(&mut self) {
//         unsafe {
//             // Створюємо event
//             let event = winapi::um::synchapi::CreateEventW(
//                 std::ptr::null_mut(),
//                 1, // manual reset
//                 0, // не сигнальний
//                 std::ptr::null()
//             );
// 
//             // Реєструємо подію закриття
//             winapi::um::winsock2::WSAEventSelect(
//                 self.handle,
//                 event,
//                 winapi::um::winsock2::FD_CLOSE
//             );
// 
//             // Shutdown
//             shutdown(self.handle, SD_BOTH);
// 
//             // Чекаємо подію (max 100ms)
//             winapi::um::synchapi::WaitForSingleObject(event, 100);
// 
//             // Закриваємо
//             closesocket(self.handle);
//             winapi::um::handleapi::CloseHandle(event);
//         }
//     }
// }