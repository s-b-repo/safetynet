use clap::{App, Arg};
use reqwest::Proxy;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Command-line argument parsing using clap
    let matches = App::new("Proxy Forwarder")
        .version("1.0")
        .about("Forwards packets through proxies to a specified URL")
        .arg(Arg::new("url")
            .short('u')
            .long("url")
            .value_name("URL")
            .help("Sets the destination URL")
            .takes_value(true)
            .required(true))
        .arg(Arg::new("packet")
            .short('p')
            .long("packet")
            .value_name("PACKET")
            .help("Sets the packet data to send")
            .takes_value(true)
            .required(true))
        .arg(Arg::new("duration")
            .short('d')
            .long("duration")
            .value_name("DURATION")
            .help("Sets the duration to send packets (in seconds)")
            .takes_value(true)
            .required(true))
        .arg(Arg::new("proxies")
            .short('f')
            .long("file")
            .value_name("FILE")
            .help("Sets the path to the proxy list file")
            .takes_value(true)
            .required(true))
        .get_matches();

    // Get the command-line argument values
    let url = matches.value_of("url").unwrap();
    let packet = matches.value_of("packet").unwrap();
    let duration: u64 = matches.value_of("duration").unwrap().parse()?;
    let proxy_file_path = matches.value_of("proxies").unwrap();

    // Read proxies from the file
    let proxies = read_proxies(proxy_file_path)?;

    // End time based on the duration
    let end_time = Instant::now() + Duration::from_secs(duration);

    // Spawn tasks for each proxy
    let tasks: Vec<_> = proxies.into_iter().map(|proxy_url| {
        let url = url.to_string();
        let packet = packet.to_string();
        let end_time = end_time.clone();
        task::spawn(async move {
            while Instant::now() < end_time {
                if let Err(e) = forward_request(&url, &proxy_url, &packet).await {
                    eprintln!("Error using proxy {}: {}", proxy_url, e);
                }
            }
        })
    }).collect();

    // Wait for all tasks to complete
    futures::future::join_all(tasks).await;

    Ok(())
}

async fn forward_request(url: &str, proxy_url: &str, packet: &str) -> Result<(), Box<dyn Error>> {
    // Set up the proxy and create a client
    let proxy = Proxy::http(proxy_url)?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .build()?;

    // Send the packet as POST request
    let response = client.post(url).body(packet.to_string()).send().await?;

    // Print the response status (or handle it as needed)
    println!("Response from {}: {:?}", proxy_url, response.status());

    Ok(())
}

fn read_proxies(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    let reader = io::BufReader::new(file);

    // Read each line as a proxy and collect them into a vector
    let proxies = reader.lines()
        .filter_map(|line| line.ok()) // Ignore lines that can't be read
        .collect();

    Ok(proxies)
}
