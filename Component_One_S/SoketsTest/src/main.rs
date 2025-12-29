use std::net::{SocketAddr, IpAddr};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// WinAPI імпорты
use winapi::um::winsock2::{
    send, recv, shutdown, SD_BOTH, closesocket, setsockopt, 
    SOL_SOCKET, SO_LINGER, LINGER, SO_RCVTIMEO, SO_SNDTIMEO, 
    SOCKET, socket, connect, SOCK_STREAM, INVALID_SOCKET, htons
};
use winapi::shared::ws2def::{AF_INET, SOCKADDR_IN, SOCKADDR};

fn run_single_request(id: i32) -> Result<String, String> {
    let addr_str = "127.0.0.1:2200";
    let addr: SocketAddr = addr_str.parse().unwrap();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        unsafe {
            let sock = socket(AF_INET, SOCK_STREAM, 6);
            if sock == INVALID_SOCKET {
                let _ = tx.send(Err("Помилка створення сокета".to_string()));
                return;
            }

            // Таймаути
            let t_val = 1000i32; 
            setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &t_val as *const _ as *const i8, 4);
            setsockopt(sock, SOL_SOCKET, SO_SNDTIMEO, &t_val as *const _ as *const i8, 4);

            let mut addr_in: SOCKADDR_IN = std::mem::zeroed();
            addr_in.sin_family = AF_INET as u16;
            addr_in.sin_port = htons(addr.port());
            if let IpAddr::V4(v4) = addr.ip() {
                std::ptr::copy_nonoverlapping(v4.octets().as_ptr(), &mut addr_in.sin_addr as *mut _ as *mut u8, 4);
            }

            println!("[{}] Підключення до {}...", id, addr_str);
            if connect(sock, &addr_in as *const _ as *const SOCKADDR, std::mem::size_of::<SOCKADDR_IN>() as i32) != 0 {
                closesocket(sock);
                let _ = tx.send(Err("Connect failed".to_string()));
                return;
            }

            let msg = format!("Hello from Rust thread iteration {}", id);
            send(sock, msg.as_ptr() as *const i8, msg.len() as i32, 0);

            let mut buffer = [0u8; 1024];
            let n = recv(sock, buffer.as_mut_ptr() as *mut i8, buffer.len() as i32, 0);

            let response = if n > 0 {
                Ok(String::from_utf8_lossy(&buffer[0..n as usize]).to_string())
            } else {
                Err("No response".to_string())
            };

            // ЖОРСТКЕ ЗАКРИТТЯ
            let linger_opt = LINGER { l_onoff: 1, l_linger: 0 };
            setsockopt(sock, SOL_SOCKET, SO_LINGER, &linger_opt as *const _ as *const i8, 8);
            shutdown(sock, SD_BOTH);
            closesocket(sock);
            
            println!("[{}] Сокет закрито дескриптор: {}", id, sock);
            let _ = tx.send(response);
        }
    });

    rx.recv_timeout(Duration::from_secs(2)).unwrap_or(Err("Timeout".to_string()))
}

fn main() {
    unsafe {
        // Ініціалізація Winsock (v2.2)
        let mut data: winapi::um::winsock2::WSADATA = std::mem::zeroed();
        let res = winapi::um::winsock2::WSAStartup(0x202, &mut data);
        if res != 0 {
            println!("WSAStartup failed with error: {}", res);
            return;
        }
    }

    println!("Winsock ініціалізовано. Починаємо тест...");

    for i in 1..=10 {
        match run_single_request(i) {
            Ok(resp) => println!("Спроба {}: Отримано: {}", i, resp),
            Err(e) => println!("Спроба {}: Помилка: {}", i, e),
        }
        thread::sleep(Duration::from_millis(500));
    }

    unsafe { winapi::um::winsock2::WSACleanup(); }
    println!("Тест завершено. Натисніть Enter...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
}