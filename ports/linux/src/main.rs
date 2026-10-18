//! ibus-engine-shanjie: the upstream core and the ported macOS shell behind IBus. `--ibus` when the daemon starts it
//! (the installed component XML); without it, a development run registers the component itself.
mod bus;
mod hub;
mod keys;
mod paths;
mod props;
mod serial;
mod tables;

use zbus::{connection, zvariant::OwnedValue};

const COMPONENT: &str = "org.freedesktop.IBus.Shanjie";
const ENGINE: &str = "shanjie";

/// The bus address: IBUS_ADDRESS when the daemon or the caller sets it, otherwise `ibus address`.
fn address() -> Result<String, Box<dyn std::error::Error>> {
    if let Ok(address) = std::env::var("IBUS_ADDRESS") {
        if !address.is_empty() {
            return Ok(address);
        }
    }
    let output = std::process::Command::new("ibus").arg("address").output()?;
    let address = String::from_utf8(output.stdout)?.trim().to_owned();
    if !output.status.success() || address.is_empty() || address == "(null)" {
        return Err("ibus-daemon is not running".into());
    }
    Ok(address)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ibus-engine-shanjie: {error:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launched_by_daemon = std::env::args().any(|a| a == "--ibus");
    futures_lite::future::block_on(async {
        let (async_tx, async_rx) = async_channel::unbounded();
        let hub = hub::start(async_tx);
        let address = address()?;
        let connection = connection::Builder::address(address.as_str())?
            .serve_at("/org/freedesktop/IBus/Factory", bus::Factory::new(ENGINE, hub))?
            .build()
            .await?;
        if !launched_by_daemon {
            let exe = std::env::current_exe()?.display().to_string();
            let component: OwnedValue = serial::Identity {
                component: COMPONENT,
                engine: ENGINE,
                longname: "善解",
                description: "善解注音輸入法",
                version: env!("CARGO_PKG_VERSION"),
                exec: &format!("{exe} --ibus"),
                symbol: "善",
                setup: "",
            }
            .component();
            connection
                .call_method(Some("org.freedesktop.IBus"), "/org/freedesktop/IBus", Some("org.freedesktop.IBus"), "RegisterComponent", &(component,))
                .await?;
        }
        connection.request_name(COMPONENT).await?;
        // Signals no D-Bus call waits for: rerank results, a destroyed engine's last effects.
        while let Ok(out) = async_rx.recv().await {
            bus::emit(&connection, out).await;
        }
        Ok(())
    })
}
