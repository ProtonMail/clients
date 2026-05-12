#
# Build android artifact locally
#
# Run from the root of the repository
#
# ./mail/mail-uniffi/android/build-local.sh
set -eo pipefail

# Same as `rust-build/build_android.sh` (and `.cargo/config.toml` Android targets): required
# or `create_mail_session` panics with "Forcego feature is not defined".
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg forcego"
# Optional: faster local Rust cross-compile (arm64 device or arm64 emulator only):
#   MAIL_ANDROID_ABIS=arm64-v8a

# Build code
rust-build/build_android.sh mail-uniffi ./mail/mail-uniffi/uniffi.toml ./mail/mail-uniffi/android/lib/src/main/
# Build archive
./mail/mail-uniffi/android/build-android-archive.sh
rm -rf /tmp/rust-builds
mkdir /tmp/rust-builds/
# Copy artifacts
cp ./mail/mail-uniffi/android/lib/build/outputs/aar/lib-release.aar /tmp/rust-builds/
# Publish to local ~/.m2 (needed by proton-mail-et / Gradle as me.proton.mail.common:lib)
CRATE_VERSION=$(cargo pkgid --manifest-path=./mail/mail-uniffi/Cargo.toml | cut -d "#" -f2)
AAR_FILE=/tmp/rust-builds/lib-release.aar
if command -v mvn > /dev/null 2>&1; then
    mvn install:install-file \
        -Dfile="${AAR_FILE}" \
        -DgroupId=me.proton.mail.common \
        -DartifactId=lib \
        -Dversion="${CRATE_VERSION}" \
        -Dpackaging=aar \
        -DgeneratePom=true
else
    echo "mvn not on PATH; copying AAR + minimal POM into ~/.m2/repository (same layout as mvn install:install-file)."
    M2_REPO="${HOME}/.m2/repository"
    DEST="${M2_REPO}/me/proton/mail/common/lib/${CRATE_VERSION}"
    mkdir -p "${DEST}"
    cp "${AAR_FILE}" "${DEST}/lib-${CRATE_VERSION}.aar"
    cat > "${DEST}/lib-${CRATE_VERSION}.pom" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>me.proton.mail.common</groupId>
  <artifactId>lib</artifactId>
  <version>${CRATE_VERSION}</version>
  <packaging>aar</packaging>
</project>
EOF
    echo "Published ${DEST}/lib-${CRATE_VERSION}.aar"
fi
