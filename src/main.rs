use clap::{Arg, ArgAction, Command};
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

fn main() -> io::Result<()> {
    // 定义命令行参数
    let matches = Command::new("串口读取工具")
        .version("1.0")
        .about("用于通过串口发送命令并读取响应")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .override_usage("serial_read -p <串口> -b <波特率> -c <命令> [其他参数]")
        .help_template("{about}\n\n用法：{usage}\n\n参数：\n{options}")
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("串口")
                .help("串口名称，例如 /dev/ttyUSB0 或 COM3")
                .required(true),
        )
        .arg(
            Arg::new("baud")
                .short('b')
                .long("baud")
                .value_name("波特率")
                .help("串口波特率，例如 230400")
                .required(true),
        )
        .arg(
            Arg::new("command")
                .short('c')
                .long("command")
                .value_name("命令")
                .help("要发送的命令，例如 config；使用 --hex 时填写十六进制字符串，例如 76657273696F6E")
                .required(true),
        )
        .arg(
            Arg::new("hex")
                .short('x')
                .long("hex")
                .help("将命令按十六进制字符串解析")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("newline")
                .short('n')
                .long("newline")
                .value_name("换行模式")
                .help("追加换行符：none=不追加，cr=回车，lf=换行，crlf=回车换行；默认 none")
                .hide_default_value(true)
                .default_value("none"),
        )
        .arg(
            Arg::new("total-timeout-ms")
                .short('T')
                .long("total-timeout-ms")
                .value_name("毫秒")
                .help("等待完整响应的总超时时间，单位为毫秒；默认 10000")
                .hide_default_value(true)
                .default_value("10000"),
        )
        .arg(
            Arg::new("partial-timeout-ms")
                .short('t')
                .long("partial-timeout-ms")
                .value_name("毫秒")
                .help("收到数据后等待后续数据的超时时间，单位为毫秒；默认 500")
                .hide_default_value(true)
                .default_value("500"),
        )
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help("打印帮助信息")
                .action(ArgAction::Help),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .help("打印版本信息")
                .action(ArgAction::Version),
        )

        .get_matches();

    // 获取命令行参数
    let port_name = matches.get_one::<String>("port").expect("缺少串口参数");
    let baud_rate: u32 = matches
        .get_one::<String>("baud")
        .expect("缺少波特率参数")
        .parse()
        .expect("波特率必须是有效数字");
    let command = matches.get_one::<String>("command").expect("缺少命令参数");
    let is_hex = matches.get_flag("hex");
    let total_timeout_ms = parse_positive_millis(&matches, "total-timeout-ms", "总超时时间")?;
    let partial_timeout_ms =
        parse_positive_millis(&matches, "partial-timeout-ms", "部分数据超时时间")?;

    // 处理命令：ASCII 或 16 进制
    let mut command_bytes = if is_hex {
        hex::decode(command).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("无效的十六进制字符串: {}", e),
            )
        })?
    } else {
        if !command.is_ascii() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "文本命令只能包含 0x00 到 0x7F 范围内的字符",
            ));
        }
        command.as_bytes().to_vec()
    };

    match matches.get_one::<String>("newline").map(|s| s.as_str()) {
        Some("cr") => command_bytes.push(0x0D),
        Some("lf") => command_bytes.push(0x0A),
        Some("crlf") => {
            command_bytes.push(0x0D);
            command_bytes.push(0x0A);
        }
        _ => {}
    }

    // 串口配置
    let total_timeout = Duration::from_millis(total_timeout_ms);
    let partial_timeout = Duration::from_millis(partial_timeout_ms);

    // 打开串口
    let mut port = serialport::new(port_name, baud_rate)
        .timeout(total_timeout)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open()?;

    println!("已连接到设备端口: {}", port_name);
    // 发送命令
    port.write_all(&command_bytes)?;
    if is_hex {
        let hex_str: String = command_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        println!("已发送命令(十六进制): {}", hex_str);
    } else {
        println!("已发送命令(文本): {}", format_ascii_bytes(&command_bytes));
    }
    port.flush()?;

    // 读取响应：先等待首包，收到后按空闲超时继续收集后续分段。
    let mut buffer = [0u8; 1024];
    let mut response_bytes = Vec::new();
    let total_deadline = Instant::now() + total_timeout;
    let mut partial_deadline = None;
    loop {
        let now = Instant::now();
        let active_deadline = partial_deadline
            .unwrap_or(total_deadline)
            .min(total_deadline);

        if now >= active_deadline {
            if response_bytes.is_empty() {
                println!("读取超时，未收到响应。");
                break;
            }

            print_response(&response_bytes, is_hex);
            return Ok(());
        }

        port.set_timeout(active_deadline.saturating_duration_since(now))?;

        match port.read(&mut buffer) {
            Ok(bytes_read) => {
                if bytes_read > 0 {
                    response_bytes.extend_from_slice(&buffer[..bytes_read]);
                    partial_deadline = Some(Instant::now() + partial_timeout);
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                if response_bytes.is_empty() {
                    println!("读取超时，未收到响应。");
                    break;
                }

                print_response(&response_bytes, is_hex);
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

fn parse_positive_millis(matches: &clap::ArgMatches, id: &str, label: &str) -> io::Result<u64> {
    let millis = matches
        .get_one::<String>(id)
        .expect("超时参数已有默认值")
        .parse()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{}必须是有效数字: {}", label, e),
            )
        })?;

    if millis == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{}必须大于 0", label),
        ));
    }

    Ok(millis)
}

fn print_response(response_bytes: &[u8], is_hex: bool) {
    if is_hex {
        // 以十六进制打印，每个字节用两位大写十六进制，空格分隔
        let hex_str: String = response_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(" ");
        println!("收到(十六进制): {}", hex_str);
    } else {
        // 普通文本模式
        println!("收到(文本): {}", format_ascii_bytes(response_bytes));
    }
}

fn format_ascii_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            b'\r' => "\\r".to_string(),
            b'\n' => "\\n".to_string(),
            b'\t' => "\\t".to_string(),
            0x20..=0x7E => char::from(byte).to_string(),
            0x00..=0x7F => format!("\\x{:02X}", byte),
            _ => format!("\\x{:02X}", byte),
        })
        .collect()
}
