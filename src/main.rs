#![no_std]
#![no_main]
#![feature(c_variadic)]
#![feature(const_mut_refs)]
#![feature(type_alias_impl_trait)]
#![feature(error_in_core)]

use embassy_executor::_export::StaticCell;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, IpListenEndpoint, Ipv4Address, Stack, StackResources};
use esp32c3_hal as hal;

use embassy_executor::Executor;
use embassy_time::{Duration, Timer};
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi};
use esp_backtrace as _;
use esp_println::logger::init_logger;
use esp_println::println;
use esp_wifi::initialize;
use esp_wifi::wifi::{WifiController, WifiDevice, WifiEvent, WifiState};
use hal::clock::{ClockControl, CpuClock};
use hal::Rng;
use hal::{embassy, peripherals::Peripherals, prelude::*, timer::TimerGroup, Rtc};

use hal::system::SystemExt;
use riscv_rt::entry;
use smoltcp::socket::tcp::State;

mod web;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

macro_rules! singleton {
    ($val:expr) => {{
        type T = impl Sized;
        static STATIC_CELL: StaticCell<T> = StaticCell::new();
        let (x,) = STATIC_CELL.init(($val,));
        x
    }};
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[entry]
fn main() -> ! {
    init_logger(log::LevelFilter::Info);
    esp_wifi::init_heap();

    let peripherals = Peripherals::take();

    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock160MHz).freeze();

    let mut rtc = Rtc::new(peripherals.RTC_CNTL);

    // Disable watchdog timers
    rtc.swd.disable();

    rtc.rwdt.disable();

    {
        use hal::systimer::SystemTimer;
        let syst = SystemTimer::new(peripherals.SYSTIMER);
        initialize(syst.alarm0, Rng::new(peripherals.RNG), &clocks).unwrap();
    }
    let (wifi_interface, controller) = esp_wifi::wifi::new();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0.timer0);

    let config = Config::Dhcp(Default::default());

    let seed = 1234; // very random, very secure seed

    // Init network stack
    let stack = &*singleton!(Stack::new(
        wifi_interface,
        config,
        singleton!(StackResources::<8>::new()),
        seed
    ));

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(connection(controller)).ok();
        spawner.spawn(net_task(&stack)).ok();
        spawner.spawn(task(1, &stack)).ok();
        spawner.spawn(task(2, &stack)).ok();
        spawner.spawn(task(3, &stack)).ok();
    });
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: PASSWORD.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice>) {
    stack.run().await
}

#[embassy_executor::task(pool_size = 4)]
async fn task(task_n: u32, stack: &'static Stack<WifiDevice>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        //Timer::after(Duration::from_millis(1_000)).await;

        println!("{} listening...", task_n);
        let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
        if let Err(e) = socket
            .accept(IpListenEndpoint {
                addr: None,
                port: 8080,
            })
            .await
        {
            println!("accept error: {:?}", e);
        }

        socket.set_timeout(Some(embassy_net::SmolDuration::from_secs(10)));

        if let Some(remote) = socket.remote_endpoint() {
            println!("Connect from {:?}", remote);
        }

        if let Err(e) = web::handle_connection(task_n, &mut socket).await {
            println!("web error {:?}", e)
        }

        socket.close();
        loop {
            match socket.state() {
                State::TimeWait | State::Closed => break,
                _ => Timer::after(Duration::from_millis(10)).await,
            }
        }
    }
}
