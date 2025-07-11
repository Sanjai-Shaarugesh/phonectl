Name:           phonectl
Version:        1.0.0
Release:        1%{?dist}
Summary:        CLI tool to control Android phones via ADB

License:        MIT
URL:            https://github.com/Sanjai-Shaarugesh/phonectl
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust
BuildRequires:  cargo
Requires:       adb

%description
phonectl is a full-featured Rust CLI tool to control Android phones wirelessly via ADB.
It supports calling, unlocking, audio routing, contact search, and more.

%prep
%autosetup

%build
cargo build --release --frozen

%install
install -Dm0755 target/release/phonectl %{buildroot}%{_bindir}/phonectl

%files
%license LICENSE
%doc README.md
%{_bindir}/phonectl

%changelog
* Thu Jul 11 2025 Sanjai Shaarugesh <your.email@example.com> - 1.0.0-1
- Initial RPM release of phonectl
