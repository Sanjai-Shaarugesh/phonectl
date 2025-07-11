Name:           phonectl
Version:        1.5.0
Release:        1%{?dist}
Summary:        Wireless ADB phone control CLI

License:        MIT
URL:            https://github.com/Sanjai-Shaarugesh/phonectl
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  openssl-devel
BuildRequires:  pkgconfig
Requires:       android-tools
Requires:       sox
Requires:       alsa-utils
Requires:       nmap-ncat

%description
Full-featured Rust CLI for wireless ADB phone control with audio routing.

%prep
%autosetup

%build
export CARGO_HOME=%{_builddir}/cargo
cargo build --release

%install
mkdir -p %{buildroot}%{_bindir}
install -Dm755 target/release/phonectl %{buildroot}%{_bindir}/phonectl

%files
%{_bindir}/phonectl

%changelog
* Tue Jul 11 2023 Sanjai Shaarugesh <your@email.com> - 1.5.0-1
- Initial package
