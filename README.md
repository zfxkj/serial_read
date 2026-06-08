# serial_read 串口读取工具

`serial_read` 是一个用于通过串口发送命令并读取响应的命令行工具。它适合调试串口设备、自动化读取设备返回值、快速验证 ASCII 或十六进制协议命令。

工具当前的读取策略是：发送命令后等待设备响应；只要收到任意数据，就继续等待后续数据；如果超过“部分数据超时时间”没有收到新数据，就认为本次响应结束并统一输出。这个机制可以减少串口数据尚未收完就被截断的问题。

## 功能特性

- 支持指定串口、波特率和发送命令。
- 支持文本命令和十六进制命令两种发送方式。
- 支持自动追加 `CR`、`LF`、`CRLF` 换行符。
- 支持总超时和部分数据超时，避免响应未收完时提前退出。
- 支持短指令，适合高频调试时快速输入。
- 输出说明为中文，文本模式下不可见字符会以转义形式显示。
- 可本地构建 x86_64 程序，也可交叉构建 arm64 程序。

## 参数说明

| 短指令 | 长指令 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `-p` | `--port` | 是 | 无 | 串口名称，例如 `/dev/ttyUSB0`、`/dev/ttyS0`、`COM3` |
| `-b` | `--baud` | 是 | 无 | 串口波特率，例如 `9600`、`115200`、`230400` |
| `-c` | `--command` | 是 | 无 | 要发送的命令；普通模式下必须是 ASCII 文本 |
| `-x` | `--hex` | 否 | 关闭 | 将 `--command` 内容按十六进制字符串解析 |
| `-n` | `--newline` | 否 | `none` | 发送命令后追加换行符，可选 `none`、`cr`、`lf`、`crlf` |
| `-T` | `--total-timeout-ms` | 否 | `10000` | 等待完整响应的总超时时间，单位毫秒 |
| `-t` | `--partial-timeout-ms` | 否 | `500` | 每次收到数据后继续等待后续数据的时间，单位毫秒 |
| `-h` | `--help` | 否 | 无 | 打印帮助信息 |
| `-V` | `--version` | 否 | 无 | 打印版本信息 |

## 快速开始

查看帮助：

```bash
serial_read -h
```

发送普通 ASCII 命令：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c config
```

发送普通 ASCII 命令，并追加 `CRLF`：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c config -n crlf
```

发送十六进制命令：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c 76657273696F6E -x
```

设置更长的总超时和部分数据超时：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c config -T 20000 -t 1000
```

## 读取超时机制

工具有两个超时参数：

- `--total-timeout-ms`：总超时。默认 `10000` 毫秒。
- `--partial-timeout-ms`：部分数据超时。默认 `500` 毫秒。

读取流程如下：

1. 发送命令。
2. 在总超时时间内等待第一批响应数据。
3. 如果一直没有收到任何数据，超过总超时后输出 `读取超时，未收到响应。`。
4. 如果收到任意数据，把数据追加到响应缓冲区。
5. 每次收到数据后，重新开始计算部分数据超时。
6. 如果超过部分数据超时时间没有新数据到达，认为响应结束，统一打印完整响应。
7. 如果持续有数据到达，则一直收集，直到部分数据超时或总超时先到达。

例如默认配置下：

- 第一批数据在 1 秒后到达：会被正常接收，因为总超时是 10 秒。
- 收到数据后 300 毫秒又收到新数据：会继续等待，因为没有超过 500 毫秒。
- 收到数据后超过 500 毫秒没有新数据：停止读取并输出当前完整响应。
- 设备一直持续输出数据：最多读取到总超时到达为止。

如果设备响应较慢，建议增大部分数据超时：

```bash
serial_read -p /dev/ttyUSB0 -b 115200 -c status -t 1500
```

如果设备处理命令较慢，建议增大总超时：

```bash
serial_read -p /dev/ttyUSB0 -b 115200 -c status -T 30000
```

## 文本模式

默认不加 `-x` 时，命令按 ASCII 文本发送。

普通文本命令必须只包含 `0x00` 到 `0x7F` 范围内的字符。当前工具不会按 UTF-8 解释普通命令，因为很多串口协议使用的是 ASCII 或二进制字节。

示例：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c version
```

如果设备返回可见 ASCII 文本，会直接显示：

```text
收到(文本): version=1.0
```

如果设备返回不可见字符，会显示为转义：

```text
收到(文本): OK\r\n
```

常见转义显示：

| 字节 | 显示 |
| --- | --- |
| 回车 `0x0D` | `\r` |
| 换行 `0x0A` | `\n` |
| 制表符 `0x09` | `\t` |
| 其他不可见字节 | `\xNN` |
| 非 ASCII 字节 | `\xNN` |

## 十六进制模式

加 `-x` 后，`-c` 或 `--command` 的内容会按十六进制字符串解析。

例如发送 ASCII 文本 `version` 对应的十六进制：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c 76657273696F6E -x
```

十六进制字符串可以使用 `hex` crate 支持的标准格式。建议输入连续十六进制字符，每个字节两位：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c 010300000002C40B -x
```

十六进制模式下，发送和接收都会以十六进制显示：

```text
已发送命令(十六进制): 01 03 00 00 00 02 C4 0B
收到(十六进制): 01 03 04 00 01 00 02 2A 32
```

如果十六进制字符串格式错误，程序会报错：

```text
无效的十六进制字符串: ...
```

## 换行模式

使用 `-n` 或 `--newline` 可以在命令末尾追加换行字节。

| 值 | 追加字节 | 说明 |
| --- | --- | --- |
| `none` | 不追加 | 默认模式 |
| `cr` | `0x0D` | 回车 |
| `lf` | `0x0A` | 换行 |
| `crlf` | `0x0D 0x0A` | 回车换行 |

示例：

```bash
serial_read -p /dev/ttyUSB0 -b 115200 -c AT -n crlf
```

上面的命令实际发送：

```text
AT\r\n
```

## 串口配置

程序当前固定使用以下串口参数：

| 配置项 | 当前值 |
| --- | --- |
| 数据位 | 8 |
| 校验位 | 无 |
| 停止位 | 1 |
| 流控 | 无 |

也就是常见的 `8N1` 配置。

如果设备需要其他数据位、校验位、停止位或流控，当前版本还没有提供命令行参数，需要修改源码中的串口配置。

## Linux 权限说明

在 Linux 上访问 `/dev/ttyUSB0`、`/dev/ttyS0` 等串口通常需要权限。

查看串口设备：

```bash
ls -l /dev/ttyUSB*
ls -l /dev/ttyS*
```

如果提示没有权限，可以临时使用 `sudo`：

```bash
sudo ./serial_read -p /dev/ttyUSB0 -b 230400 -c config
```

也可以把当前用户加入串口设备所属用户组。常见用户组是 `dialout`：

```bash
sudo usermod -aG dialout $USER
```

加入用户组后通常需要重新登录才会生效。

## 构建

### 本机调试构建

```bash
cargo build
```

产物路径：

```bash
target/debug/serial_read
```

### 本机发布构建

```bash
cargo build --release
```

产物路径：

```bash
target/release/serial_read
```

### 本地交叉构建 arm64

当前项目已经验证可以在 x86_64 Linux 主机上本地交叉构建 `aarch64-unknown-linux-gnu` 目标。

确认 Rust 目标是否已安装：

```bash
rustup target list --installed
```

如果没有 `aarch64-unknown-linux-gnu`，先安装：

```bash
rustup target add aarch64-unknown-linux-gnu
```

确认系统存在交叉编译器：

```bash
which aarch64-linux-gnu-gcc
```

如果缺少交叉编译器，需要先安装系统包。不同发行版命令不同，Ubuntu/Debian 常见命令如下：

```bash
sudo apt-get update
sudo apt-get install -y gcc-aarch64-linux-gnu
```

构建 arm64 release 程序：

```bash
cargo build --target aarch64-unknown-linux-gnu --release
```

产物路径：

```bash
target/aarch64-unknown-linux-gnu/release/serial_read
```

验证产物架构：

```bash
file target/aarch64-unknown-linux-gnu/release/serial_read
readelf -h target/aarch64-unknown-linux-gnu/release/serial_read
```

期望看到：

```text
ARM aarch64
Machine: AArch64
```

### 关于 `serialport` 依赖

项目当前在 `Cargo.toml` 中关闭了 `serialport` 的默认特性：

```toml
serialport = { version = "4.5.0", default-features = false }
```

这样做是为了让本地 arm64 交叉构建更简单，避免引入目标架构的 `libudev-dev` 依赖。

当前程序只按用户指定的 `--port` 打开串口，不需要枚举系统串口列表，因此关闭 `libudev` 默认特性是合适的。

如果以后需要增加“列出串口”功能，可能需要重新开启 `serialport` 的默认特性，并为交叉编译环境准备目标架构的 `libudev` 开发包。

## Docker / cross 构建说明

项目包含 `Cross.toml`，配置了：

```toml
[target.aarch64-unknown-linux-gnu]
dockerfile = "Dockerfile.aarch64"
```

这表示可以使用 `cross` 配合 `Dockerfile.aarch64` 构建 arm64 目标。

示例命令：

```bash
cross build --target aarch64-unknown-linux-gnu --release
```

注意：使用这种方式时，需要确保项目根目录存在 `Dockerfile.aarch64`，并且 Docker 可用。

如果只是构建当前版本，推荐优先使用本地交叉构建：

```bash
cargo build --target aarch64-unknown-linux-gnu --release
```

## 输出示例

普通文本模式：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c version -n crlf
```

可能输出：

```text
已连接到设备端口: /dev/ttyUSB0
已发送命令(文本): version\r\n
收到(文本): version=1.2.3\r\n
```

十六进制模式：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c 76657273696F6E -x
```

可能输出：

```text
已连接到设备端口: /dev/ttyUSB0
已发送命令(十六进制): 76 65 72 73 69 6F 6E
收到(十六进制): 4F 4B 0D 0A
```

无响应超时：

```text
已连接到设备端口: /dev/ttyUSB0
已发送命令(文本): config
读取超时，未收到响应。
```

## 常见问题

### 为什么收到的数据以前会被截断？

串口读取不是天然按“完整响应”返回的。一次 `read` 只能说明当前读到了部分可用字节，不代表设备已经发完。

当前程序使用部分数据超时解决这个问题：每次收到数据后继续等待一段时间，如果期间又收到数据，就重新计时。只有超过部分数据超时时间没有新数据，才认为响应结束。

### `-t` 应该设置多大？

默认 `500` 毫秒适合很多常见设备。

如果设备响应中间经常有较大间隔，可以调大：

```bash
serial_read -p /dev/ttyUSB0 -b 115200 -c config -t 1000
```

如果希望更快结束读取，可以调小：

```bash
serial_read -p /dev/ttyUSB0 -b 115200 -c config -t 200
```

### `-T` 和 `-t` 有什么区别？

`-T` 是整个读取过程的最大时间，防止设备持续输出或一直不结束。

`-t` 是收到数据后的静默等待时间，用于判断“这一轮响应是否已经发完”。

简单说：

- `-T` 控制最多等多久。
- `-t` 控制最后一批数据后再等多久。

### 为什么普通文本模式只允许 ASCII？

很多串口协议是 ASCII 文本或二进制字节，不一定是 UTF-8。为了避免 UTF-8 解码导致误解，当前普通文本模式按 ASCII 处理。

如果需要发送任意字节，请使用十六进制模式 `-x`。

### 如何发送非 ASCII 或二进制数据？

使用十六进制模式：

```bash
serial_read -p /dev/ttyUSB0 -b 230400 -c 01020304FF -x
```

### 为什么响应里的换行显示成 `\r\n`？

这是刻意设计。普通文本模式会把不可见字符显示成转义，方便看清设备实际返回了哪些字节。

### 找不到串口怎么办？

Linux 上可以查看：

```bash
ls /dev/ttyUSB*
ls /dev/ttyACM*
ls /dev/ttyS*
```

Windows 上串口通常是：

```text
COM1
COM2
COM3
```

### 运行 arm64 程序提示找不到解释器怎么办？

当前 arm64 构建产物是动态链接程序，目标系统需要有对应的 Linux arm64 动态链接器和基础运行库。

如果目标设备是常见的 arm64 Linux 系统，通常可以直接运行。如果在非 arm64 主机上运行，会因为架构不匹配而失败。

## 开发检查

检查代码是否能编译：

```bash
cargo check
```

查看帮助信息：

```bash
cargo run -- --help
```

构建 arm64 release：

```bash
cargo build --target aarch64-unknown-linux-gnu --release
```

## 当前限制

- 当前只支持 `8N1` 串口配置。
- 当前不支持列出可用串口。
- 当前不支持按协议结束符停止读取，只支持总超时和部分数据超时。
- 当前普通文本模式按 ASCII 处理，不按 UTF-8 处理。
- 当前输出到标准输出，不支持写入文件。

