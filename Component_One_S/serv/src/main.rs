use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn handle_client(mut stream: TcpStream) {
    // Буфер для читання даних (1024 байти)
    let mut buffer = [0; 1024];

    println!("Нове підключення: {}", stream.peer_addr().unwrap());

    // Читаємо дані з сокета
    match stream.read(&mut buffer) {
        Ok(size) if size > 0 => {
            println!("Отримано {} байт, повертаю назад...", size);

            // Відправляємо отримані дані назад трансмітеру
            if let Err(e) = stream.write_all(&buffer[0..size]) {
                eprintln!("Помилка при відправці: {}", e);
            }
        }
        Ok(_) => println!("Клієнт закрив з'єднання"),
        Err(e) => eprintln!("Помилка читання: {}", e),
    }
}

fn main() {
    let addr = "127.0.0.1:2200";
    let listener = TcpListener::bind(addr).expect("Не вдалося прив'язати сокет");

    println!("Сервер слухає на {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Створюємо новий потік для кожного підключення
                thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => eprintln!("Помилка вхідного з'єднання: {}", e),
        }
    }
}