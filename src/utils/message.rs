#![allow(unused)]
use anstream::{AutoStream, stderr, stdout};
pub use owo_colors::OwoColorize;

pub fn supports_color() -> bool {
    // RPC 模式由请求参数 color 决定；否则按 anstream 的终端检测
    if crate::rpc::is_enabled() {
        return crate::rpc::is_color_enabled();
    }
    !matches!(
        anstream::stdout().current_choice(),
        anstream::ColorChoice::Never
    )
}

/// 实际写入 stdout/stderr（绕过宏的上下文暂停逻辑）
pub fn raw_print(stream: &str, text: &str) {
    if stream == "stderr" {
        anstream::eprintln!("{}", text);
    } else {
        anstream::println!("{}", text);
    }
}

#[macro_export]
macro_rules! _internal_print {
    ($stream:ident, $($arg:tt)*) => {{
        let __text = ::std::format!($($arg)*);
        let __stream = if stringify!($stream) == "eprintln" {
            "stderr"
        } else {
            "stdout"
        };
        if $crate::rpc::emit_output(__stream, &__text) {
            // JSON-RPC 模式：已作为 output 通知发出
        } else if let Some(__ctx) = $crate::context::try_gctx() {
            __ctx.multiprogress.suspend(|| {
                $crate::utils::message::raw_print(__stream, &__text);
            });
        } else {
            $crate::utils::message::raw_print(__stream, &__text);
        }
    }};
}

#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {
        $crate::_internal_print!(println, $($arg)*)
    };
}

#[macro_export]
macro_rules! emsg {
    ($($arg:tt)*) => {
        $crate::_internal_print!(eprintln, $($arg)*)
    };
}

#[macro_export]
macro_rules! msg_progress {
    ($($arg:tt)*) => {
        $crate::msg!("{}", {
            let msg = format!($($arg)*);

            if $crate::utils::message::supports_color() {
                msg.lines()
                    .map(|line| format!("{}{}", ">>> ".bold(), line))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                msg.lines()
                    .map(|line| format!(">>> {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
    };
}

#[macro_export]
macro_rules! msg_info {
    ($($arg:tt)*) => {
        $crate::msg!("{}", {
            let msg = format!($($arg)*);

            if $crate::utils::message::supports_color() {
                msg.lines()
                    .map(|line| format!("{}{}", " * ".green().bold(), line))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                msg.lines()
                    .map(|line| format!("[I] {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
    };
}

#[macro_export]
macro_rules! msg_error {
    ($($arg:tt)*) => {
        $crate::msg!("{}", {
            let msg = format!($($arg)*);

            if $crate::utils::message::supports_color() {
                msg.lines()
                    .map(|line| format!("{}{}", " * ".red().bold(), line))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                msg.lines()
                    .map(|line| format!("[E] {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
    };
}

#[macro_export]
macro_rules! msg_warn {
    ($($arg:tt)*) => {
        $crate::msg!("{}", {
            let msg = format!($($arg)*);

            if $crate::utils::message::supports_color() {
                msg.lines()
                    .map(|line| format!("{}{}", " * ".yellow().bold(), line))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                msg.lines()
                    .map(|line| format!("[W] {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
    };
}

#[macro_export]
macro_rules! msg_item {
    ($status:expr, $($arg:tt)*) => {
        $crate::msg!(" - [ {} ] {}", $status, format!($($arg)*))
    };
}

pub use emsg;
pub use msg;
pub use msg_error;
pub use msg_info;
pub use msg_item;
pub use msg_progress;
pub use msg_warn;
