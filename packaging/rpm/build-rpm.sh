#!/usr/bin/env bash
# build-rpm.sh — assemble and build the Red Hat/RedOS RPM for printer-config.
#
# Prerequisites on the builder host  (usually a WSL2 / AlmaLinux / Rocky / RedOS shell):
#   - sudo dnf install -y rpm-build  (or `yum install rpm-build` on older systems)
#   - Node.js 20+ and npm (for the frontend build)
#   - cargo/rustc (only needed if you rebuild the Rust backend)
#
# Usage:
#   ./packaging/rpm/build-rpm.sh          # builds into ./rpmbuild/
#   APP_VERSION=1.2.3 ./packaging/rpm/build-rpm.sh   # custom version
#
# Output: rpmbuild/RPMS/noarch/printer-config-1.0.0-1.el9.noarch.rpm
set -euo pipefail

cd "$(dirname "$0")/../.."

APP_VERSION="${APP_VERSION:-1.0.0}"
NAME="${NAME:-printer-config}"
WORKDIR="${PWD}/rpmbuild/dist-wrap"
SOURCES_DIR="${PWD}/rpmbuild/SOURCES"
TOPDIR="${PWD}/rpmbuild"

echo "[*] npm run build"
npm run build

echo "[*] Preparing vendored frontend source tarball"
rm -rf "${WORKDIR}"
mkdir -p "${SOURCES_DIR}"
cp -a "dist" "${WORKDIR}/${NAME}-${APP_VERSION}"
tar -czf "${SOURCES_DIR}/${NAME}-${APP_VERSION}-dist.tar.gz" \
    -C "${WORKDIR}" "${NAME}-${APP_VERSION}"

for f in packaging/rpm/launcher/printer-config \
         packaging/rpm/launcher/printer-config.desktop \
         packaging/rpm/launcher/printer-config.service \
         README.md; do
  cp -f "$f" "${SOURCES_DIR}/"
done

echo "[*] Running rpmbuild"
rpmbuild -bb packaging/rpm/printer-config.spec \
  --define "_topdir ${TOPDIR}" \
  --define "_specdir ${PWD}/packaging/rpm" \
  --define "_builddir ${TOPDIR}/BUILD" \
  --define "_buildrootdir ${TOPDIR}/BUILDROOT" \
  --define "_rpmdir ${TOPDIR}/RPMS" \
  --define "_srcrpmdir ${TOPDIR}/SRPMS" \
  --define "_tmppath ${TOPDIR}/tmp" \
  --define "github_user ${GITHUB_USER:-}"

echo "[*] RPM built"
ls -l "${TOPDIR}/RPMS"/noarch/*/*.rpm 2>/dev/null || \
    ls -l "${TOPDIR}/SRPMS"/*.rpm
