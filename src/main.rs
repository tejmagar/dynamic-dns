use simple_dns::{Packet};
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
    // Bind server socket to listen for DNS queries
    let server_socket = UdpSocket::bind("127.0.0.1:53").await.unwrap();

    // Bind target socket to communicate with Google DNS
    let target = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    target.connect("8.8.8.8:53").await.unwrap();

    loop {
        let mut buf = [0; 1024];
        let (read_size, addr) = match server_socket.recv_from(&mut buf).await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error receiving query: {}", e);
                continue;
            }
        };
        println!("Received query from client: {}", addr);

        let received = &mut buf[..read_size];

        // Parse the received DNS packet
        let packet = match Packet::parse(&received) {
            Ok(packet) => packet,
            Err(e) => {
                eprintln!("Error parsing packet: {}", e);
                continue;
            }
        };
        println!("{:?}", packet.questions);
        println!("Packet ID: {}", packet.id());

        // Sending the query to Google DNS
        println!("Asking question to Google DNS");
        if let Err(e) = target.send(&received).await {
            eprintln!("Error sending query to Google DNS: {}", e);
            continue;
        }

        // Receive the response from Google DNS
        let mut buf = [0; 1204];
        let read_size = match target.recv(&mut buf).await {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Error receiving response from Google DNS: {}", e);
                continue;
            }
        };
        println!("Received answer from Google DNS");

        // Parse the response from Google DNS
        let google_response = &buf[..read_size];
        let mut response_packet = match Packet::parse(&google_response) {
            Ok(parsed_packet) => parsed_packet,
            Err(e) => {
                eprintln!("Error parsing response from Google DNS: {}", e);
                continue;
            }
        };

        // Create a response packet for the client
        response_packet.set_id(packet.id());

        // Serialize the response packet
        let mut response_buf = Vec::new();
        if let Err(e) = response_packet.write_to(&mut response_buf) {
            eprintln!("Error serializing response: {}", e);
            continue;
        }

        // Send the response to the client
        if let Err(e) = server_socket.send_to(&mut response_buf, &addr).await {
            eprintln!("Error sending response to client: {}", e);
            continue;
        }

        println!("Sending response to client: {}", addr);
    }
}
