# Fetch Browser

浏览器下载器。支持下载指定版本的 `Chromium` 和 `Firefox`。

## 安装（Windows）

在 `Powershell` 中输入如下命令：

```powershell
irm https://raw.githubusercontent.com/hamflx/fetchbrowser/master/install.ps1 | iex
```

## 使用

下载 `Chromium 98`

```powershell
fb 98
```

**注意：在特定平台第一次下载 `Chromium` 会比较慢，因为会联机查找版本信息，后续会使用缓存的数据。**

下载 `Firefox 98`

```powershell
fb --firefox 98
```
