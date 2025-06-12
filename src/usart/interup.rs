#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::mode::Async;
// Import the necessary types for async Uart, split Rx/Tx, and config
use embassy_stm32::usart::{Config, Uart, UartRx, UartTx};
// Import the macro for binding interrupts and the peripheral types
use embassy_stm32::{bind_interrupts, peripherals, usart};
// Import sync primitives for inter-task communication
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
// Import time for potential delays (though not strictly needed for echo)
use embassy_time::{Timer, Duration};
// Necessary for defmt and panic handling
use {defmt_rtt as _, panic_probe as _};

// Define the interrupt bindings. This macro associates the UART interrupt
// with the embassy-stm32 HAL's interrupt handler for that peripheral.
// The exact interrupt name (UART3 or USART3_COMMON) depends on your specific chip/HAL version.
// For many chips, it's just the peripheral name, like USART3.
bind_interrupts!(struct Irqs {
    USART3 => usart::InterruptHandler<peripherals::USART3>;
    // Add other interrupts you might need here
});

// Define a static channel for communication between the reader and writer tasks.
// It will send a tuple: ([buffer], valid_length)
// Capacity 1 means the writer must consume the previous message before the reader can send a new one.
static ECHO_CHANNEL: Channel<ThreadModeRawMutex, ([u8; 64], usize), 1> = Channel::new();

// Use the #[embassy_executor::main] macro for the entry point.
// It sets up the executor and provides a Spawner.
#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    // Initialize the STM32 peripherals
    let p = embassy_stm32::init(Default::default());
    info!("Hello World!");

    let config = Config::default();

    // Create the asynchronous Uart instance.
    // It needs the peripheral, TX pin, RX pin, the bound Irqs struct,
    // and the TX/RX DMA channels (verify these for your chip).
    let usart = Uart::new(
        p.USART3,       // The USART peripheral
        p.PD9,          // TX pin
        p.PD8,          // RX pin
        Irqs,           // The bound interrupt handler
        p.DMA1_CH0,     // Example TX DMA channel (VERIFY FOR YOUR CHIP/USART3)
        p.DMA1_CH1,     // Example RX DMA channel (VERIFY FOR YOUR CHIP/USART3)
        config,         // Configuration
    ).unwrap(); // Use unwrap for simplicity in example

    // Split the Uart instance into independent Transmit (Tx) and Receive (Rx) parts.
    let (tx, rx) = usart.split();

    // Spawn the reader task, giving it the Rx part of the UART and the channel sender.
    unwrap!(spawner.spawn(reader(rx, ECHO_CHANNEL.sender())));

    // Spawn the writer task, giving it the Tx part of the UART and the channel receiver.
    unwrap!(spawner.spawn(writer(tx, ECHO_CHANNEL.receiver())));

    // The main task can now potentially do other things, or just loop indefinitely
    // if the reader and writer tasks handle all the primary logic.
    // In this echo example, the reader/writer tasks handle everything,
    // so main can effectively just yield forever.
    loop {
        // Optionally, main could spawn other periodic tasks or handle different events.
        // info!("Main task is alive...");
        Timer::after(Duration::from_secs(5)).await;
    }
}

// Task responsible for reading bytes, buffering, and sending complete messages
#[embassy_executor::task]
async fn reader(
    mut rx: UartRx<'static, Async>, // Takes the asynchronous Rx part
    sender: embassy_sync::channel::Sender<'static, ThreadModeRawMutex, ([u8; 64], usize), 1> // Takes the channel sender
) {
    info!("Reader task started.");
    let mut buf = [0u8; 64]; // Buffer to accumulate received bytes
    let mut idx: usize = 0;  // Current index in the buffer

    loop {
        let mut byte = [0u8; 1]; // Buffer for a single byte
        // Asynchronously read one byte. This task yields until a byte arrives via interrupt/DMA.
        match rx.read(&mut byte).await {
            Ok(()) => {
                info!("Received byte: {:?}", byte[0]);

                // Check for null terminator (0)
                if byte[0] == 0 {
                    // Null terminator received, send the buffered data (excluding the 0)
                    if idx > 0 { // Only send if there's data in the buffer
                         // Send the buffer and its length to the writer task via the channel
                        unwrap!(sender.send((buf, idx)).await);
                        info!("Reader sent message (terminated by \\0) with length: {}", idx);
                    }
                    idx = 0; // Reset buffer index
                } else {
                    // Store the received byte in the buffer
                    if idx < buf.len() {
                        buf[idx] = byte[0];
                        idx += 1;
                    } else {
                        // Buffer full. Send the current buffer, then start a new one with the current byte.
                        unwrap!(sender.send((buf, idx)).await);
                        info!("Reader sent message (buffer full) with length: {}", idx);

                        // Reset buffer and add the current byte as the first element
                        buf[0] = byte[0];
                        idx = 1;
                    }
                }
            }
            Err(e) => {
                // Handle potential read errors
                error!("UART Read error: {:?}", e);
                // Depending on the error, you might want to break the loop, reset the peripheral, etc.
                // For simplicity, we just log and continue the loop trying to read again.
                Timer::after(Duration::from_millis(100)).await; // Prevent tight loop on persistent error
            }
        }
    }
}

// Task responsible for receiving complete messages from the channel and writing them out
#[embassy_executor::task]
async fn writer(
    mut tx: UartTx<'static, Async>, // Takes the asynchronous Tx part
    receiver: embassy_sync::channel::Receiver<'static, ThreadModeRawMutex, ([u8; 64], usize), 1> // Takes the channel receiver
) {
    info!("Writer task started.");
    loop {
        // Asynchronously receive a message (buffer and length) from the channel.
        // This task yields until a message is sent by the reader task.
        let (buf, len) = receiver.receive().await;
        info!("Writer received message with length: {}", len);

        // Asynchronously write the received data from the buffer (using the valid length).
        // This task yields until the transmission is complete via interrupt/DMA.
        match tx.write(&buf[..len]).await {
             Ok(()) => {
                info!("Writer successfully sent message.");
             }
             Err(e) => {
                 error!("UART Write error: {:?}", e);
                 // Handle potential write errors, similar to read errors.
                 Timer::after(Duration::from_millis(100)).await; // Prevent tight loop
             }
        }
    }
}