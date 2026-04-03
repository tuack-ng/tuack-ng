#![allow(unused)]
pub use colored;
pub use colored::Colorize;

// 核心宏：内部使用
#[macro_export]
macro_rules! _internal_print {
    ($stream:ident, $($arg:tt)*) => {
        if let Some(ctx) = crate::context::GLOBAL_CONTEXT.get() {
            ctx.multiprogress.suspend(|| {
                $stream!($($arg)*);
            });
        } else {
            $stream!($($arg)*);
        }
    };
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
        $crate::emsg!("{}", {
            let msg = format!($($arg)*);
            if $crate::colored::control::SHOULD_COLORIZE.should_colorize() {
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
        $crate::emsg!("{}", {
            let msg = format!($($arg)*);
            if $crate::colored::control::SHOULD_COLORIZE.should_colorize() {
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
        $crate::emsg!("{}", {
            let msg = format!($($arg)*);
            if $crate::colored::control::SHOULD_COLORIZE.should_colorize() {
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
        $crate::emsg!("{}", {
            let msg = format!($($arg)*);
            if $crate::colored::control::SHOULD_COLORIZE.should_colorize() {
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
