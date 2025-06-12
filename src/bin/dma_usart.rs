#![no_std]
#![no_main]
use embassy_executor::Spawner;
use cortex_m_rt::entry;
use defmt::*;
use embassy_executor::Executor;
use embassy_stm32::{bind_interrupts, peripherals, usart};
use embassy_stm32::usart::{Config, Uart};
use embassy_time::{Timer, Duration};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USART3 => usart::InterruptHandler<peripherals::USART3>;
});

#[embassy_executor::task]
async fn main_task() {
    let p = embassy_stm32::init(Default::default());

    let config = Config::default();
    // 注意：DMA 通道和引脚要与你的硬件匹配
    let mut usart = Uart::new(
        p.USART3,
        p.PD9,  // USART3_TX
        p.PD8,  // USART3_RX
        Irqs,
        p.DMA1_CH1, // TX DMA
        p.DMA1_CH2, // RX DMA
        config,
    ).unwrap();

    info!("UART DMA echo server started");

    loop    
    {
        let mut buf = [0u8; 64];
        let mut idx = 0;

        let mut buf = [0u8; 64];
        let n = usart.read_until_idle(&mut buf).await.unwrap();
        info!("Received {} bytes: {:?}", n, &buf[..n]);

        // 将buf[..n]转化为字符串
        if n > 0{
        if let Ok(s) = core::str::from_utf8(&buf[..n]) {
            info!("Received: {}", s);
            usart.write(b"Echo: ").await.unwrap();
            usart.write(s.as_bytes()).await.unwrap();
            usart.write(b"\r\n").await.unwrap();
        } else {
            info!("Received invalid UTF-8 data");
        }
    }
    }

}

#[embassy_executor::main]
async fn main(spawner: Spawner) {

    spawner.spawn(main_task()).unwrap();
}