#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate clap;

use log::debug;

mod config;
mod gdb;
mod riscv;
mod server;
mod wishbone;

use clap::{App, Arg, Shell};
use config::Config;
use server::ServerKind;

use std::sync::Arc;

fn clap_app<'a, 'b>() -> App<'a, 'b> {
    App::new("Wishbone Tool")
        .version(crate_version!())
        .author("Sean Cross <sean@xobs.io>")
        .about("Work with Wishbone devices over various bridges")
        .arg(
            Arg::with_name("completion")
                .short("c")
                .long("completion")
                .help("Generate shell auto-completion file")
                .required_unless("list")
                .conflicts_with("list")
                .required_unless("address")
                .conflicts_with("address")
                .required_unless("server-kind")
                .conflicts_with("server-kind")
                .display_order(3)
                .possible_values(&Shell::variants())
                .takes_value(true)
        )
        .arg(
            Arg::with_name("pid")
                .short("p")
                .long("pid")
                .value_name("USB_PID")
                .help("USB PID to match")
                .default_value("0x5bf0")
                .display_order(3)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("vid")
                .short("v")
                .long("vid")
                .value_name("USB_VID")
                .help("USB VID to match")
                .display_order(3)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("bus")
                .short("B")
                .long("bus")
                .value_name("USB_BUS")
                .help("USB BUS to match")
                .display_order(4)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("device")
                .short("d")
                .long("device")
                .value_name("USB_DEVICE")
                .help("USB DEVICE to match")
                .display_order(4)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("serial")
                .short("u")
                .long("serial")
                .alias("uart")
                .value_name("PORT")
                .help("Serial port to use")
                .display_order(4)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("baud")
                .short("b")
                .long("baud")
                .value_name("RATE")
                .default_value("115200")
                .help("Baudrate to use in serial mode")
                .display_order(5)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ethernet-host")
                .long("ethernet-host")
                .value_name("HOSTNAME")
                .help("Address to use when connecting via Etherbone")
                .display_order(6)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("ethernet-port")
                .long("ethernet-port")
                .value_name("PORT")
                .help("Port to use when connecting via Etherbone")
                .default_value("1234")
                .display_order(6)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("ethernet-tcp")
                .long("ethernet-tcp")
                .help("Connect using TCP, for example when using an external wishbone bridge")
                .display_order(6)
        )
        .arg(
            Arg::with_name("pcie-bar")
                .long("pcie-bar")
                .help("Connect using PCIe using the specified BAR")
                .display_order(6)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("spi-pins")
                .short("g")
                .long("spi-pins")
                .value_delimiter("PINS")
                .help("GPIO pins to use for COPI,CIPO,CLK,CS_N (e.g. 2,3,4,18)")
                .display_order(6)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("address")
                .index(1)
                .required_unless("completion")
                .conflicts_with("completion")
                .required_unless("server-kind")
                .conflicts_with("server-kind")
                .required_unless("list")
                .conflicts_with("list")
                .display_order(7)
                .help("address to read/write"),
        )
        .arg(
            Arg::with_name("value")
                .value_name("value")
                .index(2)
                .required(false)
                .display_order(8)
                .help("value to write"),
        )
        .arg(
            Arg::with_name("bind-addr")
                .short("a")
                .long("bind-addr")
                .value_name("IP_ADDRESS")
                .help("IP address to bind to")
                .default_value("127.0.0.1")
                .display_order(2)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("wishbone-port")
                .short("n")
                .long("wishbone-port")
                .alias("port")
                .value_name("PORT_NUMBER")
                .help("port number to listen on")
                .default_value("1234")
                .display_order(2)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("server-kind")
                .short("s")
                .long("server")
                .alias("server-kind")
                .takes_value(true)
                .multiple(true)
                .required_unless("completion")
                .conflicts_with("completion")
                .required_unless("address")
                .conflicts_with("address")
                .required_unless("list")
                .conflicts_with("list")
                .help("which server to run (if any)")
                .display_order(1)
                .possible_values(&["gdb", "wishbone", "random-test", "load-file", "terminal", "messible"]),
        )
        .arg(
            Arg::with_name("gdb-port")
                .long("gdb-port")
                .help("Port to listen for GDB connections")
                .default_value("3333")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("load-name")
                .long("load-name")
                .help("A file to load into RAM")
                .takes_value(true)
                .display_order(13),
        )
        .arg(
            Arg::with_name("load-address")
                .long("load-address")
                .help("Address for file to load")
                .takes_value(true)
                .display_order(13),
        )
        .arg(
            Arg::with_name("random-loops")
                .long("random-loops")
                .help("number of loops to run when doing a random-test")
                .display_order(9)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("random-range")
                .long("random-range")
                .help("the size of the random address range (i.e. how many bytes to randomly add to the address)")
                .display_order(9)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("messible-address")
                .long("messible-address")
                .help("address to use to get messible messages from")
                .display_order(9)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("csr-csv")
                .long("csr-csv")
                .help("csr.csv file containing register mappings")
                .display_order(9)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("register-offset")
                .long("register-offset")
                .alias("csr-csv-offset")
                .help("apply an offset to addresses, e.g. to specify PCIe BAR offset")
                .display_order(9)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("random-address")
                .long("random-address")
                .help("address to write to when doing a random-test")
                .display_order(10)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("debug-offset")
                .long("debug-offset")
                .help("address to use for debug bridge")
                .default_value("0xf00f0000")
                .display_order(11)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("terminal-mouse")
                .long("terminal-mouse")
                .help("capture mouse events for use with terminal server")
                .takes_value(false)
        )
}

fn main() -> Result<(), String> {
    flexi_logger::Logger::with_env_or_str("wishbone_tool=info")
        .format_for_stderr(|write, now, record| {
            flexi_logger::colored_default_format(write, now, record)?;
            write!(write, "\r")
        })
        .start()
        .unwrap();

    let matches = clap_app().get_matches();

    // If they specify a "--completion", print it to stdout and exit without error.
    if let Some(shell_str) = matches.value_of("completion") {
        use std::io;
        use std::str::FromStr;
        // Unwrap is safe since `get_matches()` validated it above
        let shell = Shell::from_str(shell_str).unwrap();
        clap_app().gen_completions_to(crate_name!(), shell, &mut io::stdout());
        return Ok(());
    }

    let (cfg, bridge) = Config::parse(matches).map_err(|e| match e {
        config::ConfigError::NumberParseError(num, e) => {
            format!("unable to parse the number \"{}\": {}", num, e)
        }
        config::ConfigError::NoOperationSpecified => format!("no operation was specified"),
        config::ConfigError::UnknownServerKind(s) => format!("unknown server '{}', see --help", s),
        config::ConfigError::SpiParseError(s) => format!("couldn't parse spi pins: {}", s),
        config::ConfigError::IoError(s) => format!("file error: {}", s),
        config::ConfigError::InvalidConfig(s) => format!("invalid configuration: {}", s),
        config::ConfigError::AddressOutOfRange(s) => {
            format!("address was not in mappable range: {}", s)
        }
    })?;

    bridge.connect().map_err(|e| format!("unable to connect to bridge: {}", e))?;

    let cfg = Arc::new(cfg);
    let mut threads = vec![];
    for server_kind in cfg.server_kind.iter() {
        use std::thread;
        let bridge = bridge.clone();
        let cfg = cfg.clone();
        let server_kind = *server_kind;
        let thr_handle = thread::spawn(move || {
            match server_kind {
                ServerKind::GDB => server::gdb_server(&cfg, bridge),
                ServerKind::Wishbone => server::wishbone_server(&cfg, bridge),
                ServerKind::RandomTest => server::random_test(&cfg, bridge),
                ServerKind::LoadFile => server::load_file(&cfg, bridge),
                ServerKind::Terminal => server::terminal_client(&cfg, bridge),
                ServerKind::MemoryAccess => server::memory_access(&cfg, bridge),
                ServerKind::Messible => server::messible_client(&cfg, bridge),
            }
            .expect("couldn't start server");
            debug!("Exited {:?} thread", server_kind);
        });
        threads.push(thr_handle);
    }
    for handle in threads {
        handle.join().ok();
    }

    Ok(())
}
