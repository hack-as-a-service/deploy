#[macro_use]
extern crate lazy_static;

use std::{process::exit, time::Duration};

use bollard::{
    container::Config,
    container::CreateContainerOptions,
    image::CreateImageOptions,
    models::{EndpointSettings, HostConfig, RestartPolicy, RestartPolicyNameEnum},
    network::ConnectNetworkOptions,
    Docker,
};
use clap::{App, Arg};
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use termion::{color, style};
use tokio::{fs, time::sleep};

mod lock;

async fn pull_image(docker: &Docker, image: &str) -> Result<(), String> {
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(log) = stream.next().await {
        if let Err(err) = log {
            return Err(err.to_string());
        }
    }

    Ok(())
}

async fn start_container(docker: &Docker, name: &str, image: &str) -> Result<String, String> {
    let env = fs::read_to_string(format!("/home/deploy/.{}.env", name))
        .await
        .map(|v| v.lines().map(|e| e.to_owned()).collect::<Vec<String>>())
        .ok();

    if let Some(env) = &env {
        println!("Setting {} environment variables", env.len());
    }

    // Create the container
    let container = docker
        .create_container::<String, String>(
            Some(CreateContainerOptions {
                name: format!("{}_next", name),
            }),
            Config {
                image: Some(String::from(image)),
                env,
                host_config: Some(HostConfig {
                    restart_policy: Some(RestartPolicy {
                        name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    // Connect to HaaS admin network
    docker
        .connect_network(
            "haas_admin",
            ConnectNetworkOptions {
                container: &container.id,
                endpoint_config: EndpointSettings {
                    ..Default::default()
                },
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    // Start the container
    docker
        .start_container::<String>(&container.id, None)
        .await
        .map_err(|e| e.to_string())?;

    let container = docker
        .inspect_container(&container.id, None)
        .await
        .map_err(|e| e.to_string())?;

    let ip = container
        .network_settings
        .unwrap()
        .networks
        .unwrap()
        .get("haas_admin")
        .unwrap()
        .ip_address
        .to_owned()
        .unwrap();

    Ok(ip)
}

async fn update_proxy(name: &str, ip: &str, port: i32) -> Result<(), String> {
    let client = Client::new();

    let id = format!("{}_upstream", name);

    client
        .patch(format!("http://localhost:2019/id/{}", id))
        .json(&json!({ "@id": id, "dial": format!("{}:{}", ip, port) }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn run(name: &str, image: &str, port: i32) -> Result<(), String> {
    let docker =
        Docker::connect_with_local_defaults().expect("Error connecting to Docker - is it running?");

    println!(
        "Pulling image: {bold}{}{reset}\n",
        image,
        bold = style::Bold,
        reset = style::Reset
    );
    pull_image(&docker, image).await?;

    println!("Starting container...");
    let ip = start_container(&docker, name, image).await?;

    println!("New IP: {}\n", ip);

    // wait
    sleep(Duration::from_secs(5)).await;

    println!("Redirecting traffic to new deployment...");
    update_proxy(name, &ip, port).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    let matches = App::new("HaaS Deployer 9000")
        .author("Caleb Denio 🤺")
        .arg(
            Arg::with_name("name")
                .long("name")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("image")
                .long("image")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .default_value("3000"),
        )
        .get_matches();

    let image = matches.value_of("image").unwrap();
    let name = matches.value_of("name").unwrap();
    let port: i32 = matches.value_of("port").unwrap().parse().unwrap();

    if lock::is_locked(name) {
        println!("Deployment locked, waiting for release...");

        while lock::is_locked(name) {
            println!("still locked...");

            sleep(Duration::from_secs(5)).await;
        }
    }

    lock::lock(name);

    if let Err(err) = run(name, image, port).await {
        println!(
            "{red}{bold}Deployment failed{reset}: {}",
            err,
            red = color::Fg(color::Red),
            bold = style::Bold,
            reset = style::Reset
        );

        lock::unlock(name);

        exit(1);
    } else {
        println!(
            "{green}{bold}Deployment succeeded!{reset}",
            green = color::Fg(color::Green),
            bold = style::Bold,
            reset = style::Reset
        );

        lock::unlock(name);
    }
}
