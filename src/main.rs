use simple_dns::Packet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // Bind server socket to listen for DNS queries
    let server_socket = Arc::new(Mutex::new(
        UdpSocket::bind("157.245.97.9:53").await.unwrap(),
    ));

    // Bind target socket to communicate with Google DNS
    let target = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
    target.connect("8.8.8.8:53").await.unwrap();

    loop {
        println!("Listening");
        let mut buf = [0; 1024];

        let server_socket_clone = server_socket.clone();
        let target = target.clone();

        let result: Option<(usize, SocketAddr)> = async {
            let server_socket = server_socket_clone.lock().await;
            return match server_socket.recv_from(&mut buf).await {
                Ok(result) => Some(result),
                Err(e) => {
                    eprintln!("Error receiving query: {}", e);
                    return None;
                }
            };
        }.await;

        let (read_size, addr);
        if let Some(result) = result {
            read_size = result.0;
            addr = result.1;
        } else {
            return;
        }

        println!("Received query from client: {}", addr);

        let received = &mut buf[..read_size];

        // Parse the received DNS packet
        let packet = match Packet::parse(&received) {
            Ok(packet) => Arc::new(packet),
            Err(e) => {
                eprintln!("Error parsing packet: {}", e);
                return;
            }
        };
        println!("{:?}", packet.questions);
        println!("Packet ID: {}", packet.id());

        let received = received.to_owned();

        let target = target.clone();
        let _ = tokio::time::timeout(Duration::from_secs(5), async move {
            // Sending the query to Google DNS
            println!("Asking question to Google DNS");
            if let Err(e) = target.send(&received).await {
                eprintln!("Error sending query to Google DNS: {}", e);
                return;
            }

            // Receive the response from Google DNS
            let mut buf = [0; 1024];
            let read_size = match target.recv(&mut buf).await {
                Ok(size) => size,
                Err(e) => {
                    eprintln!("Error receiving response from Google DNS: {}", e);
                    return;
                }
            };
            println!("Received answer from Google DNS");

            // Parse the response from Google DNS
            let google_response = &buf[..read_size];
            let mut response_packet = match Packet::parse(&google_response) {
                Ok(parsed_packet) => parsed_packet,
                Err(e) => {
                    eprintln!("Error parsing response from Google DNS: {}", e);
                    return;
                }
            };

            // Create a response packet for the client
            response_packet.set_id(packet.id());

            // Serialize the response packet
            let mut response_buf = Vec::new();
            if let Err(e) = response_packet.write_to(&mut response_buf) {
                eprintln!("Error serializing response: {}", e);
                return;
            }

            let server_socket_clone = server_socket_clone.clone();
            let _ = async {
                let server_socket = server_socket_clone.lock().await;
                // Send the response to the client
                if let Err(e) = server_socket.send_to(&mut response_buf, &addr).await {
                    eprintln!("Error sending response to client: {}", e);
                }

                println!("Sending response to client: {}", addr);
            }.await;
        })
        .await;
    }
}
