use std::cell::RefCell;
use std::sync::Arc;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use std::net::IpAddr;
use serde_json::Value;

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

    fn send_request(&self, message: String, postfix: &str) -> Result<String, ()> {
        let mut stream = self.init_tcp_stream()?;
        let mut message = message;
        message.push_str(postfix);

        if let Err(_) = stream.write_all(message.as_bytes()) {
            return Err(self.set_err(ErrEn::Other));
        }

        let mut buffer = [0; 4096];
        let data =stream.read(&mut buffer);

        let _ = stream.flush();

        match data {
            Ok(size) if size > 0 => Ok(String::from_utf8_lossy(&buffer[0..size]).to_string()),
            Ok(_) => Err(self.set_err(ErrEn::ServerDropConnection)),
            Err(_) => Err(self.set_err(ErrEn::Other)),
        }
    }

    fn init_tcp_stream(&self) -> Result<TcpStream, ()> {
        let addr = match *self.target_addr.borrow() {
            Some(addr) => addr,
            None => return Err(self.set_err(ErrEn::ComponentNoInit)),
        };
        // Підключення з таймаутом
        let stream = match TcpStream::connect_timeout(
            &addr,
            Duration::from_secs(5),
        ) {
            Ok(s) => s,
            Err(_) => {
                return Err(self.set_err(ErrEn::ConnectError));
            }
        };
        // Таймаути на читання/запис
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        Ok(stream)
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