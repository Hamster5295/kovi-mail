# Mail

Mail 是基于 [Kovi](https://kovi.thricecola.com/) 框架的邮件提醒插件，可以定期检查邮箱，并在有新邮件时提醒你。  

## 安装

1. 根据[教程](https://kovi.thricecola.com/start/fast.html)创建一个 Kovi 工程
2. 在项目根目录运行
```bash
cargo add kovi-plugin-mail
```

3. 在 `build_bot!` 宏中传入插件
```rust
let bot = build_bot!(kovi-plugin-mail /* 和其他你正在使用的插件，用 , 分割 */ );
```

## 配置

Mail 可以通过 `toml` 文件进行配置。如果配置文件不存在，则使用默认配置。  

```toml
# 检查间隔(分钟)
interval = 5

# 要检查的邮箱们
[[mails]]
server = "imap.xxx.com"         # IMAP 地址
email = "xxx@yyy.com"           # 你的邮箱
password = "SomeIMAPToken"      # 用于登录 IMAP 的密码 / Token，不同的邮箱系统可能有差异
notify_users = [12345678]       # 如果有新邮件，则向这个列表中的 QQ 发起提醒

```  

配置文件应放置于编译后与可执行文件同级的 `data/kovi-plugin-mail/config.toml` 中。