use clap::{App, Arg};
use reqwest::Proxy;
use rand::seq::SliceRandom;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::task;
use tokio::time::sleep;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

#[tokio::main(worker_threads = 8)]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up tracing for performance profiling
    let subscriber = Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
    console_subscriber::init();

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
    let mut proxies = read_proxies(proxy_file_path)?;
    
    // Shuffle proxies to distribute load
    proxies.shuffle(&mut rand::thread_rng());

    // End time based on the duration
    let end_time = Instant::now() + Duration::from_secs(duration);

    // Process proxies in chunks to optimize task management
    let chunk_size = 100;  // Adjust chunk size based on your system's capabilities
    for chunk in proxies.chunks(chunk_size) {
        let tasks: Vec<_> = chunk.iter().map(|proxy_url| {
            let url = url.to_string();
            let packet = packet.to_string();
            let end_time = end_time.clone();
            task::spawn(async move {
                // Batch sending requests until the duration ends
                while Instant::now() < end_time {
                    if let Err(e) = forward_request(&url, proxy_url, &packet).await {
                        eprintln!("Error using proxy {}: {}", proxy_url, e);
                    }
                    // Optionally, add a sleep here to rate limit requests
                    // sleep(Duration::from_millis(100)).await;
                }
            })
        }).collect();

        // Wait for all tasks in the chunk to complete
        futures::future::join_all(tasks).await;
    }

    Ok(())
}

async fn forward_request(url: &str, proxy_url: &str, packet: &str) -> Result<(), Box<dyn Error>> {
    let proxy = Proxy::http(proxy_url)?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .pool_max_idle_per_host(10)  // Increase max idle connections per host
        .pool_idle_timeout(Duration::from_secs(10))  // Adjust idle timeout
        .build()?;

    // Batch requests within a single task
    let mut tasks = vec![];
    for _ in 0..10 {  // Adjust the batch size as needed
        let client = client.clone();
        let url = url.to_string();
        let packet = packet.to_string();
        tasks.push(tokio::spawn(async move {
            let response = client.post(&url).body(packet).send().await;
            response
        }));
    }

    for task in tasks {
        if let Ok(Ok(response)) = task.await {
            println!("Response from {}: {:?}", proxy_url, response.status());
        }
    }

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
