#![no_std]
#![no_main]

extern crate alloc;
use alloc::string::String;

use cortex_m_rt::entry;
use defmt::*;
use embassy_executor::Executor;
use embassy_stm32::usart::{Config, Uart};
use embassy_time::{Timer, Duration};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};
// 导入内存分配器
use linked_list_allocator::LockedHeap;
// 定义全局内存分配器
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// 定义堆内存区域
const HEAP_SIZE: usize = 1024 * 8; // 例如，分配 8KB 作为堆
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[embassy_executor::task]
async fn main_task() {
    let p = embassy_stm32::init(Default::default());

    let config = Config::default();
    let mut usart = Uart::new_blocking(
        p.USART3,
        p.PD9,  // USART3_TX
        p.PD8,  // USART3_RX
        config,
    ).unwrap();

    info!("UART echo server started");

    let mut buf = [0u8; 64]; // 缓冲区更大一些
    let mut idx = 0;

    loop {
        let mut byte = [0u8; 1];
        // 阻塞读取单个字节
        // if let Ok(()) = usart.blocking_read(&mut byte) {
        //     if byte[0] == 0 { // 遇到\0
        //         // 发送缓冲区内容（只发有效部分）
        //         unwrap!(usart.blocking_write(&buf[..idx]));
        //         info!("Echoed string: {:?}", &buf[..idx]);
        //         idx = 0; // 重置缓冲区索引
        //     } else {
        //         if idx < buf.len() {
        //             buf[idx] = byte[0];
        //             idx += 1;
        //         } else {
        //             // 缓冲区满了，自动回显并重置
        //             unwrap!(usart.blocking_write(&buf[..idx]));
        //             info!("Buffer full, echoed string: {:?}", &buf[..idx]);
        //             idx = 0;
        //         }
        //     }
        // }

if let Ok(()) = usart.blocking_read(&mut byte) {
    if byte[0] == 0 { // 遇到\0
        unwrap!(usart.blocking_write(&buf[..idx]));
        // info!("Echoed string: {:?}", &buf[..idx]);
        let echoed_str: String = (&buf[..idx]).iter().map(|b| *b as char).collect();
        info!("Echoed string: {:?}", echoed_str.as_str());
        idx = 0;
    } else {
        if idx < buf.len() {
            buf[idx] = byte[0];
            idx += 1;
            info!("Received byte: {:?}", byte[0]);
        } else {
            // 缓冲区满了，先发送，再把当前字节作为新一轮的第一个字节
            unwrap!(usart.blocking_write(&buf[..idx]));
            info!("Buffer full, echoed string: {:?}", &buf[..idx]);
            buf[0] = byte[0];
            idx = 1;
        }
    }
}


        // 可选：周期性消息
        // unwrap!(usart.blocking_write(b"Hello from Nucleo-H743ZI (every 2s)!\r\n"));
        // Timer::after(Duration::from_secs(2)).await;
    }


}


#[embassy_executor::task]
async fn periodic_task() {

    // 这里可以添加周期性任务的代码
    loop {
        info!("Periodic task running...");
        // Timer::after(Duration::from_secs(2)).await;
        Timer::after(Duration::from_micros(20)).await;
        info!("Periodic task finished.");
    }
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[entry]
fn main() -> ! {
    info!("Starting UART echo example with periodic messages");
    let executor = EXECUTOR.init(Executor::new());
    // executor.run(|spawner| {
    //     unwrap!(spawner.spawn(main_task()));
    // });
unsafe { // 因为操作原始指针和静态可变变量，需要 unsafe
    ALLOCATOR.lock().init(HEAP_MEM.as_mut_ptr(), HEAP_SIZE);
}
    let sss = ALLOCATOR.    lock();
    executor.run(|spawner| {
        unwrap!(spawner.spawn(main_task()));
        unwrap!(spawner.spawn(periodic_task()));
    });
} 