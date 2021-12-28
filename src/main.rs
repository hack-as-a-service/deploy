use std::process::exit;

use bollard::{image::CreateImageOptions, Docker};
use clap::{App, Arg};
use futures::StreamExt;
use termion::style;

#[tokio::main]
async fn main() {
    let docker =
        Docker::connect_with_local_defaults().expect("Error connecting to Docker - is it running?");

    let matches = App::new("HaaS Deployer 9000")
        .author("Caleb Denio 🤺")
        .arg(
            Arg::with_name("image")
                .long("image")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let image = matches.value_of("image").unwrap();

    println!(
        "Pulling image: {bold}{}{reset}\n",
        image,
        bold = style::Bold,
        reset = style::Reset
    );

    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            tag: "main",
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(log) = stream.next().await {
        match log {
            Ok(x) => {
                if let Some(progress) = x.progress {
                    println!(
                        "  {faint}|{reset} {}\t{bold}{}{reset}",
                        x.status.unwrap(),
                        progress,
                        faint = style::Faint,
                        bold = style::Bold,
                        reset = style::Reset
                    )
                } else {
                    println!(
                        "  {faint}|{reset} {}",
                        x.status.unwrap(),
                        faint = style::Faint,
                        reset = style::Reset
                    )
                }
            }
            Err(x) => {
                println!(
                    "  {faint}|{reset} ❌ {}",
                    x.to_string(),
                    faint = style::Faint,
                    reset = style::Reset
                );
                exit(1);
            }
        }
    }
}
