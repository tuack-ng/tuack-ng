# tuack-plugin-sdk

tuack-ng 插件的 Rust SDK：实现 `Processor` / `Renderer` / `Dumper`，用宏注册为 extism 导出函数，
JSON 编解码、内存与错误处理由 SDK 接管。

编译目标为 `wasm32-wasip1`，产物作为 tuack-ng 插件包的 `entry`。

```rust,ignore
#![no_main]
use tuack_plugin_sdk::{processor, Document, Error, Processor, ProcessorOutput};

struct MyProcessor;

impl Processor for MyProcessor {
    fn new() -> Self {
        MyProcessor
    }

    fn process(&self, doc: Document) -> Result<ProcessorOutput, Error> {
        Ok(ProcessorOutput { ast: doc, warnings: Vec::new() })
    }
}

processor!(MyProcessor);
```

许可证：AGPL-3.0-or-later。
