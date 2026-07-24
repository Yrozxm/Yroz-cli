Name:           yroz
Version:        0.1.0
Release:        1%{?dist}
Summary:        Universal software manager for Linux written in Rust
License:        MIT
URL:            https://github.com/Yrozxm/Yroz-cli
Source0:        https://github.com/Yrozxm/Yroz-cli/archive/refs/tags/v%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
Requires:       curl

%description
Universal software manager for Linux written in Rust.

%prep
%setup -q -n Yroz-cli-%{version}

%build
cargo build --release --locked

%install
rm -rf $RPM_BUILD_ROOT
install -D -m 755 target/release/yroz %{buildroot}%{_bindir}/yroz

%files
%{_bindir}/yroz

%changelog
* Fri Jul 24 2026 Yrozxm <aiiiilobinbutter@gmail.com> - 0.1.0-1
- Initial package release
