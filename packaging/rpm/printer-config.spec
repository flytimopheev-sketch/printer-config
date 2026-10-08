Name:           printer-config
Version:        1.0.0
Release:        1%{?dist}
Summary:        Printer-management web UI (Tauri frontend) — Red Hat / RedOS RPM

License:        MIT
URL:            https://github.com/<user>/printer-config
BuildArch:      noarch
BuildRequires:  python3

# --- Sources ---
# Source0 = vendored frontend dist/ (wrapped in a version-named folder so %setup
#           can extract it) so the package also builds from an SRPM.
# Sources 1-3 = local launchers (start script, .desktop, systemd unit).
# Source4   = README shipped into the doc directory.
Source0:        %{name}-%{version}-dist.tar.gz
Source1:        launcher/printer-config
Source2:        launcher/printer-config.desktop
Source3:        launcher/printer-config.service
Source4:        README.md

%description
printer-config is a Tauri 2 desktop application (React 19 + TypeScript + Vite)
for managing printer queues and discovering network printers over mDNS. This RPM
ships the compiled web UI plus a localhost HTTP launcher, which makes the UI easy
to test on Red Hat Enterprise Linux / RedOS without a desktop environment.

The native Rust backend (Windows Spooler API via winspool) is Windows-only and
stays out of scope for this RPM; a libcups/lp port is tracked as TODO.

%prep
%setup -q -n %{name}-%{version}

%build
# No compilation step: the web frontend is vendored in %{SOURCE0}.

%install
rm -rf %{buildroot}

mkdir -p %{buildroot}%{_datadir}/%{name}
cp -r %{_builddir}/%{name}-%{version}/dist/* %{buildroot}%{_datadir}/%{name}/

mkdir -p %{buildroot}%{_bindir}
cp %{SOURCE1} %{buildroot}%{_bindir}/printer-config
chmod 0755 %{buildroot}%{_bindir}/printer-config

mkdir -p %{buildroot}%{_datadir}/applications
cp %{SOURCE2} %{buildroot}%{_datadir}/applications/

mkdir -p %{buildroot}%{_unitdir}
cp %{SOURCE3} %{buildroot}%{_unitdir}/printer-config.service

mkdir -p %{buildroot}%{_datadir}/doc/%{name}
cp %{SOURCE4} %{buildroot}%{_datadir}/doc/%{name}/README.md

%files
%license %{SOURCE4}
%attr(0755,root,root) %{_bindir}/printer-config
%dir %{_datadir}/%{name}
%dir %{_datadir}/%{name}/dist
%{_datadir}/%{name}/dist/*
%{_datadir}/applications/printer-config.desktop
%{_unitdir}/printer-config.service
%{_datadir}/doc/%{name}/README.md

%clean
rm -rf %{buildroot}

%post
/usr/bin/systemctl daemon-reload >/dev/null 2>&1 || :
/usr/bin/systemctl enable printer-config.service >/dev/null 2>&1 || :
/usr/bin/systemctl restart printer-config.service >/dev/null 2>&1 || :

%preun
/usr/bin/systemctl stop printer-config.service >/dev/null 2>&1 || :
/usr/bin/systemctl disable printer-config.service >/dev/null 2>&1 || :

%changelog
* Mon Oct 08 2026 AI Coding Agent <agent@printer-config.local> - 1.0.0-1
- Initial RPM release for Red Hat Enterprise Linux / RedOS.
