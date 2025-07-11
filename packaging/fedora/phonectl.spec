Name:           phonectl
Version:        1.5
Release:        1%{?dist}
Summary:        Wireless ADB Phone Control CLI
License:        MIT
URL:            https://github.com/Sanjai-Shaarugesh/phonectl
Source0:        phonectl-%{version}.tar.gz
BuildRequires:  rust cargo
Requires:       android-tools sox alsa-utils nmap-ncat procps

%description
A Rust-based CLI for wireless ADB control of Android devices.

%prep
%setup -q

%build
cargo build --release

%install
install -Dm755 target/release/phonectl %{buildroot}%{_bindir}/phonectl
install -Dm644 assets/sndcpy.apk %{buildroot}%{_datadir}/phonectl/sndcpy.apk

%files
%{_bindir}/phonectl
%{_datadir}/phonectl/sndcpy.apk
%doc LICENSE README.md

%changelog
* Fri Jul 11 2025 Sanjai Shaarugesh <shaarugesh6@gmail.com> - 1.5-1
- Initial release


