#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::Config;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

// 定义LED模式
#[derive(defmt::Format, Clone, Copy, PartialEq)]
enum LedMode {
    Slow,
    Medium,
    Fast,
    Off,
}

impl LedMode {
    fn next(self) -> Self {
        match self {
            LedMode::Slow => LedMode::Medium,
            LedMode::Medium => LedMode::Fast,
            LedMode::Fast => LedMode::Off,
            LedMode::Off => LedMode::Slow,
        }
    }
    
    fn duration(&self) -> Duration {
        match self {
            LedMode::Slow => Duration::from_millis(1000),
            LedMode::Medium => Duration::from_millis(500),
            LedMode::Fast => Duration::from_millis(200),
            LedMode::Off => Duration::from_millis(100), // 用于防抖检查
        }
    }
}

static MODE: embassy_sync::mutex::Mutex<ThreadModeRawMutex, LedMode> = 
    embassy_sync::mutex::Mutex::new(LedMode::Slow);
static SIGNAL: embassy_sync::signal::Signal<ThreadModeRawMutex, ()> = 
    embassy_sync::signal::Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("LED Mode Switch Demo Started!");

    // 控制PB0引脚的LED（主LED）
    let led = Output::new(p.PB0, Level::High, Speed::Low);
    
    // 配置PC13为按键输入（上拉模式）
    let button = Input::new(p.PC13, Pull::Down);
    
    // 生成按键检测任务和LED控制任务
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(led_task(led)).unwrap();
}

// 按键检测任务
#[embassy_executor::task]
async fn button_task(button: Input<'static>) {
    let debounce_time = Duration::from_millis(50);
    let mut last_state = button.is_high();
    let mut last_press = embassy_time::Instant::now();
    let mut long_press_timer = embassy_time::Instant::now();
    
    loop {
        let current_state = button.is_high();
        
        // 检测下降沿（按键按下）
        if last_state && !current_state {
            long_press_timer = embassy_time::Instant::now();
        }
        
        // 检测上升沿（按键释放）
        if !last_state && current_state {
            let now = embassy_time::Instant::now();
            
            // 防抖处理
            if now.duration_since(last_press) > debounce_time {
                // 短按处理
                if now.duration_since(long_press_timer) < Duration::from_millis(1000) {
                    info!("Button short pressed");
                    
                    // 切换模式
                    let mut mode = MODE.lock().await;
                    *mode = mode.next();
                    info!("Mode changed to: {:?}", *mode);
                    
                    // 发送信号通知LED任务
                    SIGNAL.signal(());
                }
            }
            last_press = now;
        }
        
        last_state = current_state;
        Timer::after(Duration::from_millis(10)).await; // 10ms扫描一次
    }
}

// LED控制任务
#[embassy_executor::task]
async fn led_task(mut led: Output<'static>) {
    loop {
        // 获取当前模式
        let current_mode = {
            let mode = MODE.lock().await;
            *mode
        };
        
        match current_mode {
            LedMode::Off => {
                led.set_low();
                // 等待信号或超时检查
                SIGNAL.wait().await;
            }
            _ => {
                let duration = current_mode.duration();
                
                // LED开
                led.set_high();
                
                // 等待期间检查信号
                if embassy_time::with_timeout(duration, SIGNAL.wait()).await.is_ok() {
                    continue; // 收到信号，立即检查新模式
                }
                
                // LED关
                led.set_low();
                
                // 等待期间检查信号
                if embassy_time::with_timeout(duration, SIGNAL.wait()).await.is_ok() {
                    continue; // 收到信号，立即检查新模式
                }
            }
        }
    }
}