# 📱 `phonectl` – Rust ADB Phone Control CLI

Control your Android phone over ADB from the terminal with ease.

> ⚙️ Built with Rust. Works on **Fedora**, **Debian**, **Ubuntu**, and **Arch Linux** systems.

---

## 🧰 Installation

### 🔧 From Source

```bash
git clone https://github.com/Sanjai-Shaarugesh/phonectl.git
cd phonectl
./install.sh
```

### ➰️ curl command 
```bash
curl -fsSL https://raw.githubusercontent.com/Sanjai-Shaarugesh/phonectl/master/install.sh | bash
```

> 📦 This compiles `phonectl` in release mode and installs it to `/usr/local/bin/`.

---

## ❌ Uninstallation

```bash
./uninstall.sh
```
## ➰️ curl command 
```bash
curl -fsSL https://raw.githubusercontent.com/Sanjai-Shaarugesh/phonectl/master/uninstall.sh | bash
```

> 🧹 This removes the `phonectl` binary from your system.

---

## 📋 Commands

| Command                     | Description                                     |
|----------------------------|-------------------------------------------------|
| `setup`                    | 🔌 Setup and connect ADB over Wi-Fi             |
| `unlock`                   | 🔓 Unlock the phone and keep it awake           |
| `list`                     | 📇 List all contacts                            |
| `search <query>`           | 🔍 Search contact by name or number             |
| `call <name\|number>`      | 📞 Call a contact or number                     |
| `dial <name\|number>`      | ☎️  Open dialer for a contact or number         |
| `answer`                   | ✅ Answer incoming call                         |
| `reject` / `end`           | ❌ End or reject the current call               |

---

## 📦 Package Manager Support (Coming Soon)

We're working on native packages for:

- 🐧 **Fedora** – via [COPR](https://copr.fedorainfracloud.org/)
- 🎩 **Ubuntu/Debian** – via `.deb` and Launchpad PPA
- 🅰️ **Arch Linux** – via the AUR (`yay -S phonectl`)

Stay tuned or contribute a package! 📬

---

## 📝 License

Licensed under the MIT License.
© 2025 [Sanjai Shaarugesh](https://github.com/Sanjai-Shaarugesh)
