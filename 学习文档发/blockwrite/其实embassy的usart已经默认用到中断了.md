# 1. 问题


```rust
#![no_std]

#![no_main]



use cortex_m_rt::entry;

use defmt::*;

use embassy_executor::Executor;

use embassy_stm32::usart::{Config, Uart};

use embassy_time::{Timer, Duration};

use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};



#[embassy_executor::task]

async fn main_task() {

    let p = embassy_stm32::init(Default::default());



    let config = Config::default();

    let mut usart = Uart::new_blocking(

        p.USART3,

        p.PD9,  // USART3_TX

        p.PD8,  // USART3_RX

        config,

    ).unwrap();



    info!("UART echo server started");



    let mut buf = [0u8; 64]; // 缓冲区更大一些

    let mut idx = 0;



    loop {

        let mut byte = [0u8; 1];

        // 阻塞读取单个字节

        // if let Ok(()) = usart.blocking_read(&mut byte) {

        //     if byte[0] == 0 { // 遇到\0

        //         // 发送缓冲区内容（只发有效部分）

        //         unwrap!(usart.blocking_write(&buf[..idx]));

        //         info!("Echoed string: {:?}", &buf[..idx]);

        //         idx = 0; // 重置缓冲区索引

        //     } else {

        //         if idx < buf.len() {

        //             buf[idx] = byte[0];

        //             idx += 1;

        //         } else {

        //             // 缓冲区满了，自动回显并重置

        //             unwrap!(usart.blocking_write(&buf[..idx]));

        //             info!("Buffer full, echoed string: {:?}", &buf[..idx]);

        //             idx = 0;

        //         }

        //     }

        // }



if let Ok(()) = usart.blocking_read(&mut byte) {

    if byte[0] == 0 { // 遇到\0

        unwrap!(usart.blocking_write(&buf[..idx]));

        info!("Echoed string: {:?}", &buf[..idx]);

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



static EXECUTOR: StaticCell<Executor> = StaticCell::new();



#[entry]

fn main() -> ! {

    info!("Starting UART echo example with periodic messages");

    let executor = EXECUTOR.init(Executor::new());

    executor.run(|spawner| {

        unwrap!(spawner.spawn(main_task()));

    });

}


```
也就是说这里其实已经用到了中断？


您好！

是的，您说得对，您的第一个代码示例中使用 `Uart::new_blocking` 和 `blocking_read`、`blocking_write` 方法，**底层很可能已经使用了中断**来等待数据或等待发送完成，而不是纯粹的自旋轮询（Busy-waiting）。

**解释一下：**

在大多数现代的 HAL（硬件抽象层）实现中，即使是所谓的“阻塞”（Blocking）I/O 操作，为了提高效率和允许 CPU 进入低功耗模式等待，通常会启用相关外设的中断。当操作可以继续时（例如，接收到一个字节触发 RXNE 中断，或者发送缓冲区空触发 TXE 中断），中断服务程序会被执行。这个 ISR 然后会做一些最小的事情（比如清除中断标志），并通知等待的阻塞函数可以继续执行了。

所以，当您的代码调用 `usart.blocking_read(&mut byte)` 时：

1.  UART 接收中断 (RXNE) 可能被启用。
2.  您的 `main_task` 会进入一个等待状态，让出 CPU（不像纯轮询那样一直占用 CPU）。
3.  当 UART 接收到一个字节时，RXNE 中断触发。
4.  `embassy-stm32` HAL 内部的中断处理程序运行。
5.  中断处理程序会通知 `blocking_read` 函数（或者更底层等待机制）接收到了数据。
6.  `blocking_read` 函数从 UART 数据寄存器读取字节。
7.  `main_task` 从等待状态恢复，继续执行。

**关键点在于：**

虽然使用了中断，但它仍然是**阻塞的**。`blocking_read` 调用会暂停当前的 `main_task` 的执行，直到收到数据为止。在这期间，Embassy 的执行器无法在同一个 CPU 核上运行同一个任务内的其他代码（比如主循环中位于 `blocking_read` 之后的代码）。

而之前我们讨论的**异步（Async/Await）**方法 (`usart.read().await`) 也是依赖中断（通常是 DMA 中断）来驱动的，但它的不同之处在于，当调用 `.await` 时，当前的**任务会主动让出 CPU**，允许 Embasssy 执行器去运行**其他**准备好的任务。当对应的中断发生时，执行器会被通知，然后重新调度之前等待的那个任务继续执行。

所以，是的，您的第一个示例已经用到了中断来避免纯粹的 CPU 浪费式轮询。但它属于**阻塞式中断驱动 I/O**，而我们后面讨论的第二个示例属于**异步（非阻塞）中断/DMA 驱动 I/O**，后者更能体现 Embassy 协作式多任务和异步编程的优势。

您最初想研究的“通过中断处理函数来处理接收到的信息”这种模式，在阻塞式 I/O 中，中断处理函数通常只负责非常简单的通知或数据暂存；在异步 I/O 中，中断处理函数（通常是 HAL 内部的）负责唤醒等待的异步任务，真正的应用逻辑（缓冲、判断、发送到 Channel）在异步任务中完成。您第一个例子正是阻塞式中断驱动的一个体现。

# 2. 具体疑问

```shell
embassy是怎么能够在没有就绪任务时（比如当前总共就一个任务，这个任务在block_read）,block_read如何使得让出 CPU（此时没有任何代码执行），即cpu此时在休眠等待中断

这是可行的吗？
```


您的疑问非常棒！这确实是理解阻塞式调用和异步调用在底层如何与硬件交互的关键点。

您是对的，当只有一个任务，并且这个任务调用了 `blocking_read` 时，这个任务本身是被“冻结”在那里，等待数据到来。Embassy 的协作式执行器在这种情况下**确实没有任何其他应用层任务可以运行**。

那 CPU 是如何等待中断的呢？答案在于 `blocking_read` 底层的实现，它利用了微控制器的**低功耗等待指令**，最常见的是 `WFI` (Wait For Interrupt) 或 `WFE` (Wait For Event) 指令。

以下是 `blocking_read` 在这种场景下的大致工作原理：

1.  **启用中断:** `blocking_read` 函数首先会配置 UART 硬件，启用接收中断（例如 RXNE - Receive Data Not Empty 中断）。
2.  **清除中断标志:** 确保没有待处理的旧中断标志。
3.  **进入等待循环:** 函数会进入一个内部循环。在这个循环的某个点，当它发现数据还没到来（RXNE 标志未设置）时，它会执行 `cortex_m::asm::wfi()` 指令。
4.  **CPU 休眠 (WFI):** `WFI` 是一个 CPU 指令。当 CPU 执行到这条指令时，它会停止执行后续的指令，并进入一个低功耗的睡眠状态，**直到发生一个被允许的中断**。
5.  **中断唤醒:** 当 UART 接收到一个字节时，硬件会触发 RXNE 中断。
6.  **中断服务程序 (ISR) 执行:** CPU 被中断唤醒后，会立即跳转到对应的中断服务程序（在这个例子中，是 `embassy-stm32` HAL 内部为该 UART 中断绑定的 ISR）。这个 ISR 通常会非常简短，可能只是清除中断标志，或者在更复杂的场景下将数据从寄存器读出暂存。
7.  **返回等待循环:** ISR 执行完毕后，CPU 会返回到之前执行 `WFI` 指令的下一条指令。
8.  **检查条件并继续:** `blocking_read` 函数内部的循环会继续执行。它会再次检查 RXNE 标志（现在应该已经设置了，因为数据来了），然后从数据寄存器读取字节。
9.  **退出阻塞:** 一旦成功读取到所需的字节，`blocking_read` 函数就完成了它的任务，返回到调用者。此时，`main_task` 恢复执行其后续代码。

所以，即使只有一个任务，并且这个任务调用了 `blocking_read`，CPU 也能通过执行 `WFI` 指令进入休眠状态，等待中断的到来。它并非由 Embassy 执行器的调度逻辑来“让出 CPU”，而是由底层的 CPU 指令直接控制进入休眠。

**与 `async`/`await` 的核心区别在于：**

* `blocking_read` 使用 `WFI` 让 CPU 休眠，但这**阻塞了当前的 Embassy 任务**，该任务无法执行其他代码，也无法通过协作让出 CPU 给其他 **Embassy 任务**运行。
* `async_read().await` 会让当前的异步任务**主动向 Embassy 执行器报告它需要等待**（例如，等待 DMA 完成或特定中断），然后**挂起自己**，将 CPU 控制权交回给执行器。执行器此时就可以选择运行**其他准备好的异步任务**。当等待的事件发生时（由中断驱动），执行器会被通知，然后将之前挂起的任务重新标记为可运行，并在适当的时机再次调度执行。

所以，您的理解“CPU 此时在休眠等待中断”是正确的，这是通过 `WFI` 等指令实现的。但这种休眠是与阻塞调用绑定的，不同于异步任务通过 `.await` 产生的协作式让出 CPU 的方式。