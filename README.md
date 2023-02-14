# Fetch Browser

浏览器下载器。支持下载指定版本的 `Chromium` 和 `Firefox`。

A browser downloader. Supports downloading specified versions of Chromium and Firefox.

## 安装—Windows（Installation—Windows）

在 `Powershell` 中输入如下命令：

Enter the following command in Powershell:

```powershell
irm https://raw.githubusercontent.com/hamflx/fetchbrowser/master/install.ps1 | iex
```

## 使用（Usage）

下载 `Chromium 98`：

Download `Chromium 98`:

```powershell
fb 98
```

**注意：在特定平台第一次下载 `Chromium` 会比较慢，因为会联机查找版本信息，后续会使用缓存的数据。**

**Note: The first time downloading Chromium on a specific platform may be slow due to online version information lookup, but subsequent downloads will use cached data.**

下载 `Firefox 98`：

Download `Firefox 98`:

```powershell
fb --firefox 98
```

使用 socks5 代理：

Using SOCKS5 proxy:

```powershell
# 使用 socks5h 以使 DNS 通过代理解析。
# Use socks5h to resolve DNS through the proxy.
fb --proxy socks5h://127.0.0.1:10801 98

# 或仅使用 socks5 代理。
# Or simply use socks5 proxy.
fb --proxy socks5://127.0.0.1:10801 98
```

## 许可（License）

MIT @ 2023 hamflx
