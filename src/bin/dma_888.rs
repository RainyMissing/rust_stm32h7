#![no_std]
#![no_main]

use embassy_executor::Spawner;
use cortex_m_rt::entry;
use defmt::*;
use embassy_executor::Executor;
use embassy_stm32::{bind_interrupts, peripherals, usart};
use embassy_stm32::usart::{Config, Uart};
use embassy_time::{Timer, Duration};
use static_cell::StaticCell; // Assuming this is the crate version provided by the user
use {defmt_rtt as _, panic_probe as _};

// 导入通道、互斥锁和 CriticalSectionRawMutex
use embassy_sync::channel::{Channel, Sender, Receiver};
// 修正：CriticalSectionRawMutex 的路径已更改
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex; // Import CriticalSectionRawMutex
use embassy_sync::mutex::Mutex; // Mutex struct is still in embassy_sync::mutex
use embassy_stm32::mode::Async; // Import Async mode for Uart

bind_interrupts!(struct Irqs {
    USART3 => usart::InterruptHandler<peripherals::USART3>;
});

// 定义一个 StaticCell 来管理 Mutex 的一次性初始化
// 修正：Mutex 需要 CriticalSectionRawMutex 作为第一个泛型参数
static RX_BUF_CELL: StaticCell<Mutex<CriticalSectionRawMutex, [u8; 64]>> = StaticCell::new();

// 定义一个通道，用于 main_task 通知 processing_task 有数据可用
// 通道发送 usize 类型（数据长度），容量为 1
// 修正：Channel 需要 CriticalSectionRawMutex 作为第一个泛型参数
static DATA_CHANNEL: StaticCell<Channel<CriticalSectionRawMutex, usize, 1>> = StaticCell::new();

// 函数 a：返回 "你好！"
fn a() -> &'static str {
    "你好！"
}

// 函数 b：返回 "您们好！"
fn b() -> &'static str {
    "您们好！"
}

#[embassy_executor::task]
async fn main_task(
    // 修正：Uart 类型只需要生命周期和模式泛型参数
    mut usart: Uart<'static, Async>,
    // 修正：直接接收 Mutex 的引用
    rx_buf_mutex: &'static Mutex<CriticalSectionRawMutex, [u8; 64]>,
    data_sender: Sender<'static, CriticalSectionRawMutex, usize, 1>, // 修正：Sender 泛型参数
) {
    info!("UART DMA echo server started");

    loop {
        // 获取 Mutex 锁，以便写入数据
        // .await 在这里是正确的用法，它会等待锁被释放
        let mut buf = rx_buf_mutex.lock().await;

        // 读取数据直到空闲
        // 注意：read_until_idle 会将数据直接写入我们提供的缓冲区
        // 修正：将 MutexGuard 转换为可变切片引用
        let n = match usart.read_until_idle(&mut *buf).await {
            Ok(n) => n,
            Err(e) => {
                error!("UART read error: {:?}", e);
                // 发生错误时释放锁并跳过本次循环
                drop(buf);
                continue;
            }
        };

        info!("Received {} bytes", n);

        // 如果接收到数据，通过通道发送数据长度给 processing_task
        if n > 0 {
            // 释放 Mutex 锁，让 processing_task 可以访问数据
            drop(buf);
            // 发送数据长度
            // send().await 返回 ()，不是 Result
            data_sender.send(n).await;
            info!("Sent {} bytes length to processing task", n);

        } else {
            // 如果没有接收到数据，释放 Mutex 锁
             drop(buf);
        }
    }
}

#[embassy_executor::task]
async fn processing_task(
    // 修正：直接接收 Mutex 的引用
    rx_buf_mutex: &'static Mutex<CriticalSectionRawMutex, [u8; 64]>,
    data_receiver: Receiver<'static, CriticalSectionRawMutex, usize, 1>, // 修正：Receiver 泛型参数
) {
    info!("Processing task started");

    loop {
        // 等待从通道接收数据
        let n = data_receiver.receive().await;



        // Now 'n' is guaranteed to be a usize if we reach this point
        info!("Processing {} bytes", n);

        // 获取 Mutex 锁，以便读取数据
        // .await 在这里是正确的用法
        let buf = rx_buf_mutex.lock().await;

        // 将接收到的数据（最多 n 个字节）尝试转换为 UTF-8 字符串
        if n > 0 {
            let received_data = &buf[..n];
            if let Ok(s) = core::str::from_utf8(received_data) {
                info!("Received string: {}", s);

                // 根据收到的数据判断调用哪个函数
                if s.contains("hello") { // 示例：如果包含 "hello"
                    let result = a();
                    info!("Called function a: {}", result);
                    // 这里可以将 result 通过 UART 发送回去，或者进行其他处理
                    // 如果需要在 processing_task 中发送，需要将 Uart 的 Tx 部分传递进来
                    // usart.write(result.as_bytes()).await.unwrap();
                } else { // 否则调用 b
                    let result = b();
                    info!("Called function b: {}", result);
                    // usart.write(result.as_bytes()).await.unwrap();
                }
            } else {
                info!("Received non-UTF-8 data");
                // 处理非 UTF-8 数据，例如打印十六进制
                info!("Received raw data: {:?}", received_data);
            }
        }

        // 释放 Mutex 锁
        drop(buf);
    }
}


#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let config = Config::default();
    // 注意：DMA 通道和引脚要与你的硬件匹配
    // Uart::new 返回 Uart<'d, Async> 当提供 DMA
    let usart = Uart::new(
        p.USART3,
        p.PD9, // USART3_TX
        p.PD8, // USART3_RX
        Irqs,
        p.DMA1_CH1, // TX DMA
        p.DMA1_CH2, // RX DMA
        config,
    ).unwrap();

    // 初始化共享缓冲区和通道
    // 修正：在这里初始化 RX_BUF_CELL，并且只初始化一次
    // 捕获 init 返回的 Mutex 引用
    let rx_buf_mutex_ref = unsafe { RX_BUF_CELL.init(Mutex::new([0u8; 64])) };
    let data_channel = unsafe { DATA_CHANNEL.init(Channel::new()) };

    // 不需要手动分割 Uart，因为 main_task 接收整个 Uart 实例
    // 如果 processing_task 需要发送，需要考虑其他方式传递 Tx 实例或发送请求

    // Spawn main_task 和 processing_task
    // 修正：将捕获到的 Mutex 引用传递给任务
    spawner.spawn(main_task(usart, rx_buf_mutex_ref, data_channel.sender())).unwrap();
    spawner.spawn(processing_task(rx_buf_mutex_ref, data_channel.receiver())).unwrap();
}
